use std;
use std::process::{Child, Command, Stdio};
use std::thread::{self, JoinHandle};

use crate::config::{EventKeyType, EventObserverConfig};
use crate::neon;
use crate::tests::neon_integrations::{get_account, submit_tx, test_observer};
use crate::tests::{make_contract_call, make_contract_publish, to_addr};
use clarity::types::chainstate::StacksAddress;
use clarity::util::get_epoch_time_secs;
use clarity::vm::database::ClaritySerializable;
use clarity::vm::representations::ContractName;
use clarity::vm::types::PrincipalData;
use clarity::vm::Value;
use reqwest::Response;
use stacks::burnchains::Burnchain;
use stacks::chainstate::burn::db::sortdb::SortitionDB;
use stacks::chainstate::stacks::{StacksPrivateKey, StacksTransaction, TransactionPayload};
use stacks::codec::StacksMessageCodec;
use stacks::net::CallReadOnlyRequestBody;
use stacks::net::RPCPeerInfoData;
use stacks::util::hash::hex_bytes;
use stacks::vm::types::QualifiedContractIdentifier;
use stacks::vm::ClarityName;
use std::convert::{TryFrom, TryInto};
use std::env;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(std::fmt::Debug)]
pub enum SubprocessError {
    SpawnFailed(String),
}

type SubprocessResult<T> = Result<T, SubprocessError>;

/// In charge of running L1 `stacks-node`.
pub struct StacksL1Controller {
    sub_process: Option<Child>,
    config_path: String,
    printer_handle: Option<JoinHandle<()>>,
    log_process: bool,
}

lazy_static! {
    pub static ref MOCKNET_PRIVATE_KEY_1: StacksPrivateKey = StacksPrivateKey::from_hex(
        "aaf57b4730f713cf942bc63f0801c4a62abe5a6ac8e3da10389f9ca3420b0dc701"
    )
    .unwrap();
    pub static ref MOCKNET_PRIVATE_KEY_2: StacksPrivateKey = StacksPrivateKey::from_hex(
        "0916e2eb04b5702e0e946081829cee67d3bb76e1792af506646843db9252ff4101"
    )
    .unwrap();
}

pub fn call_read_only(
    http_origin: &str,
    addr: &StacksAddress,
    contract_name: &str,
    function_name: &str,
    args: Vec<String>,
) -> serde_json::Value {
    let client = reqwest::blocking::Client::new();

    let path = format!(
        "{}/v2/contracts/call-read/{}/{}/{}",
        &http_origin, addr, contract_name, function_name
    );
    let principal: PrincipalData = addr.clone().into();
    let body = CallReadOnlyRequestBody {
        sender: principal.to_string(),
        arguments: args,
    };

    let read_info = client
        .post(&path)
        .json(&body)
        .send()
        .unwrap()
        .json::<serde_json::Value>()
        .unwrap();

    read_info
}

impl StacksL1Controller {
    pub fn new(config_path: String, log_process: bool) -> StacksL1Controller {
        StacksL1Controller {
            sub_process: None,
            config_path,
            printer_handle: None,
            log_process,
        }
    }

    pub fn start_process(&mut self) -> SubprocessResult<()> {
        let binary = match env::var("STACKS_BASE_DIR") {
            Err(_) => {
                // assume stacks-node is in path
                "stacks-node".into()
            }
            Ok(path) => path,
        };
        let mut command = Command::new(&binary);
        command
            .stderr(Stdio::piped())
            .arg("start")
            .arg("--config=".to_owned() + &self.config_path);

        info!("stacks-node mainchain spawn: {:?}", command);

        let mut process = match command.spawn() {
            Ok(child) => child,
            Err(e) => return Err(SubprocessError::SpawnFailed(format!("{:?}", e))),
        };

        let printer_handle = if self.log_process {
            let child_out = process.stderr.take().unwrap();
            Some(thread::spawn(|| {
                let buffered_out = BufReader::new(child_out);
                for line in buffered_out.lines() {
                    let line = match line {
                        Ok(x) => x,
                        Err(_e) => return,
                    };
                    println!("L1: {}", line);
                }
            }))
        } else {
            None
        };

        info!("stacks-node mainchain spawned, waiting for startup");

        self.sub_process = Some(process);
        self.printer_handle = printer_handle;

        Ok(())
    }

    pub fn kill_process(&mut self) {
        if let Some(mut sub_process) = self.sub_process.take() {
            sub_process.kill().unwrap();
        }
        if let Some(handle) = self.printer_handle.take() {
            println!("Joining print handler: {:?}", handle.join());
        }
    }
}

impl Drop for StacksL1Controller {
    fn drop(&mut self) {
        self.kill_process();
    }
}

/// Longest time to wait for a stacks block before aborting.
const PANIC_TIMEOUT_SECS: u64 = 600;

/// Height of the current stacks tip.
fn get_stacks_tip_height(sortition_db: &SortitionDB) -> i64 {
    let tip_snapshot = SortitionDB::get_canonical_burn_chain_tip(&sortition_db.conn())
        .expect("Could not read from SortitionDB.");

    tip_snapshot.canonical_stacks_tip_height.try_into().unwrap()
}

/// Wait for the *height* of the stacks chain tip to increment.
pub fn wait_for_next_stacks_block(sortition_db: &SortitionDB) -> bool {
    let current = get_stacks_tip_height(sortition_db);
    let mut next = current;
    info!(
        "wait_for_next_stacks_block: STARTS waiting at time {:?}, Stacks block height {:?}",
        Instant::now(),
        current
    );
    let start = Instant::now();
    while next <= current {
        if start.elapsed() > Duration::from_secs(PANIC_TIMEOUT_SECS) {
            panic!("Timed out waiting for block to process, aborting test.");
        }
        thread::sleep(Duration::from_millis(100));
        next = get_stacks_tip_height(sortition_db);
    }
    info!(
        "wait_for_next_stacks_block: STOPS waiting at time {:?}, Stacks block height {}",
        Instant::now(),
        next
    );
    true
}

/// Deserializes the `StacksTransaction` objects from `blocks` and returns all those that
/// match `test_fn`.
fn select_transactions_where(
    blocks: &Vec<serde_json::Value>,
    test_fn: fn(&StacksTransaction) -> bool,
) -> Vec<StacksTransaction> {
    let mut result = vec![];
    for block in blocks {
        let transactions = block.get("transactions").unwrap().as_array().unwrap();
        for tx in transactions.iter() {
            let raw_tx = tx.get("raw_tx").unwrap().as_str().unwrap();
            let tx_bytes = hex_bytes(&raw_tx[2..]).unwrap();
            let parsed = StacksTransaction::consensus_deserialize(&mut &tx_bytes[..]).unwrap();
            let test_value = test_fn(&parsed);
            if test_value {
                result.push(parsed);
            }
        }
    }

    return result;
}

/// This test brings up the Stacks-L1 chain in "mocknet" mode, and ensures that our listener can hear and record burn blocks
/// from the Stacks-L1 chain.
#[test]
fn l1_basic_listener_test() {
    if env::var("STACKS_NODE_TEST") != Ok("1".into()) {
        return;
    }

    // Start Stacks L1.
    let l1_toml_file = "../../contrib/conf/stacks-l1-mocknet.toml";
    let mut stacks_l1_controller = StacksL1Controller::new(l1_toml_file.to_string(), true);
    let _stacks_res = stacks_l1_controller
        .start_process()
        .expect("stacks l1 controller didn't start");

    // Start the L2 run loop.
    let mut config = super::new_test_conf();
    config.burnchain.first_burn_header_hash =
        "9946c68526249c259231f1660be4c72e915ebe1f25a8c8400095812b487eb279".to_string();
    config.burnchain.first_burn_header_height = 1;
    config.burnchain.chain = "stacks_layer_1".to_string();
    config.burnchain.mode = "hyperchain".to_string();
    config.burnchain.rpc_ssl = false;
    config.burnchain.rpc_port = 20443;
    config.burnchain.peer_host = "127.0.0.1".into();

    let mut run_loop = neon::RunLoop::new(config.clone());
    let termination_switch = run_loop.get_termination_switch();
    let run_loop_thread = thread::spawn(move || run_loop.start(None, 0));

    // Start Stacks L1.
    let l1_toml_file = "../../contrib/conf/stacks-l1-mocknet.toml";
    let mut stacks_l1_controller = StacksL1Controller::new(l1_toml_file.to_string(), true);
    let _stacks_res = stacks_l1_controller
        .start_process()
        .expect("stacks l1 controller didn't start");

    // Sleep to give the run loop time to listen to blocks.
    thread::sleep(Duration::from_millis(45000));

    // The burnchain should have registered what the listener recorded.
    let burnchain = Burnchain::new(
        &config.get_burn_db_path(),
        &config.burnchain.chain,
        &config.burnchain.mode,
    )
    .unwrap();
    let (_, burndb) = burnchain.open_db(true).unwrap();
    let tip = burndb
        .get_canonical_chain_tip()
        .expect("couldn't get chain tip");
    info!("burnblock chain tip is {:?}", &tip);

    // Ensure that the tip height has moved beyond height 0.
    // We check that we have moved past 3 just to establish we are reliably getting blocks.
    assert!(tip.block_height > 3);

    termination_switch.store(false, Ordering::SeqCst);
    stacks_l1_controller.kill_process();
    run_loop_thread.join().expect("Failed to join run loop.");
}

#[test]
fn l1_integration_test() {
    // running locally:
    // STACKS_BASE_DIR=~/devel/stacks-blockchain/target/release/stacks-node STACKS_NODE_TEST=1 cargo test --workspace l1_integration_test
    if env::var("STACKS_NODE_TEST") != Ok("1".into()) {
        return;
    }

    // Start Stacks L1.
    let l1_toml_file = "../../contrib/conf/stacks-l1-mocknet.toml";
    let l1_rpc_origin = "http://127.0.0.1:20443";
    let nft_trait_name = "nft-trait-standard";
    let ft_trait_name = "ft-trait-standard";

    // Start the L2 run loop.
    let mut config = super::new_test_conf();
    config.node.mining_key = Some(MOCKNET_PRIVATE_KEY_2.clone());
    let miner_account = to_addr(&MOCKNET_PRIVATE_KEY_2);

    config.burnchain.first_burn_header_hash =
        "9946c68526249c259231f1660be4c72e915ebe1f25a8c8400095812b487eb279".to_string();
    config.burnchain.first_burn_header_height = 1;
    config.burnchain.chain = "stacks_layer_1".to_string();
    config.burnchain.mode = "hyperchain".to_string();
    config.burnchain.rpc_ssl = false;
    config.burnchain.rpc_port = 20443;
    config.burnchain.peer_host = "127.0.0.1".into();
    config.node.wait_time_for_microblocks = 10_000;
    config.node.rpc_bind = "127.0.0.1:30443".into();
    config.node.p2p_bind = "127.0.0.1:30444".into();
    let l2_rpc_origin = format!("http://{}", &config.node.rpc_bind);

    config.burnchain.contract_identifier = QualifiedContractIdentifier::new(
        to_addr(&MOCKNET_PRIVATE_KEY_1).into(),
        "hyperchain-controller".into(),
    );

    config.node.miner = true;

    let mut run_loop = neon::RunLoop::new(config.clone());
    let termination_switch = run_loop.get_termination_switch();
    let run_loop_thread = thread::spawn(move || run_loop.start(None, 0));

    // Give the run loop time to start.
    thread::sleep(Duration::from_millis(2_000));

    let burnchain = Burnchain::new(
        &config.get_burn_db_path(),
        &config.burnchain.chain,
        &config.burnchain.mode,
    )
    .unwrap();
    let (sortition_db, burndb) = burnchain.open_db(true).unwrap();

    let mut stacks_l1_controller = StacksL1Controller::new(l1_toml_file.to_string(), true);
    let _stacks_res = stacks_l1_controller
        .start_process()
        .expect("stacks l1 controller didn't start");

    // Sleep to give the L1 chain time to start
    thread::sleep(Duration::from_millis(10_000));

    // Publish the NFT/FT traits
    let ft_trait_content =
        include_str!("../../../../core-contracts/contracts/helper/ft-trait-standard.clar");
    let ft_trait_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        0,
        1_000_000,
        &ft_trait_name,
        &ft_trait_content,
    );
    let nft_trait_content =
        include_str!("../../../../core-contracts/contracts/helper/nft-trait-standard.clar");
    let nft_trait_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        1,
        1_000_000,
        &nft_trait_name,
        &nft_trait_content,
    );

    // Publish the default hyperchains contract on the L1 chain
    let contract_content = include_str!("../../../../core-contracts/contracts/hyperchains.clar");
    let hc_contract_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        2,
        1_000_000,
        config.burnchain.contract_identifier.name.as_str(),
        &contract_content,
    );

    submit_tx(l1_rpc_origin, &ft_trait_publish);
    submit_tx(l1_rpc_origin, &nft_trait_publish);
    // Because the nonce ensures that the FT contract and NFT contract
    // are published before the HC contract, we can broadcast them
    // all at once, even though the HC contract depends on those
    // contracts.
    submit_tx(l1_rpc_origin, &hc_contract_publish);

    println!("Submitted FT, NFT, and Hyperchain contracts!");

    // Wait for exactly two stacks blocks.
    wait_for_next_stacks_block(&sortition_db);
    wait_for_next_stacks_block(&sortition_db);

    // The burnchain should have registered what the listener recorded.
    let tip = burndb
        .get_canonical_chain_tip()
        .expect("couldn't get chain tip");
    info!("burnblock chain tip is {:?}", &tip);

    // Ensure that the tip height has moved beyond height 0.
    // We check that we have moved past 3 just to establish we are reliably getting blocks.
    assert!(tip.block_height > 3);

    eprintln!("Miner account: {}", miner_account);

    // test the miner's nonce has incremented: this shows that L2 blocks have
    //  been mined (because the coinbase transactions bump the miner's nonce)
    let account = get_account(&l2_rpc_origin, &miner_account);
    assert_eq!(account.balance, 0);
    assert!(
        account.nonce >= 2,
        "Miner should have produced at least 2 coinbase transactions"
    );

    termination_switch.store(false, Ordering::SeqCst);
    stacks_l1_controller.kill_process();
    run_loop_thread.join().expect("Failed to join run loop.");
}

#[test]
fn l1_deposit_asset_integration_test() {
    // running locally:
    // STACKS_BASE_DIR=~/devel/stacks-blockchain/target/release/stacks-node STACKS_NODE_TEST=1 cargo test --workspace l1_deposit_asset_integration_test
    if env::var("STACKS_NODE_TEST") != Ok("1".into()) {
        return;
    }

    // Start Stacks L1.
    let l1_toml_file = "../../contrib/conf/stacks-l1-mocknet.toml";
    let l1_rpc_origin = "http://127.0.0.1:20443";
    let nft_trait_name = "nft-trait-standard";
    let ft_trait_name = "ft-trait-standard";

    // Start the L2 run loop.
    let mut config = super::new_test_conf();
    config.node.mining_key = Some(MOCKNET_PRIVATE_KEY_2.clone());
    let miner_account = to_addr(&MOCKNET_PRIVATE_KEY_2);
    let user_addr = to_addr(&MOCKNET_PRIVATE_KEY_1);
    config.add_initial_balance(user_addr.to_string(), 10000000);
    config.add_initial_balance(miner_account.to_string(), 10000000);

    config.burnchain.first_burn_header_hash =
        "9946c68526249c259231f1660be4c72e915ebe1f25a8c8400095812b487eb279".to_string();
    config.burnchain.first_burn_header_height = 1;
    config.burnchain.chain = "stacks_layer_1".to_string();
    config.burnchain.mode = "hyperchain".to_string();
    config.burnchain.rpc_ssl = false;
    config.burnchain.rpc_port = 20443;
    config.burnchain.peer_host = "127.0.0.1".into();
    config.node.wait_time_for_microblocks = 10_000;
    config.node.rpc_bind = "127.0.0.1:30443".into();
    config.node.p2p_bind = "127.0.0.1:30444".into();
    let l2_rpc_origin = format!("http://{}", &config.node.rpc_bind);
    let mut l2_nonce = 0;

    config.burnchain.contract_identifier =
        QualifiedContractIdentifier::new(user_addr.into(), "hyperchain-controller".into());

    config.node.miner = true;

    let mut run_loop = neon::RunLoop::new(config.clone());
    let termination_switch = run_loop.get_termination_switch();
    let run_loop_thread = thread::spawn(move || run_loop.start(None, 0));

    // Give the run loop time to start.
    thread::sleep(Duration::from_millis(2_000));

    let mut stacks_l1_controller = StacksL1Controller::new(l1_toml_file.to_string(), true);
    let _stacks_res = stacks_l1_controller
        .start_process()
        .expect("stacks l1 controller didn't start");
    let mut l1_nonce = 0;

    // Sleep to give the L1 chain time to start
    thread::sleep(Duration::from_millis(10_000));

    // Publish the NFT/FT traits
    let ft_trait_content =
        include_str!("../../../../core-contracts/contracts/helper/ft-trait-standard.clar");
    let ft_trait_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        &ft_trait_name,
        &ft_trait_content,
    );
    l1_nonce += 1;
    let nft_trait_content =
        include_str!("../../../../core-contracts/contracts/helper/nft-trait-standard.clar");
    let nft_trait_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        &nft_trait_name,
        &nft_trait_content,
    );
    l1_nonce += 1;
    // Publish a simple FT and NFT
    let ft_content = include_str!("../../../../core-contracts/contracts/helper/simple-ft.clar");
    let ft_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        "simple-ft",
        &ft_content,
    );
    l1_nonce += 1;
    let ft_contract_name = ContractName::from("simple-ft");
    let ft_contract_id = QualifiedContractIdentifier::new(user_addr.into(), ft_contract_name);
    let nft_content = include_str!("../../../../core-contracts/contracts/helper/simple-nft.clar");
    let nft_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        "simple-nft",
        &nft_content,
    );
    l1_nonce += 1;
    let nft_contract_name = ContractName::from("simple-nft");
    let nft_contract_id = QualifiedContractIdentifier::new(user_addr.into(), nft_contract_name);

    // Publish the default hyperchains contract on the L1 chain
    let contract_content = include_str!("../../../../core-contracts/contracts/hyperchains.clar");
    let hc_contract_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        config.burnchain.contract_identifier.name.as_str(),
        &contract_content,
    );
    l1_nonce += 1;

    submit_tx(l1_rpc_origin, &ft_trait_publish);
    submit_tx(l1_rpc_origin, &nft_trait_publish);
    submit_tx(l1_rpc_origin, &nft_publish);
    submit_tx(l1_rpc_origin, &ft_publish);
    // Because the nonce ensures that the FT contract and NFT contract
    // are published before the HC contract, we can broadcast them
    // all at once, even though the HC contract depends on those
    // contracts.
    submit_tx(l1_rpc_origin, &hc_contract_publish);

    println!("Submitted FT, NFT, and Hyperchain contracts!");

    // Sleep to give the run loop time to listen to blocks,
    //  and start mining L2 blocks
    thread::sleep(Duration::from_secs(60));

    // The burnchain should have registered what the listener recorded.
    let burnchain = Burnchain::new(
        &config.get_burn_db_path(),
        &config.burnchain.chain,
        &config.burnchain.mode,
    )
    .unwrap();
    let (_, burndb) = burnchain.open_db(true).unwrap();
    let tip = burndb
        .get_canonical_chain_tip()
        .expect("couldn't get chain tip");

    // Ensure that the tip height has moved beyond height 0.
    // We check that we have moved past 3 just to establish we are reliably getting blocks.
    assert!(tip.block_height > 3);

    // test the miner's nonce has incremented: this shows that L2 blocks have
    //  been mined (because the coinbase transactions bump the miner's nonce)
    let account = get_account(&l2_rpc_origin, &miner_account);
    assert!(
        account.nonce >= 2,
        "Miner should have produced at least 2 coinbase transactions"
    );

    // Publish hyperchains contract for ft-token
    let hyperchain_simple_ft = "
    (define-fungible-token ft-token)

    (define-public (hyperchain-deposit-ft-token (amount uint) (recipient principal))
      (ft-mint? ft-token amount recipient)
    )

    (define-read-only (get-token-balance (user principal))
        (ft-get-balance ft-token user)
    )
    ";
    let hyperchain_ft_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        l2_nonce,
        1_000_000,
        "simple-ft",
        hyperchain_simple_ft,
    );
    l2_nonce += 1;
    let hc_ft_contract_id =
        QualifiedContractIdentifier::new(user_addr.into(), ContractName::from("simple-ft"));
    // Publish hyperchains contract for nft-token
    let hyperchain_simple_nft = "
    (define-non-fungible-token nft-token uint)

    (define-public (hyperchain-deposit-nft-token (id uint) (recipient principal))
      (nft-mint? nft-token id recipient)
    )

    (define-read-only (get-token-owner (id uint))
        (nft-get-owner? nft-token id)
    )
    ";
    let hyperchain_nft_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        l2_nonce,
        1_000_000,
        "simple-nft",
        hyperchain_simple_nft,
    );
    l2_nonce += 1;
    let hc_nft_contract_id =
        QualifiedContractIdentifier::new(user_addr.into(), ContractName::from("simple-nft"));

    // Mint a ft-token for user on L1 chain (amount = 1)
    let l1_mint_ft_tx = make_contract_call(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        &user_addr,
        "simple-ft",
        "gift-tokens",
        &[Value::Principal(user_addr.into())],
    );
    l1_nonce += 1;
    // Mint a nft-token for user on L1 chain (ID = 1)
    let l1_mint_nft_tx = make_contract_call(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        &user_addr,
        "simple-nft",
        "test-mint",
        &[Value::Principal(user_addr.into())],
    );
    l1_nonce += 1;

    // Setup hyperchains contract
    let hc_setup_tx = make_contract_call(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        &user_addr,
        config.burnchain.contract_identifier.name.as_str(),
        "setup-allowed-contracts",
        &[],
    );
    l1_nonce += 1;

    submit_tx(&l2_rpc_origin, &hyperchain_ft_publish);
    submit_tx(&l2_rpc_origin, &hyperchain_nft_publish);
    submit_tx(l1_rpc_origin, &l1_mint_ft_tx);
    submit_tx(l1_rpc_origin, &l1_mint_nft_tx);
    submit_tx(l1_rpc_origin, &hc_setup_tx);

    // Sleep to give the run loop time to mine a block
    thread::sleep(Duration::from_secs(30));

    // Check that the user does not own any of the fungible tokens on the hyperchain now
    let res = call_read_only(
        &l2_rpc_origin,
        &user_addr,
        "simple-ft",
        "get-token-balance",
        vec![Value::Principal(user_addr.into()).serialize()],
    );
    assert!(res.get("cause").is_none());
    assert!(res["okay"].as_bool().unwrap());
    assert_eq!(res["result"], "0x0100000000000000000000000000000000");
    // Check that the user does not own the NFT on the hyperchain now
    let res = call_read_only(
        &l2_rpc_origin,
        &user_addr,
        "simple-nft",
        "get-token-owner",
        vec![Value::UInt(1).serialize()],
    );
    assert!(res.get("cause").is_none());
    assert!(res["okay"].as_bool().unwrap());
    let mut result = res["result"].as_str().unwrap().to_string();
    result = result.strip_prefix("0x").unwrap().to_string();
    assert_eq!(result, Value::none().serialize());

    let l1_deposit_ft_tx = make_contract_call(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        &user_addr,
        config.burnchain.contract_identifier.name.as_str(),
        "deposit-ft-asset",
        &[
            Value::UInt(1),
            Value::Principal(user_addr.into()),
            Value::none(),
            Value::Principal(PrincipalData::Contract(ft_contract_id)),
            Value::Principal(PrincipalData::Contract(hc_ft_contract_id.clone())),
        ],
    );
    l1_nonce += 1;
    let l1_deposit_nft_tx = make_contract_call(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        &user_addr,
        config.burnchain.contract_identifier.name.as_str(),
        "deposit-nft-asset",
        &[
            Value::UInt(1),
            Value::Principal(user_addr.into()),
            Value::Principal(PrincipalData::Contract(nft_contract_id)),
            Value::Principal(PrincipalData::Contract(hc_nft_contract_id.clone())),
        ],
    );
    l1_nonce += 1;

    // deposit ft-token into hyperchains contract on L1
    submit_tx(&l1_rpc_origin, &l1_deposit_ft_tx);
    // deposit nft-token into hyperchains contract on L1
    submit_tx(&l1_rpc_origin, &l1_deposit_nft_tx);

    // Sleep to give the run loop time to mine a block
    thread::sleep(Duration::from_secs(25));

    // Check that the user owns a fungible token on the hyperchain now
    let res = call_read_only(
        &l2_rpc_origin,
        &user_addr,
        "simple-ft",
        "get-token-balance",
        vec![Value::Principal(user_addr.into()).serialize()],
    );
    assert!(res.get("cause").is_none());
    assert!(res["okay"].as_bool().unwrap());
    assert_eq!(res["result"], "0x0100000000000000000000000000000001");
    // Check that the user owns the NFT on the hyperchain now
    let res = call_read_only(
        &l2_rpc_origin,
        &user_addr,
        "simple-nft",
        "get-token-owner",
        vec![Value::UInt(1).serialize()],
    );
    assert!(res.get("cause").is_none());
    assert!(res["okay"].as_bool().unwrap());
    let mut result = res["result"].as_str().unwrap().to_string();
    result = result.strip_prefix("0x").unwrap().to_string();
    assert_eq!(
        result,
        Value::some(Value::Principal(user_addr.into()))
            .unwrap()
            .serialize()
    );

    termination_switch.store(false, Ordering::SeqCst);
    stacks_l1_controller.kill_process();
    run_loop_thread.join().expect("Failed to join run loop.");
}

/// This test calls the `deposit-stx` function in the hyperchains contract.
/// We expect to see the stx balance for the user in question increase.
#[test]
fn l1_deposit_stx_integration_test() {
    // running locally:
    // STACKS_BASE_DIR=~/devel/stacks-blockchain/target/release/stacks-node STACKS_NODE_TEST=1 cargo test --workspace l1_deposit_stx_integration_test
    if env::var("STACKS_NODE_TEST") != Ok("1".into()) {
        return;
    }

    // Start Stacks L1.
    let l1_toml_file = "../../contrib/conf/stacks-l1-mocknet.toml";
    let l1_rpc_origin = "http://127.0.0.1:20443";

    // Start the L2 run loop.
    let mut config = super::new_test_conf();
    config.node.mining_key = Some(MOCKNET_PRIVATE_KEY_2.clone());
    let miner_account = to_addr(&MOCKNET_PRIVATE_KEY_2);
    let user_addr = to_addr(&MOCKNET_PRIVATE_KEY_1);
    config.add_initial_balance(user_addr.to_string(), 10000000);
    config.add_initial_balance(miner_account.to_string(), 10000000);

    config.burnchain.first_burn_header_hash =
        "9946c68526249c259231f1660be4c72e915ebe1f25a8c8400095812b487eb279".to_string();
    config.burnchain.first_burn_header_height = 1;
    config.burnchain.chain = "stacks_layer_1".to_string();
    config.burnchain.mode = "hyperchain".to_string();
    config.burnchain.rpc_ssl = false;
    config.burnchain.rpc_port = 20443;
    config.burnchain.peer_host = "127.0.0.1".into();
    config.node.wait_time_for_microblocks = 10_000;
    config.node.rpc_bind = "127.0.0.1:30443".into();
    config.node.p2p_bind = "127.0.0.1:30444".into();
    let l2_rpc_origin = format!("http://{}", &config.node.rpc_bind);

    let mut l2_nonce = 0;

    config.burnchain.contract_identifier =
        QualifiedContractIdentifier::new(user_addr.into(), "hyperchain-controller".into());

    config.node.miner = true;

    let mut run_loop = neon::RunLoop::new(config.clone());
    let termination_switch = run_loop.get_termination_switch();
    let run_loop_thread = thread::spawn(move || run_loop.start(None, 0));

    // Sleep to give the run loop time to start
    thread::sleep(Duration::from_millis(2_000));

    let burnchain = Burnchain::new(
        &config.get_burn_db_path(),
        &config.burnchain.chain,
        &config.burnchain.mode,
    )
    .unwrap();
    let (_, burndb) = burnchain.open_db(true).unwrap();

    let mut stacks_l1_controller = StacksL1Controller::new(l1_toml_file.to_string(), true);
    let _stacks_res = stacks_l1_controller
        .start_process()
        .expect("stacks l1 controller didn't start");
    let mut l1_nonce = 0;

    // Sleep to give the L1 chain time to start
    thread::sleep(Duration::from_millis(10_000));

    // Publish the NFT/FT traits
    let ft_trait_content =
        include_str!("../../../../core-contracts/contracts/helper/ft-trait-standard.clar");
    let ft_trait_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        "ft-trait-standard",
        &ft_trait_content,
    );
    l1_nonce += 1;

    let nft_trait_content =
        include_str!("../../../../core-contracts/contracts/helper/nft-trait-standard.clar");
    let nft_trait_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        "nft-trait-standard",
        &nft_trait_content,
    );
    l1_nonce += 1;
    // Publish the default hyperchains contract on the L1 chain
    let contract_content = include_str!("../../../../core-contracts/contracts/hyperchains.clar");
    let hc_contract_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        config.burnchain.contract_identifier.name.as_str(),
        &contract_content,
    );

    l1_nonce += 1;
    submit_tx(l1_rpc_origin, &ft_trait_publish);
    submit_tx(l1_rpc_origin, &nft_trait_publish);
    submit_tx(l1_rpc_origin, &hc_contract_publish);

    // Sleep to give the run loop time to listen to blocks,
    //  and start mining L2 blocks
    thread::sleep(Duration::from_secs(60));

    // The burnchain should have registered what the listener recorded.
    let tip = burndb
        .get_canonical_chain_tip()
        .expect("couldn't get chain tip");

    // Ensure that the tip height has moved beyond height 0.
    // We check that we have moved past 3 just to establish we are reliably getting blocks.
    assert!(tip.block_height > 3);

    // test the miner's nonce has incremented: this shows that L2 blocks have
    //  been mined (because the coinbase transactions bump the miner's nonce)
    let account = get_account(&l2_rpc_origin, &miner_account);
    assert!(
        account.nonce >= 2,
        "Miner should have produced at least 2 coinbase transactions"
    );

    // Setup hyperchains contract
    let hc_setup_tx = make_contract_call(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        &user_addr,
        config.burnchain.contract_identifier.name.as_str(),
        "setup-allowed-contracts",
        &[],
    );
    l1_nonce += 1;
    submit_tx(l1_rpc_origin, &hc_setup_tx);

    // Sleep to give the run loop time to listen to blocks
    thread::sleep(Duration::from_secs(30));

    // Check that the user does not own any STX on the hyperchain now
    let account = get_account(&l2_rpc_origin, &user_addr);
    assert_eq!(account.balance, 10000000);

    let l1_deposit_stx_tx = make_contract_call(
        &MOCKNET_PRIVATE_KEY_1,
        l1_nonce,
        1_000_000,
        &user_addr,
        config.burnchain.contract_identifier.name.as_str(),
        "deposit-stx",
        &[Value::UInt(1), Value::Principal(user_addr.into())],
    );
    l1_nonce += 1;

    // deposit stx into hyperchains contract on L1
    submit_tx(&l1_rpc_origin, &l1_deposit_stx_tx);

    // Sleep to give the run loop time to mine a block
    thread::sleep(Duration::from_secs(25));

    // Check that the user owns STX on the hyperchain now
    let account = get_account(&l2_rpc_origin, &user_addr);
    assert_eq!(account.balance, 10000001);
    termination_switch.store(false, Ordering::SeqCst);
    stacks_l1_controller.kill_process();
    run_loop_thread.join().expect("Failed to join run loop.");
}

/// Test that we can bring up an L2 node and make some simple calls to the L2 chain.
/// Set up the L2 chain, make N calls, check that they are found in the listener.
#[test]
fn l2_simple_contract_calls() {
    if env::var("STACKS_NODE_TEST") != Ok("1".into()) {
        return;
    }

    // Start Stacks L1.
    let l1_toml_file = "../../contrib/conf/stacks-l1-mocknet.toml";
    let l1_rpc_origin = "http://127.0.0.1:20443";

    let nft_trait_name = "nft-trait-standard";
    let ft_trait_name = "ft-trait-standard";

    // Start the L2 run loop.
    let mut config = super::new_test_conf();
    config.node.mining_key = Some(MOCKNET_PRIVATE_KEY_2.clone());

    config.burnchain.first_burn_header_hash =
        "9946c68526249c259231f1660be4c72e915ebe1f25a8c8400095812b487eb279".to_string();
    config.burnchain.first_burn_header_height = 1;
    config.burnchain.chain = "stacks_layer_1".to_string();
    config.burnchain.mode = "hyperchain".to_string();
    config.burnchain.rpc_ssl = false;
    config.burnchain.rpc_port = 20443;
    config.burnchain.peer_host = "127.0.0.1".into();
    config.node.wait_time_for_microblocks = 10_000;
    config.node.rpc_bind = "127.0.0.1:30443".into();
    config.node.p2p_bind = "127.0.0.1:30444".into();
    let l2_rpc_origin = format!("http://{}", &config.node.rpc_bind);

    config.burnchain.contract_identifier = QualifiedContractIdentifier::new(
        to_addr(&MOCKNET_PRIVATE_KEY_1).into(),
        "hyperchain-controller".into(),
    );

    config.node.miner = true;

    let user_addr = to_addr(&MOCKNET_PRIVATE_KEY_1);
    config.add_initial_balance(user_addr.to_string(), 10000000);

    config.events_observers.push(EventObserverConfig {
        endpoint: format!("localhost:{}", test_observer::EVENT_OBSERVER_PORT),
        events_keys: vec![EventKeyType::AnyEvent],
    });

    test_observer::spawn();

    let mut run_loop = neon::RunLoop::new(config.clone());
    let termination_switch = run_loop.get_termination_switch();
    let run_loop_thread = thread::spawn(move || run_loop.start(None, 0));

    // Sleep to give the run loop time to start
    thread::sleep(Duration::from_millis(2_000));

    let burnchain = Burnchain::new(
        &config.get_burn_db_path(),
        &config.burnchain.chain,
        &config.burnchain.mode,
    )
    .unwrap();
    let (sortition_db, _) = burnchain.open_db(true).unwrap();

    let mut stacks_l1_controller = StacksL1Controller::new(l1_toml_file.to_string(), true);
    let _stacks_res = stacks_l1_controller
        .start_process()
        .expect("stacks l1 controller didn't start");
    // Sleep to give the L1 chain time to start
    thread::sleep(Duration::from_millis(10_000));

    // Publish the NFT/FT traits
    let ft_trait_content =
        include_str!("../../../../core-contracts/contracts/helper/ft-trait-standard.clar");
    let ft_trait_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        0,
        1_000_000,
        &ft_trait_name,
        &ft_trait_content,
    );
    let nft_trait_content =
        include_str!("../../../../core-contracts/contracts/helper/nft-trait-standard.clar");
    let nft_trait_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        1,
        1_000_000,
        &nft_trait_name,
        &nft_trait_content,
    );
    // Publish the default hyperchains contract on the L1 chain
    let contract_content = include_str!("../../../../core-contracts/contracts/hyperchains.clar");
    let hc_contract_publish = make_contract_publish(
        &MOCKNET_PRIVATE_KEY_1,
        2,
        1_000_000,
        config.burnchain.contract_identifier.name.as_str(),
        &contract_content,
    );

    submit_tx(l1_rpc_origin, &ft_trait_publish);
    submit_tx(l1_rpc_origin, &nft_trait_publish);
    submit_tx(l1_rpc_origin, &hc_contract_publish);
    println!("Submitted FT, NFT, and Hyperchain contracts!");

    wait_for_next_stacks_block(&sortition_db);
    wait_for_next_stacks_block(&sortition_db);

    let small_contract = "(define-public (return-one) (ok 1))";
    let mut l2_nonce = 0;
    {
        let hyperchain_small_contract_publish = make_contract_publish(
            &MOCKNET_PRIVATE_KEY_1,
            l2_nonce,
            1000,
            "small-contract",
            small_contract,
        );
        l2_nonce += 1;
        submit_tx(&l2_rpc_origin, &hyperchain_small_contract_publish);
    }
    wait_for_next_stacks_block(&sortition_db);
    wait_for_next_stacks_block(&sortition_db);

    // Make two contract calls to "return-one".
    for _ in 0..2 {
        let small_contract_call1 = make_contract_call(
            &MOCKNET_PRIVATE_KEY_1,
            l2_nonce,
            1000,
            &user_addr,
            "small-contract",
            "return-one",
            &[],
        );
        l2_nonce += 1;
        submit_tx(&l2_rpc_origin, &small_contract_call1);
        wait_for_next_stacks_block(&sortition_db);
    }
    // Wait extra blocks to avoid flakes.
    wait_for_next_stacks_block(&sortition_db);
    wait_for_next_stacks_block(&sortition_db);

    // Check for two calls to "return-one".
    let small_contract_calls = select_transactions_where(
        &test_observer::get_blocks(),
        |transaction| match &transaction.payload {
            TransactionPayload::ContractCall(contract) => {
                contract.contract_name == ContractName::try_from("small-contract").unwrap()
                    && contract.function_name == ClarityName::try_from("return-one").unwrap()
            }
            _ => false,
        },
    );
    assert_eq!(small_contract_calls.len(), 2);
    termination_switch.store(false, Ordering::SeqCst);
    stacks_l1_controller.kill_process();
    run_loop_thread.join().expect("Failed to join run loop.");
}

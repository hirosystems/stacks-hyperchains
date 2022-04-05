use crate::burnchains::tests::{make_test_new_block, random_sortdb_test_dir};
use crate::config::BurnchainConfig;
use crate::stacks::burnchains::BurnchainIndexer;
use crate::{burnchains::db_indexer::DBBurnchainIndexer, rand::RngCore};
use rand;
use stacks::burnchains::events::{NewBlock, NewBlockTxEvent};
use stacks::types::chainstate::{BurnchainHeaderHash, StacksBlockId};
use stacks::util::hash::to_hex;

/// Create config settings for the tests.
fn make_test_config() -> BurnchainConfig {
    let db_path_dir = random_sortdb_test_dir();
    info!("db_path_dir {:?}", &db_path_dir);
    let mut config = BurnchainConfig::default();
    config.indexer_base_db_path = db_path_dir;
    config.first_burn_header_hash =
        "1111111111111111111111111111111111111111111111111111111111111111".to_string();
    config
}

/// Make indexer with test settings.
fn make_test_indexer() -> Box<dyn BurnchainIndexer> {
    Box::new(DBBurnchainIndexer::new(make_test_config()).expect("Couldn't create indexer."))
}

/// Tests that we can make a DBBurnchainIndexer and connect.
#[test]
fn test_connect() {
    let mut indexer = make_test_indexer();
    indexer.connect(true).expect("Couldn't connect.");
}

/// Make indexer with test settings and add 10 test new blocks.
fn make_test_indexer_add_10_block_branch() -> Box<dyn BurnchainIndexer> {
    let mut indexer = make_test_indexer();
    indexer.connect(true).expect("Couldn't connect.");

    let input_channel = indexer.get_input_channel();

    // Add heights up to 10.
    for block_idx in 1..11 {
        let new_block = make_test_new_block(
            block_idx,
            block_idx as u8,
            (block_idx - 1) as u8,
            make_test_config().contract_identifier.clone(),
        );
        input_channel
            .push_block(new_block)
            .expect("Failed to push block");
    }

    indexer
}
/// Tests that we can open an input channel, input some blocks, and see that reflected
/// in `get_highest_header_height`.
#[test]
fn test_highest_height() {
    let indexer = make_test_indexer_add_10_block_branch();
    let highest_height = indexer
        .get_highest_header_height()
        .expect("Couldn't get height");
    assert_eq!(10, highest_height);
}

#[test]
fn test_read_headers() {
    let indexer = make_test_indexer_add_10_block_branch();
    let headers = indexer.read_headers(1, 11).expect("Couldn't get height");
    for header in &headers {
        info!("{:?}", &header);
    }
    assert_eq!(10, headers.len());
}

/// Create the following fork:
///    / 3
/// 1
///    \ 2 -> 4
///
/// These are added in the order [1, 3, 2, 4]. Becasue of lexicographic tie-breaking based on hash,
/// the first (only) reorg is at 4.
#[test]
fn test_detect_reorg() {
    let mut indexer = make_test_indexer();
    indexer.connect(true).expect("Couldn't connect.");

    let input_channel = indexer.get_input_channel();

    let contract_identifier = make_test_config().contract_identifier.clone();
    input_channel
        .push_block(make_test_new_block(
            0,
            1u8,
            0u8,
            contract_identifier.clone(),
        ))
        .expect("Failed to push block");
    // Highest height is 0.
    assert_eq!(
        0,
        indexer
            .find_chain_reorg()
            .expect("Call to `find_chain_reorg` failed.")
    );

    input_channel
        .push_block(make_test_new_block(
            1,
            3u8,
            1u8,
            contract_identifier.clone(),
        ))
        .expect("Failed to push block");
    // Only one chain, at height 1.
    assert_eq!(
        1,
        indexer
            .find_chain_reorg()
            .expect("Call to `find_chain_reorg` failed.")
    );

    input_channel
        .push_block(make_test_new_block(
            1,
            2u8,
            1u8,
            contract_identifier.clone(),
        ))
        .expect("Failed to push block");
    // Chain tip hasn't changed based on lexicographic tie-breaking. Same chain tip as before.
    assert_eq!(
        1,
        indexer
            .find_chain_reorg()
            .expect("Call to `find_chain_reorg` failed.")
    );

    input_channel
        .push_block(make_test_new_block(
            2,
            4u8,
            2u8,
            contract_identifier.clone(),
        ))
        .expect("Failed to push block");
    // New chain tip, common ancestor is at height 0.
    assert_eq!(
        0,
        indexer
            .find_chain_reorg()
            .expect("Call to `find_chain_reorg` failed.")
    );
}

// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020 Stacks Open Internet Foundation
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use crate::burnchains::Burnchain;
// This module contains the "main loop" that drives everything
use crate::burnchains::Error as burnchain_error;
use crate::chainstate::burn::ConsensusHash;
use clarity::vm::costs::ExecutionCost;
use clarity::vm::types::QualifiedContractIdentifier;
use stacks_common::util::log;
use std::collections::HashSet;
use std::convert::TryFrom;

pub use self::mempool::MemPoolDB;
use crate::types::chainstate::StacksBlockId;
use crate::types::chainstate::{BlockHeaderHash, BurnchainHeaderHash};
use stacks_common::types::StacksEpoch as GenericStacksEpoch;
pub use stacks_common::types::StacksEpochId;
pub mod mempool;

#[cfg(test)]
pub mod tests;

use clarity::vm::ClarityVersion;
use std::cmp::Ord;
use std::cmp::Ordering;
use std::cmp::PartialOrd;

pub type StacksEpoch = GenericStacksEpoch<ExecutionCost>;

// fork set identifier -- to be mixed with the consensus hash (encodes the version)
pub const SYSTEM_FORK_SET_VERSION: [u8; 4] = [23u8, 0u8, 0u8, 0u8];

// chain id
pub const LAYER_1_CHAIN_ID_MAINNET: u32 = 0x00000001;
pub const LAYER_1_CHAIN_ID_TESTNET: u32 = 0x80000000;

/// Stacks epoch that we assume in subnets.
pub const SUBNETS_STACKS_EPOCH: StacksEpochId = StacksEpochId::Epoch21;

// ClarityVersion in use.
pub const SUBNETS_CLARITY_VERSION: ClarityVersion = ClarityVersion::Clarity2;

// peer version (big-endian)
// first byte == major network protocol version (currently 0x18)
// second and third bytes are unused
// fourth byte == highest epoch supported by this node
pub const PEER_VERSION_MAINNET_MAJOR: u32 = 0x18000000;
pub const PEER_VERSION_TESTNET_MAJOR: u32 = 0xfacade00;

pub const PEER_VERSION_EPOCH_1_0: u8 = 0x00;
pub const PEER_VERSION_EPOCH_2_0: u8 = 0x00;
pub const PEER_VERSION_EPOCH_2_05: u8 = 0x05;
pub const PEER_VERSION_EPOCH_2_1: u8 = 0x06;
pub const PEER_VERSION_EPOCH_2_2: u8 = 0x07;
pub const PEER_VERSION_EPOCH_2_3: u8 = 0x08;
pub const PEER_VERSION_EPOCH_2_4: u8 = 0x09;

// this should be updated to the latest network epoch version supported by
//  this node. this will be checked by the `validate_epochs()` method.
pub const PEER_NETWORK_EPOCH: u32 = PEER_VERSION_EPOCH_2_4 as u32;

// set the fourth byte of the peer version
pub const PEER_VERSION_MAINNET: u32 = PEER_VERSION_MAINNET_MAJOR | PEER_NETWORK_EPOCH;
pub const PEER_VERSION_TESTNET: u32 = PEER_VERSION_TESTNET_MAJOR | PEER_NETWORK_EPOCH;

// network identifiers
pub const NETWORK_ID_MAINNET: u32 = 0x17000000;
pub const NETWORK_ID_TESTNET: u32 = 0xff000000;

// default port
pub const NETWORK_P2P_PORT: u16 = 6265;

// sliding burnchain window over which a miner's past block-commit payouts will be used to weight
// its current block-commit in a sortition
pub const MINING_COMMITMENT_WINDOW: u8 = 6;

// This controls a miner heuristic for dropping a transaction from repeated consideration
//  in the mempool. If the transaction caused the block limit to be reached when the block
//  was previously `TX_BLOCK_LIMIT_PROPORTION_HEURISTIC`% full, the transaction will be dropped
//  from the mempool. 20% is chosen as a heuristic here to allow for large transactions to be
//  attempted, but if they cannot be included in an otherwise mostly empty block, not to consider
//  them again.
pub const TX_BLOCK_LIMIT_PROPORTION_HEURISTIC: u64 = 20;

pub const GENESIS_EPOCH: StacksEpochId = StacksEpochId::Epoch20;

/// The number of blocks which will share the block bonus
///   from burn blocks that occurred without a sortition.
///   (See: https://forum.stacks.org/t/pox-consensus-and-stx-future-supply)
#[cfg(test)]
pub const INITIAL_MINING_BONUS_WINDOW: u16 = 10;
#[cfg(not(test))]
pub const INITIAL_MINING_BONUS_WINDOW: u16 = 10_000;

pub const STACKS_EPOCH_MAX: u64 = i64::MAX as u64;

pub const SUBNET_GENESIS_ROOT_HASH: &str =
    "455fdc5a17b482edd66ae9ca20990bb57795b208a3a2ab0f7707f3ca6bb6560b";

/// This is the "dummy" parent to the actual first burnchain block that we process.
pub const FIRST_BURNCHAIN_CONSENSUS_HASH: ConsensusHash = ConsensusHash([0u8; 20]);

// TODO: TO BE SET BY STACKS_V1_MINER_THRESHOLD
pub const BITCOIN_MAINNET_FIRST_BLOCK_HEIGHT: u64 = 666050;
pub const BITCOIN_MAINNET_FIRST_BLOCK_TIMESTAMP: u32 = 1610643248;
pub const BITCOIN_MAINNET_FIRST_BLOCK_HASH: &str =
    "0000000000000000000ab248c8e35c574514d052a83dbc12669e19bc43df486e";
pub const BITCOIN_MAINNET_INITIAL_REWARD_START_BLOCK: u64 = 651389;
pub const BITCOIN_MAINNET_STACKS_2_05_BURN_HEIGHT: u64 = 713_000;
pub const BITCOIN_MAINNET_STACKS_21_BURN_HEIGHT: u64 = 781_551;
/// This is Epoch-2.2 activation height proposed in SIP-022
pub const BITCOIN_MAINNET_STACKS_22_BURN_HEIGHT: u64 = 787_651;
/// This is Epoch-2.3 activation height proposed in SIP-023
pub const BITCOIN_MAINNET_STACKS_23_BURN_HEIGHT: u64 = 788_240;
/// This is Epoch-2.3, now Epoch-2.4, activation height proposed in SIP-024
pub const BITCOIN_MAINNET_STACKS_24_BURN_HEIGHT: u64 = 791_551;

pub const BITCOIN_TESTNET_FIRST_BLOCK_HEIGHT: u64 = 2000000;
pub const BITCOIN_TESTNET_FIRST_BLOCK_TIMESTAMP: u32 = 1622691840;
pub const BITCOIN_TESTNET_FIRST_BLOCK_HASH: &str =
    "000000000000010dd0863ec3d7a0bae17c1957ae1de9cbcdae8e77aad33e3b8c";
pub const BITCOIN_TESTNET_STACKS_2_05_BURN_HEIGHT: u64 = 2_104_380;
pub const BITCOIN_TESTNET_STACKS_21_BURN_HEIGHT: u64 = 2_422_101;
pub const BITCOIN_TESTNET_STACKS_22_BURN_HEIGHT: u64 = 2_431_300;
pub const BITCOIN_TESTNET_STACKS_23_BURN_HEIGHT: u64 = 2_431_633;
pub const BITCOIN_TESTNET_STACKS_24_BURN_HEIGHT: u64 = 2_432_545;

pub const BITCOIN_REGTEST_FIRST_BLOCK_HEIGHT: u64 = 0;
pub const BITCOIN_REGTEST_FIRST_BLOCK_TIMESTAMP: u32 = 0;
pub const BITCOIN_REGTEST_FIRST_BLOCK_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

pub const FIRST_STACKS_BLOCK_HASH: BlockHeaderHash = BlockHeaderHash([0u8; 32]);
pub const EMPTY_MICROBLOCK_PARENT_HASH: BlockHeaderHash = BlockHeaderHash([0u8; 32]);

lazy_static! {
    pub static ref FIRST_STACKS_BLOCK_ID: StacksBlockId =
        StacksBlockId::new(&FIRST_BURNCHAIN_CONSENSUS_HASH, &FIRST_STACKS_BLOCK_HASH);
    pub static ref DEFAULT_SUBNET_GOVERNING_CONTRACT: QualifiedContractIdentifier =
        QualifiedContractIdentifier::parse("STXMJXCJDCT4WPF2X1HE42T6ZCCK3TPMBRZ51JEG.subnet")
            .unwrap();
}

pub const BOOT_BLOCK_HASH: BlockHeaderHash = BlockHeaderHash([0xff; 32]);
pub const BURNCHAIN_BOOT_CONSENSUS_HASH: ConsensusHash = ConsensusHash([0xff; 20]);

pub const MICROSTACKS_PER_STACKS: u32 = 1_000_000;

pub const POX_SUNSET_START: u64 = 100_000;
pub const POX_SUNSET_END: u64 = POX_SUNSET_START + 400_000;

pub const POX_PREPARE_WINDOW_LENGTH: u32 = 100;
pub const POX_REWARD_CYCLE_LENGTH: u32 = 2100;
pub const BLOCK_INVENTORY_SYNC_CYCLE_SIZE: u32 = 1000;
/// The maximum amount that PoX rewards can be scaled by.
///  That is, if participation is very low, rewards are:
///      POX_MAXIMAL_SCALING x (rewards with 100% participation)
///  Set a 4x, this implies the lower bound of participation for scaling
///   is 25%
pub const POX_MAXIMAL_SCALING: u128 = 4;
/// This is the amount that PoX threshold adjustments are stepped by.
pub const POX_THRESHOLD_STEPS_USTX: u128 = 10_000 * (MICROSTACKS_PER_STACKS as u128);

pub const POX_MAX_NUM_CYCLES: u8 = 12;

pub const POX_V1_MAINNET_EARLY_UNLOCK_HEIGHT: u32 =
    (BITCOIN_MAINNET_STACKS_21_BURN_HEIGHT as u32) + 1;
pub const POX_V1_TESTNET_EARLY_UNLOCK_HEIGHT: u32 =
    (BITCOIN_TESTNET_STACKS_21_BURN_HEIGHT as u32) + 1;

pub const POX_V2_MAINNET_EARLY_UNLOCK_HEIGHT: u32 =
    (BITCOIN_MAINNET_STACKS_22_BURN_HEIGHT as u32) + 1;
pub const POX_V2_TESTNET_EARLY_UNLOCK_HEIGHT: u32 =
    (BITCOIN_TESTNET_STACKS_22_BURN_HEIGHT as u32) + 1;

/// Burn block height at which the ASTRules::PrecheckSize becomes the default behavior on mainnet
pub const AST_RULES_PRECHECK_SIZE: u64 = 752000; // on or about Aug 30 2022

// Block limit for the subnet.
pub const SUBNET_BLOCK_LIMIT: ExecutionCost = ExecutionCost {
    write_length: 15_0_000_000,
    write_count: 5_0_000,
    read_length: 1_000_000_000,
    read_count: 5_0_000,
    // allow much more runtime in helium blocks than mainnet
    runtime: 100_000_000_000,
};

pub const FAULT_DISABLE_MICROBLOCKS_COST_CHECK: &str = "MICROBLOCKS_DISABLE_COST_CHECK";
pub const FAULT_DISABLE_MICROBLOCKS_BYTES_CHECK: &str = "MICROBLOCKS_DISABLE_BYTES_CHECK";

pub fn check_fault_injection(fault_name: &str) -> bool {
    use std::env;

    // only activates if we're testing
    if env::var("BITCOIND_TEST") != Ok("1".to_string()) {
        return false;
    }

    env::var(fault_name) == Ok("1".to_string())
}

lazy_static! {
    pub static ref SUBNET_EPOCHS: [StacksEpoch; 7] = [
        StacksEpoch {
            epoch_id: StacksEpochId::Epoch10,
            start_height: 0,
            end_height: 0,
            block_limit: SUBNET_BLOCK_LIMIT.clone(),
            network_epoch: PEER_VERSION_EPOCH_1_0
        },
        StacksEpoch {
            epoch_id: StacksEpochId::Epoch20,
            start_height: 0,
            end_height: 0,
            block_limit: SUBNET_BLOCK_LIMIT.clone(),
            network_epoch: PEER_VERSION_EPOCH_2_0
        },
        StacksEpoch {
            epoch_id: StacksEpochId::Epoch2_05,
            start_height: 0,
            end_height: 0,
            block_limit: SUBNET_BLOCK_LIMIT.clone(),
            network_epoch: PEER_VERSION_EPOCH_2_05
        },
        StacksEpoch {
            epoch_id: StacksEpochId::Epoch21,
            start_height: 0,
            end_height: 0,
            block_limit: SUBNET_BLOCK_LIMIT.clone(),
            network_epoch: PEER_VERSION_EPOCH_2_1
        },
        StacksEpoch {
            epoch_id: StacksEpochId::Epoch22,
            start_height: 0,
            end_height: 0,
            block_limit: SUBNET_BLOCK_LIMIT.clone(),
            network_epoch: PEER_VERSION_EPOCH_2_2
        },
        StacksEpoch {
            epoch_id: StacksEpochId::Epoch23,
            start_height: 0,
            end_height: 0,
            block_limit: SUBNET_BLOCK_LIMIT.clone(),
            network_epoch: PEER_VERSION_EPOCH_2_3
        },
        StacksEpoch {
            epoch_id: StacksEpochId::Epoch24,
            start_height: 0,
            end_height: STACKS_EPOCH_MAX,
            block_limit: SUBNET_BLOCK_LIMIT.clone(),
            network_epoch: PEER_VERSION_EPOCH_2_4
        },
    ];
}

/// Stacks 2.05 epoch marker.  All block-commits in 2.05 must have a memo bitfield with this value
/// *or greater*.
pub static STACKS_EPOCH_2_05_MARKER: u8 = 0x05;

#[test]
fn test_ord_for_stacks_epoch() {
    let epochs = SUBNET_EPOCHS.clone();
    assert_eq!(epochs[0].cmp(&epochs[1]), Ordering::Less);
    assert_eq!(epochs[1].cmp(&epochs[2]), Ordering::Less);
    assert_eq!(epochs[0].cmp(&epochs[2]), Ordering::Less);
    assert_eq!(epochs[0].cmp(&epochs[0]), Ordering::Equal);
    assert_eq!(epochs[1].cmp(&epochs[1]), Ordering::Equal);
    assert_eq!(epochs[2].cmp(&epochs[2]), Ordering::Equal);
    assert_eq!(epochs[3].cmp(&epochs[3]), Ordering::Equal);
    assert_eq!(epochs[4].cmp(&epochs[4]), Ordering::Equal);
    assert_eq!(epochs[2].cmp(&epochs[0]), Ordering::Greater);
    assert_eq!(epochs[2].cmp(&epochs[1]), Ordering::Greater);
    assert_eq!(epochs[1].cmp(&epochs[0]), Ordering::Greater);
    assert_eq!(epochs[3].cmp(&epochs[0]), Ordering::Greater);
    assert_eq!(epochs[3].cmp(&epochs[1]), Ordering::Greater);
    assert_eq!(epochs[3].cmp(&epochs[2]), Ordering::Greater);
    assert_eq!(epochs[4].cmp(&epochs[0]), Ordering::Greater);
    assert_eq!(epochs[4].cmp(&epochs[1]), Ordering::Greater);
    assert_eq!(epochs[4].cmp(&epochs[2]), Ordering::Greater);
    assert_eq!(epochs[4].cmp(&epochs[3]), Ordering::Greater);
}

#[test]
fn test_ord_for_stacks_epoch_id() {
    assert_eq!(
        StacksEpochId::Epoch10.cmp(&StacksEpochId::Epoch20),
        Ordering::Less
    );
    assert_eq!(
        StacksEpochId::Epoch20.cmp(&StacksEpochId::Epoch2_05),
        Ordering::Less
    );
    assert_eq!(
        StacksEpochId::Epoch10.cmp(&StacksEpochId::Epoch2_05),
        Ordering::Less
    );
    assert_eq!(
        StacksEpochId::Epoch10.cmp(&StacksEpochId::Epoch10),
        Ordering::Equal
    );
    assert_eq!(
        StacksEpochId::Epoch20.cmp(&StacksEpochId::Epoch20),
        Ordering::Equal
    );
    assert_eq!(
        StacksEpochId::Epoch2_05.cmp(&StacksEpochId::Epoch2_05),
        Ordering::Equal
    );
    assert_eq!(
        StacksEpochId::Epoch2_05.cmp(&StacksEpochId::Epoch20),
        Ordering::Greater
    );
    assert_eq!(
        StacksEpochId::Epoch2_05.cmp(&StacksEpochId::Epoch10),
        Ordering::Greater
    );
    assert_eq!(
        StacksEpochId::Epoch20.cmp(&StacksEpochId::Epoch10),
        Ordering::Greater
    );
}

pub trait StacksEpochExtension {
    #[cfg(test)]
    fn unit_test(stacks_epoch_id: StacksEpochId, epoch_2_0_block_height: u64) -> Vec<StacksEpoch>;
    #[cfg(test)]
    fn unit_test_2_05(epoch_2_0_block_height: u64) -> Vec<StacksEpoch>;
    #[cfg(test)]
    fn unit_test_2_05_only(epoch_2_0_block_height: u64) -> Vec<StacksEpoch>;
    #[cfg(test)]
    fn unit_test_pre_2_05(epoch_2_0_block_height: u64) -> Vec<StacksEpoch>;
    #[cfg(test)]
    fn unit_test_2_1(epoch_2_0_block_height: u64) -> Vec<StacksEpoch>;
    #[cfg(test)]
    fn unit_test_2_2(epoch_2_0_block_height: u64) -> Vec<StacksEpoch>;
    #[cfg(test)]
    fn unit_test_2_3(epoch_2_0_block_height: u64) -> Vec<StacksEpoch>;
    #[cfg(test)]
    fn unit_test_2_4(epoch_2_0_block_height: u64) -> Vec<StacksEpoch>;
    #[cfg(test)]
    fn unit_test_2_1_only(epoch_2_0_block_height: u64) -> Vec<StacksEpoch>;
    fn all(
        epoch_2_0_block_height: u64,
        epoch_2_05_block_height: u64,
        epoch_2_1_block_height: u64,
    ) -> Vec<StacksEpoch>;
    fn validate_epochs(epochs: &[StacksEpoch]) -> Vec<StacksEpoch>;
}

impl StacksEpochExtension for StacksEpoch {
    #[cfg(test)]
    fn unit_test_pre_2_05(first_burnchain_height: u64) -> Vec<StacksEpoch> {
        info!(
            "StacksEpoch unit_test first_burn_height = {}",
            first_burnchain_height
        );

        vec![
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch10,
                start_height: 0,
                end_height: first_burnchain_height,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch20,
                start_height: first_burnchain_height,
                end_height: STACKS_EPOCH_MAX,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_2_0,
            },
        ]
    }

    #[cfg(test)]
    fn unit_test_2_05(first_burnchain_height: u64) -> Vec<StacksEpoch> {
        info!(
            "StacksEpoch unit_test first_burn_height = {}",
            first_burnchain_height
        );

        vec![
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch10,
                start_height: 0,
                end_height: first_burnchain_height,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch20,
                start_height: first_burnchain_height,
                end_height: first_burnchain_height + 4,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_2_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch2_05,
                start_height: first_burnchain_height + 4,
                end_height: STACKS_EPOCH_MAX,
                block_limit: ExecutionCost {
                    write_length: 205205,
                    write_count: 205205,
                    read_length: 205205,
                    read_count: 205205,
                    runtime: 205205,
                },
                network_epoch: PEER_VERSION_EPOCH_2_05,
            },
        ]
    }

    #[cfg(test)]
    fn unit_test_2_05_only(first_burnchain_height: u64) -> Vec<StacksEpoch> {
        info!(
            "StacksEpoch unit_test first_burn_height = {}",
            first_burnchain_height
        );

        vec![
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch10,
                start_height: 0,
                end_height: 0,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch20,
                start_height: 0,
                end_height: first_burnchain_height,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_2_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch2_05,
                start_height: first_burnchain_height,
                end_height: STACKS_EPOCH_MAX,
                block_limit: ExecutionCost {
                    write_length: 205205,
                    write_count: 205205,
                    read_length: 205205,
                    read_count: 205205,
                    runtime: 205205,
                },
                network_epoch: PEER_VERSION_EPOCH_2_05,
            },
        ]
    }

    #[cfg(test)]
    fn unit_test_2_1(first_burnchain_height: u64) -> Vec<StacksEpoch> {
        info!(
            "StacksEpoch unit_test first_burn_height = {}",
            first_burnchain_height
        );

        vec![
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch10,
                start_height: 0,
                end_height: first_burnchain_height,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch20,
                start_height: first_burnchain_height,
                end_height: first_burnchain_height + 4,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_2_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch2_05,
                start_height: first_burnchain_height + 4,
                end_height: first_burnchain_height + 8,
                block_limit: ExecutionCost {
                    write_length: 205205,
                    write_count: 205205,
                    read_length: 205205,
                    read_count: 205205,
                    runtime: 205205,
                },
                network_epoch: PEER_VERSION_EPOCH_2_05,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch21,
                start_height: first_burnchain_height + 8,
                end_height: STACKS_EPOCH_MAX,
                block_limit: ExecutionCost {
                    write_length: 210210,
                    write_count: 210210,
                    read_length: 210210,
                    read_count: 210210,
                    runtime: 210210,
                },
                network_epoch: PEER_VERSION_EPOCH_2_1,
            },
        ]
    }

    #[cfg(test)]
    fn unit_test_2_2(first_burnchain_height: u64) -> Vec<StacksEpoch> {
        info!(
            "StacksEpoch unit_test first_burn_height = {}",
            first_burnchain_height
        );

        vec![
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch10,
                start_height: 0,
                end_height: first_burnchain_height,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch20,
                start_height: first_burnchain_height,
                end_height: first_burnchain_height + 4,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_2_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch2_05,
                start_height: first_burnchain_height + 4,
                end_height: first_burnchain_height + 8,
                block_limit: ExecutionCost {
                    write_length: 205205,
                    write_count: 205205,
                    read_length: 205205,
                    read_count: 205205,
                    runtime: 205205,
                },
                network_epoch: PEER_VERSION_EPOCH_2_05,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch21,
                start_height: first_burnchain_height + 8,
                end_height: first_burnchain_height + 12,
                block_limit: ExecutionCost {
                    write_length: 210210,
                    write_count: 210210,
                    read_length: 210210,
                    read_count: 210210,
                    runtime: 210210,
                },
                network_epoch: PEER_VERSION_EPOCH_2_1,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch22,
                start_height: first_burnchain_height + 12,
                end_height: STACKS_EPOCH_MAX,
                block_limit: ExecutionCost {
                    write_length: 210210,
                    write_count: 210210,
                    read_length: 210210,
                    read_count: 210210,
                    runtime: 210210,
                },
                network_epoch: PEER_VERSION_EPOCH_2_2,
            },
        ]
    }

    #[cfg(test)]
    fn unit_test_2_3(first_burnchain_height: u64) -> Vec<StacksEpoch> {
        info!(
            "StacksEpoch unit_test_2_3 first_burn_height = {}",
            first_burnchain_height
        );

        vec![
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch10,
                start_height: 0,
                end_height: first_burnchain_height,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch20,
                start_height: first_burnchain_height,
                end_height: first_burnchain_height + 4,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_2_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch2_05,
                start_height: first_burnchain_height + 4,
                end_height: first_burnchain_height + 8,
                block_limit: ExecutionCost {
                    write_length: 205205,
                    write_count: 205205,
                    read_length: 205205,
                    read_count: 205205,
                    runtime: 205205,
                },
                network_epoch: PEER_VERSION_EPOCH_2_05,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch21,
                start_height: first_burnchain_height + 8,
                end_height: first_burnchain_height + 12,
                block_limit: ExecutionCost {
                    write_length: 210210,
                    write_count: 210210,
                    read_length: 210210,
                    read_count: 210210,
                    runtime: 210210,
                },
                network_epoch: PEER_VERSION_EPOCH_2_1,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch22,
                start_height: first_burnchain_height + 12,
                end_height: first_burnchain_height + 16,
                block_limit: ExecutionCost {
                    write_length: 220220,
                    write_count: 220220,
                    read_length: 220220,
                    read_count: 220220,
                    runtime: 220220,
                },
                network_epoch: PEER_VERSION_EPOCH_2_2,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch23,
                start_height: first_burnchain_height + 16,
                end_height: STACKS_EPOCH_MAX,
                block_limit: ExecutionCost {
                    write_length: 230230,
                    write_count: 230230,
                    read_length: 230230,
                    read_count: 230230,
                    runtime: 230230,
                },
                network_epoch: PEER_VERSION_EPOCH_2_3,
            },
        ]
    }

    #[cfg(test)]
    fn unit_test_2_4(first_burnchain_height: u64) -> Vec<StacksEpoch> {
        info!(
            "StacksEpoch unit_test_2_4 first_burn_height = {}",
            first_burnchain_height
        );

        vec![
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch10,
                start_height: 0,
                end_height: first_burnchain_height,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch20,
                start_height: first_burnchain_height,
                end_height: first_burnchain_height + 4,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_2_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch2_05,
                start_height: first_burnchain_height + 4,
                end_height: first_burnchain_height + 8,
                block_limit: ExecutionCost {
                    write_length: 205205,
                    write_count: 205205,
                    read_length: 205205,
                    read_count: 205205,
                    runtime: 205205,
                },
                network_epoch: PEER_VERSION_EPOCH_2_05,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch21,
                start_height: first_burnchain_height + 8,
                end_height: first_burnchain_height + 12,
                block_limit: ExecutionCost {
                    write_length: 210210,
                    write_count: 210210,
                    read_length: 210210,
                    read_count: 210210,
                    runtime: 210210,
                },
                network_epoch: PEER_VERSION_EPOCH_2_1,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch22,
                start_height: first_burnchain_height + 12,
                end_height: first_burnchain_height + 16,
                block_limit: ExecutionCost {
                    write_length: 210210,
                    write_count: 210210,
                    read_length: 210210,
                    read_count: 210210,
                    runtime: 210210,
                },
                network_epoch: PEER_VERSION_EPOCH_2_2,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch23,
                start_height: first_burnchain_height + 16,
                end_height: first_burnchain_height + 20,
                block_limit: ExecutionCost {
                    write_length: 210210,
                    write_count: 210210,
                    read_length: 210210,
                    read_count: 210210,
                    runtime: 210210,
                },
                network_epoch: PEER_VERSION_EPOCH_2_3,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch24,
                start_height: first_burnchain_height + 20,
                end_height: STACKS_EPOCH_MAX,
                block_limit: ExecutionCost {
                    write_length: 210210,
                    write_count: 210210,
                    read_length: 210210,
                    read_count: 210210,
                    runtime: 210210,
                },
                network_epoch: PEER_VERSION_EPOCH_2_4,
            },
        ]
    }

    #[cfg(test)]
    fn unit_test_2_1_only(first_burnchain_height: u64) -> Vec<StacksEpoch> {
        info!(
            "StacksEpoch unit_test first_burn_height = {}",
            first_burnchain_height
        );

        vec![
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch10,
                start_height: 0,
                end_height: 0,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch20,
                start_height: 0,
                end_height: 0,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_2_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch2_05,
                start_height: 0,
                end_height: first_burnchain_height,
                block_limit: ExecutionCost {
                    write_length: 205205,
                    write_count: 205205,
                    read_length: 205205,
                    read_count: 205205,
                    runtime: 205205,
                },
                network_epoch: PEER_VERSION_EPOCH_2_05,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch21,
                start_height: first_burnchain_height,
                end_height: STACKS_EPOCH_MAX,
                block_limit: ExecutionCost {
                    write_length: 210210,
                    write_count: 210210,
                    read_length: 210210,
                    read_count: 210210,
                    runtime: 210210,
                },
                network_epoch: PEER_VERSION_EPOCH_2_1,
            },
        ]
    }

    #[cfg(test)]
    fn unit_test(stacks_epoch_id: StacksEpochId, first_burnchain_height: u64) -> Vec<StacksEpoch> {
        match stacks_epoch_id {
            StacksEpochId::Epoch10 | StacksEpochId::Epoch20 => {
                StacksEpoch::unit_test_pre_2_05(first_burnchain_height)
            }
            StacksEpochId::Epoch2_05 => StacksEpoch::unit_test_2_05(first_burnchain_height),
            StacksEpochId::Epoch21 => StacksEpoch::unit_test_2_1(first_burnchain_height),
            StacksEpochId::Epoch22 => StacksEpoch::unit_test_2_2(first_burnchain_height),
            StacksEpochId::Epoch23 => StacksEpoch::unit_test_2_3(first_burnchain_height),
            StacksEpochId::Epoch24 => StacksEpoch::unit_test_2_4(first_burnchain_height),
        }
    }

    fn all(
        epoch_2_0_block_height: u64,
        epoch_2_05_block_height: u64,
        epoch_2_1_block_height: u64,
    ) -> Vec<StacksEpoch> {
        vec![
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch10,
                start_height: 0,
                end_height: epoch_2_0_block_height,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch20,
                start_height: epoch_2_0_block_height,
                end_height: epoch_2_05_block_height,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch2_05,
                start_height: epoch_2_05_block_height,
                end_height: epoch_2_1_block_height,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
            StacksEpoch {
                epoch_id: StacksEpochId::Epoch21,
                start_height: epoch_2_1_block_height,
                end_height: STACKS_EPOCH_MAX,
                block_limit: ExecutionCost::max_value(),
                network_epoch: PEER_VERSION_EPOCH_1_0,
            },
        ]
    }

    /// Verify that a list of epochs is well-formed, and if so, return the list of epochs.
    /// Epochs must proceed in order, and must represent contiguous block ranges.
    /// Panic if the list is not well-formed.
    fn validate_epochs(epochs_ref: &[StacksEpoch]) -> Vec<StacksEpoch> {
        // sanity check -- epochs must all be contiguous, each epoch must be unique,
        // and the range of epochs should span the whole non-negative i64 space.
        let mut epochs = epochs_ref.to_vec();
        let mut seen_epochs = HashSet::new();
        epochs.sort();

        let max_epoch = epochs_ref
            .iter()
            .max()
            .expect("FATAL: expect at least one epoch");
        assert!(
            max_epoch.network_epoch as u32 <= PEER_NETWORK_EPOCH,
            "stacks-blockchain static network epoch should be greater than or equal to the max epoch's"
        );

        assert!(
            StacksEpochId::latest() >= max_epoch.epoch_id,
            "StacksEpochId::latest() should be greater than or equal to any epoch defined in the node"
        );

        let mut epoch_end_height = 0;
        for epoch in epochs.iter() {
            assert!(
                epoch.start_height <= epoch.end_height,
                "{} > {} for {:?}",
                epoch.start_height,
                epoch.end_height,
                &epoch.epoch_id
            );

            if epoch_end_height == 0 {
                // first ever epoch must be defined for all of the prior chain history
                assert_eq!(epoch.start_height, 0);
                epoch_end_height = epoch.end_height;
            } else {
                assert_eq!(epoch_end_height, epoch.start_height);
                epoch_end_height = epoch.end_height;
            }
            if seen_epochs.contains(&epoch.epoch_id) {
                panic!("BUG: duplicate epoch");
            }

            seen_epochs.insert(epoch.epoch_id);
        }

        assert_eq!(epoch_end_height, STACKS_EPOCH_MAX);
        epochs
    }
}

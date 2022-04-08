use std::convert::TryInto;
use std::fmt::Display;
use std::fmt::Formatter;

use burnchains::Txid;
use serde::de::Error as DeserError;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serializer;
use util::HexError;
use vm::types::QualifiedContractIdentifier;
use vm::types::Value as ClarityValue;

use crate::types::chainstate::BlockHeaderHash;
use crate::types::chainstate::StacksBlockId;
use crate::vm::types::CharType;
use crate::vm::types::SequenceData;

use super::StacksHyperBlock;
use super::StacksHyperOp;
use super::StacksHyperOpType;
use std::fmt::Write;

/// Parsing struct for the transaction event types of the
/// `stacks-node` events API
#[derive(PartialEq, Clone, Debug, Serialize)]
pub enum TxEventType {
    ContractEvent,
    Other,
}

/// Parsing struct for the contract_event field in transaction events
/// of the `stacks-node` events API
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ContractEvent {
    #[serde(serialize_with = "ser_contract_identifier")]
    #[serde(deserialize_with = "deser_contract_identifier")]
    pub contract_identifier: QualifiedContractIdentifier,
    pub topic: String,
    #[serde(rename = "raw_value", serialize_with = "ser_clarity_value")]
    #[serde(deserialize_with = "deser_clarity_value")]
    pub value: ClarityValue,
}

/// Parsing struct for the transaction events of the `stacks-node`
/// events API
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewBlockTxEvent {
    #[serde(serialize_with = "ser_as_hexstr")]
    #[serde(deserialize_with = "deser_txid")]
    pub txid: Txid,
    pub event_index: usize,
    pub committed: bool,
    #[serde(rename = "type", deserialize_with = "deser_tx_event_type")]
    pub event_type: TxEventType,
    #[serde(default)]
    pub contract_event: Option<ContractEvent>,
}

/// Parsing struct for the new block events of the `stacks-node`
/// events API
#[derive(Clone, Serialize, Deserialize)]
pub struct NewBlock {
    pub block_height: u64,
    pub burn_block_time: u64,
    #[serde(serialize_with = "ser_as_hexstr")]
    #[serde(deserialize_with = "deser_stacks_block_id")]
    pub index_block_hash: StacksBlockId,
    #[serde(serialize_with = "ser_as_hexstr")]
    #[serde(deserialize_with = "deser_stacks_block_id")]
    pub parent_index_block_hash: StacksBlockId,
    pub events: Vec<NewBlockTxEvent>,
}

impl std::fmt::Debug for NewBlock {
    /// Shortened debug string, for logging.
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "NewBlock(hash={:?}, parent_hash={:?}, block_height={}, num_events={})",
            &self.index_block_hash,
            &self.parent_index_block_hash,
            self.block_height,
            self.events.len()
        )
    }
}

/// Method for deserializing a ClarityValue from the `raw_value` field of contract
/// transaction events.
fn deser_clarity_value<'de, D>(deser: D) -> Result<ClarityValue, D::Error>
where
    D: Deserializer<'de>,
{
    let str_val = String::deserialize(deser)?;
    ClarityValue::try_deserialize_hex_untyped(&str_val).map_err(DeserError::custom)
}

/// Convert a slice of u8 to a hex string. TODO: use general fn.
pub fn to_hex(s: &[u8]) -> String {
    let mut r = String::with_capacity(s.len() * 2);
    for b in s.iter() {
        write!(r, "{:02x}", b).unwrap();
    }
    return r;
}

/// Serialize a clarity value to work with `deser_clarity_value`.
fn ser_clarity_value<S>(value: &ClarityValue, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut byte_serialization = Vec::new();
    value
        .serialize_write(&mut byte_serialization)
        .expect("IOError filling byte buffer.");
    let string_value = to_hex(byte_serialization.as_slice());
    s.serialize_str(&string_value)
}

/// Method for deserializing a contract identifier from `contract_identifier` fields in
/// transaction events.
fn deser_contract_identifier<'de, D>(deser: D) -> Result<QualifiedContractIdentifier, D::Error>
where
    D: Deserializer<'de>,
{
    let str_val = String::deserialize(deser)?;
    QualifiedContractIdentifier::parse(&str_val).map_err(DeserError::custom)
}

/// Serialize a contract to work with `deser_contract_identifier`.
fn ser_contract_identifier<S>(
    contract_id: &QualifiedContractIdentifier,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&contract_id.to_string())
}

/// Method for deserializing a `Txid` from transaction events.
fn deser_txid<'de, D>(deser: D) -> Result<Txid, D::Error>
where
    D: Deserializer<'de>,
{
    let str_val = String::deserialize(deser)?;
    match str_val.get(2..) {
        Some(hex) => Txid::from_hex(hex).map_err(DeserError::custom),
        None => Err(DeserError::custom(HexError::BadLength(2))),
    }
}

/// Method for deserializing a `StacksBlockId` from transaction events.
fn deser_stacks_block_id<'de, D>(deser: D) -> Result<StacksBlockId, D::Error>
where
    D: Deserializer<'de>,
{
    let str_val = String::deserialize(deser)?;
    match str_val.get(2..) {
        Some(hex) => StacksBlockId::from_hex(hex).map_err(DeserError::custom),
        None => Err(DeserError::custom(HexError::BadLength(2))),
    }
}

// Only works if Display implementation uses a hex string, which Txid and StacksBlockId do
fn ser_as_hexstr<T: Display, S: Serializer>(input: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("0x{}", &input.to_string()))
}

/// Method for deserializing a `TxEventType` from transaction events.
/// This module is currently only interested in `contract_event` types,
/// so all other events are parsed as `Other`.
fn deser_tx_event_type<'de, D>(deser: D) -> Result<TxEventType, D::Error>
where
    D: Deserializer<'de>,
{
    let str_val = String::deserialize(deser)?;
    match str_val.as_str() {
        "contract_event" => Ok(TxEventType::ContractEvent),
        _ => Ok(TxEventType::Other),
    }
}

impl StacksHyperOp {
    /// This method tries to parse a `StacksHyperOp` from a Clarity value: this should be a tuple
    /// emitted from the hyperchain contract in a statement like:
    /// `(print { event: "block-commit", block-commit: 0x123... })`
    ///
    /// If the provided value does not match that tuple, this method will return an error.
    pub fn try_from_clar_value(
        v: ClarityValue,
        txid: Txid,
        event_index: u32,
        in_block: &StacksBlockId,
    ) -> Result<Self, String> {
        let tuple = if let ClarityValue::Tuple(tuple) = v {
            Ok(tuple)
        } else {
            Err("Expected Clarity type to be tuple")
        }?;

        let event = tuple
            .get("event")
            .map_err(|_| "No 'event' field in Clarity tuple")?;
        let event = if let ClarityValue::Sequence(SequenceData::String(clar_str)) = event {
            Ok(clar_str.to_string())
        } else {
            Err("Expected 'event' type to be string")
        }?;

        match event.as_str() {
            "\"block-commit\"" => {
                let block_commit = tuple
                    .get("block-commit")
                    .map_err(|_| "No 'block-commit' field in Clarity tuple")?;
                let block_commit =
                    if let ClarityValue::Sequence(SequenceData::Buffer(buff_data)) = block_commit {
                        if u32::from(buff_data.len()) != 32 {
                            Err(format!(
                                "Expected 'block-commit' type to be length 32, found {}",
                                buff_data.len()
                            ))
                        } else {
                            let mut buff = [0; 32];
                            buff.copy_from_slice(&buff_data.data);
                            Ok(buff)
                        }
                    } else {
                        Err("Expected 'block-commit' type to be buffer".into())
                    }?;

                Ok(Self {
                    txid,
                    event_index,
                    in_block: in_block.clone(),
                    opcode: 0,
                    event: StacksHyperOpType::BlockCommit {
                        subnet_block_hash: BlockHeaderHash(block_commit),
                    },
                })
            }
            event_type => Err(format!("Unexpected 'event' string: {}", event_type)),
        }
    }
}

impl StacksHyperBlock {
    /// Process a `NewBlock` event from a layer-1 Stacks node, filter
    /// for the transaction events in the block that are relevant to
    /// the hyperchain and parse out the `StacksHyperOp`s from the
    /// block, producing a `StacksHyperBlock` struct.
    pub fn from_new_block_event(
        subnets_contract: &QualifiedContractIdentifier,
        b: NewBlock,
    ) -> Self {
        let NewBlock {
            events,
            index_block_hash,
            parent_index_block_hash,
            block_height,
            ..
        } = b;

        let ops = events
            .into_iter()
            .filter_map(|e| {
                if !e.committed {
                    None
                } else if e.event_type != TxEventType::ContractEvent {
                    None
                } else {
                    let NewBlockTxEvent {
                        txid,
                        contract_event,
                        event_index,
                        ..
                    } = e;

                    let event_index: u32 = match event_index.try_into() {
                        Ok(x) => Some(x),
                        Err(_e) => {
                            warn!(
                                "StacksHyperBlock skipped event because event_index was not a u32"
                            );
                            None
                        }
                    }?;

                    if let Some(contract_event) = contract_event {
                        if &contract_event.contract_identifier != subnets_contract {
                            None
                        } else {
                            match StacksHyperOp::try_from_clar_value(
                                contract_event.value,
                                txid,
                                event_index,
                                &index_block_hash,
                            ) {
                                Ok(x) => Some(x),
                                Err(e) => {
                                    info!(
                                        "StacksHyperBlock parser skipped event because of {:?}",
                                        e
                                    );
                                    None
                                }
                            }
                        }
                    } else {
                        None
                    }
                }
            })
            .collect();

        Self {
            current_block: index_block_hash,
            parent_block: parent_index_block_hash,
            block_height,
            ops,
        }
    }
}

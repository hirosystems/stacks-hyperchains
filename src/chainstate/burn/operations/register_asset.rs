use crate::burnchains::{Burnchain, StacksSubnetOp, StacksSubnetOpType};
use crate::chainstate::burn::db::sortdb::SortitionHandleTx;
use crate::chainstate::burn::operations::Error as op_error;
use crate::chainstate::burn::operations::RegisterAssetOp;
use clarity::types::chainstate::BurnchainHeaderHash;
use std::convert::TryFrom;

impl TryFrom<&StacksSubnetOp> for RegisterAssetOp {
    type Error = op_error;

    fn try_from(value: &StacksSubnetOp) -> Result<Self, Self::Error> {
        if let StacksSubnetOpType::RegisterAsset {
            ref l1_contract_id,
            ref l2_contract_id,
            ref asset_type,
        } = value.event
        {
            Ok(RegisterAssetOp {
                txid: value.txid.clone(),
                // use the StacksBlockId in the L1 event as the burnchain header hash
                burn_header_hash: BurnchainHeaderHash(value.in_block.0.clone()),
                asset_type: asset_type.clone(),
                l1_contract_id: l1_contract_id.clone(),
                l2_contract_id: l2_contract_id.clone(),
            })
        } else {
            Err(op_error::InvalidInput)
        }
    }
}

impl RegisterAssetOp {
    pub fn check(
        &self,
        _burnchain: &Burnchain,
        _tx: &mut SortitionHandleTx,
    ) -> Result<(), op_error> {
        // good to go!
        Ok(())
    }

    #[cfg(test)]
    pub fn set_burn_height(&mut self, _height: u64) {}
}
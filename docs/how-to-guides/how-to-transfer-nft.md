---
title: How to Transfer NFT
---

## Transfer an NFT

On the hyperchains, the NFT should belong to the principal that sent the deposit transaction, `USER_ADDR`. 
This principal can now transfer the NFT within the hyperchain. The principal `USER_ADDR` will now make a 
transaction to transfer the NFT to `ALT_USER_ADDR` using the command shown below.

```
node ./transfer_nft.js 2
```
Next, use the `grep` command to retrieve the transaction ID of the transfer transaction like the example below.

```
docker logs hyperchain-node.nft-use-case.devnet 2>&1 | grep "6acc2c756ddaed2c4cfb7351dd5930aa93ba923504be85e47db056c99a7e81aa"
```

In the response, look for text similar to the following line:
```
Jul 19 13:04:43.177993 INFO Tx successfully processed. (ThreadId(9), src/chainstate/stacks/miner.rs:235), event_name: transaction_result, tx_id: 74949992488b2519e2d8408169f242c86a6cdacd927638bd4604b3b8d48ea187, event_type: success, payload: ContractCall
```
## Mint an NFT on the hyperchain

As an added step, you may try minting an NFT on the hyperchain. This requires calling the `gift-nft` function in the 
contract `simple-nft-l2`. You can tweak the `transfer_nft.js` file to make this call. 
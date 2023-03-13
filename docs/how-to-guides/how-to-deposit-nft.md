---
title: How to Deposit NFT
---

## Calling the deposit NFT function

In order to call the deposit NFT function in the hyperchains interface contract, the principal `USER_ADDR` must be called using the command shown below.

```
node ./deposit_nft.js 3
```

## Verifying the transaction and successful deposit

Verify that the transaction is acknowledged in the next few blocks of the L1 chain. After the transaction is confirmed in an anchored block on the L1 (this means it is included in an explicitlynumbered block in the Clarinet console), you may also want to verify the asset was successfully deposited on the hyperchain by grepping for the deposit transaction ID.

To `grep` the deposit transaction ID, use the following command:
```
docker logs hyperchain-node.nft-use-case.devnet 2>&1 | grep "8d042c14323cfd9d31e121cc48c2c641a8db01dce19a0f6dd531eb33689dff44"
```
In the response, look for a line similr to the example shown below.

```
Jul 19 12:51:02.396923 INFO ACCEPTED burnchain operation (ThreadId(8), src/chainstate/burn/db/sortdb.rs:3042), op: deposit_nft, l1_stacks_block_id: 8b5c4eb05afae6daaafdbd59aecaade6da1a8eab5eb1041062c6381cd7104b75, txid: 67cfd6220ed01c3aca3912c8f1ff55d374e5b3acadb3b995836ae913108e0514, l1_contract_id: ST2CY5V39NHDPWSXMW9QDT3HC3GD6Q6XX4CFRK9AG.simple-nft-l1, hc_contract_id: ST2CY5V39NHDPWSXMW9QDT3HC3GD6Q6XX4CFRK9AG.simple-nft-l2, hc_function_name: hyperchain-deposit-nft-token, id: 5, sender: ST2CY5V39NHDPWSXMW9QDT3HC3GD6Q6XX4CFRK9AG
```
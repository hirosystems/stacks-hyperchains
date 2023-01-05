## 1. Start the subnet miner

```bash
subnet-node start --config=$STACKS_SUBNET_PATH/contrib/conf/subnet-l2.toml 2>&1 | tee -i /tmp/stacks-subnet.log
```

## 2. Start a local Stacks network

```bash
stacks-node start --config=$STACKS_SUBNET_PATH/contrib/conf/stacks-l1-mocknet-local.toml 2>&1 | tee -i /tmp/stacks-mocknet.log
```

## 3. Launch the contract

Collect the contracts:

```bash
mkdir my-subnet/
mkdir my-subnet/contracts
cp stacks-subnets/core-contracts/contracts/subnet.clar my-subnet/contracts/
cp stacks-subnets/core-contracts/contracts/helper/ft-trait-standard.clar my-subnet/contracts/
cp stacks-subnets/core-contracts/contracts/helper/nft-trait-standard.clar my-subnet/contracts/
```

Set the miners list to contain the address generated in Step 1:

```bash
sed -ie "s#^(define-data-var miner (optional principal) none)#(define-data-var miner (optional principal) (some \'ST2GE6HSXT81X9X3ATQ14WPT49X915R8X7FVERMBP))#" my-subnet/contracts/subnet.clar
```

Make the transactions -- you will need to set the private key of the contract publisher as an env var:

```bash
export CONTRACT_PUBLISH_KEY=0916e2eb04b5702e0e946081829cee67d3bb76e1792af506646843db9252ff4101
```

This is the private key from the first step.

```bash
mkdir my-subnet/scripts
cp stacks-subnets/contrib/scripts/* my-subnet/scripts/
cd my-subnet/scripts/
npm i @stacks/network
npm i @stacks/transactions
mkdir ../transactions/
node ./publish_tx.js ft-trait-standard ../contracts/ft-trait-standard.clar 0 > ../transactions/ft-publish.hex
node ./publish_tx.js nft-trait-standard ../contracts/nft-trait-standard.clar 1 > ../transactions/nft-publish.hex
node ./publish_tx.js subnet ../contracts/subnet.clar 2 > ../transactions/subnet-publish.hex
```

Submit the transactions:

```bash
for I in `ls ../transactions/`; do node ./broadcast_tx.js "../transactions/$I" http://localhost:20443; done
```

## 4. Deposit some funds to L2

```js
const network = require('@stacks/network');
const transactions = require('@stacks/transactions');
const senderKey = "aaf57b4730f713cf942bc63f0801c4a62abe5a6ac8e3da10389f9ca3420b0dc701"
const layer1 = new network.StacksTestnet();
layer1.coreApiUrl = "http://localhost:20443";

const depositTransaction = await transactions.makeContractCall({
   senderKey, network: layer1, anchorMode: transactions.AnchorMode.Any,
   nonce: 0,
   contractAddress: "ST2GE6HSXT81X9X3ATQ14WPT49X915R8X7FVERMBP",
   contractName: "subnet",
   functionName: "deposit-stx",
   functionArgs: [ transactions.uintCV(100000000000),
                   transactions.standardPrincipalCV("ST18F1AHKW194BWQ3CEFDPWVRARA79RBGFEWSDQR8")],
   fee: 10000,
   postConditionMode: transactions.PostConditionMode.Allow,
});

const depositTxid = await transactions.broadcastTransaction(depositTransaction, layer1);
```

Check that you received the funds in L2:

```js
const layer2 = new network.StacksTestnet();
layer2.coreApiUrl = "http://localhost:19443";
await fetch(layer2.getAccountApiUrl("ST18F1AHKW194BWQ3CEFDPWVRARA79RBGFEWSDQR8")).then(x => x.json()).then(x => parseInt(x.balance));
```

## 5. Submit an L2 transaction


```js
const codeBody = "(define-public (stx-withdraw (amount uint)) (stx-withdraw? amount tx-sender))";
const contractName = "withdraw-helper";
const deployWithdrawal = await transactions.makeContractDeploy({
    codeBody, contractName, senderKey, network: layer2,
    anchorMode: transactions.AnchorMode.Any, nonce: 0,
    fee: 10000,
  });
  
await transactions.broadcastTransaction(deployWithdrawal, layer2);
```


## 6. Withdraw

Perform the withdrawal on layer-2

```js
const withdrawTransaction = await transactions.makeContractCall({
   senderKey, network: layer2, anchorMode: transactions.AnchorMode.Any,
   nonce: 1,
   contractAddress: "ST18F1AHKW194BWQ3CEFDPWVRARA79RBGFEWSDQR8",
   contractName: "withdraw-helper",
   functionName: "stx-withdraw",
   functionArgs: [ transactions.uintCV(50000) ],
   fee: 10000,
   postConditionMode: transactions.PostConditionMode.Allow,
});

await transactions.broadcastTransaction(withdrawTransaction, layer2);
```

Find the withdrawal event in our log:

```bash
cat /tmp/stacks-subnet.log | grep "Parsed L2"
```

Perform the withdrawal on layer-1

```js
let withdrawUrl = "http://localhost:19443/v2/withdrawal/stx/14/ST18F1AHKW194BWQ3CEFDPWVRARA79RBGFEWSDQR8/0/50000";
let json_merkle_entry = await fetch(withdrawUrl).then(x => x.json())
let cv_merkle_entry = {
    withdrawal_leaf_hash: transactions.deserializeCV(json_merkle_entry.withdrawal_leaf_hash),
    withdrawal_root: transactions.deserializeCV(json_merkle_entry.withdrawal_root),
    sibling_hashes: transactions.deserializeCV(json_merkle_entry.sibling_hashes),
};

const layer1WithdrawTransaction = await transactions.makeContractCall({
   senderKey, network: layer1, anchorMode: transactions.AnchorMode.Any,
   nonce: 1,
   contractAddress: "ST2GE6HSXT81X9X3ATQ14WPT49X915R8X7FVERMBP",
   contractName: "subnet",
   functionName: "withdraw-stx",
   functionArgs: [ transactions.uintCV(50000),
                   transactions.standardPrincipalCV("ST18F1AHKW194BWQ3CEFDPWVRARA79RBGFEWSDQR8"),
                   cv_merkle_entry.withdrawal_root,
                   cv_merkle_entry.withdrawal_leaf_hash,
                   cv_merkle_entry.sibling_hashes ],
   fee: 5000,
   postConditionMode: transactions.PostConditionMode.Allow,
});

await transactions.broadcastTransaction(layer1WithdrawTransaction, layer1)
;
```

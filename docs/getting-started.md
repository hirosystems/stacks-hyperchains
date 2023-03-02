---
# The default id is the same as the one being defined below. so not needed
title: Getting Started
---

# Getting Started

Subnets can be built using any of the following methods:

- Build with Clarinet
- Build with Testnet
- Test Locally

> **_NOTE:_**
>
> Subnets was earlier referred as Hyperchains. While the process of updating the content is ongoing, there may still be some references to hyperchains instead of subnets.

## Build with Clarinet

Make sure you have `clarinet` installed locally, and the clarinet version is at version 0.33.0 or above.
If you do not already have clarinet installed, please refer to the clarinet installation instructions [here](https://github.com/hirosystems/clarinet) for installation procedures.

### Create a new clarinet project

To create a new clarinet project, run the command shown below.

```
clarinet new nft-use-case 
```

This command creates a new directory with a clarinet project already initialized.

### Copy contract files and scripts

Next, copy the contract files and scripts over from the `stacks-hyperchains` repository into the `nft-use-case` directory. If you do not already have the `stacks-hyperchains` repository, simply [clone the repository](https://github.com/hirosystems/stacks-hyperchains).

To clone the `stacks-hyperchains` repository, use the following command:

```
git clone https://github.com/hirosystems/stacks-hyperchains.git
```

### Set HYPERCHAIN_PATH environment variable

After copying contract files and scripts, set the environment variable `HYPERCHAIN_PATH` to the location of the `stacks-hyperchains` repository on your computer using the command below.

```
export HYPERCHAIN_PATH=<YOUR_PATH_HERE>
```

### Copy files from the `stacks-hyperchains` repository

Now, copy files from the `stacks-hyperchains` repository. These files are contracts that define the layer-1
and layer-2 Clarity traits for NFTs and fungible tokens, implement an NFT in layer-1 and layer-2, and some NodeJS scripts for helping to deploy the contracts. Enter the commands shown below.

```
mkdir nft-use-case/contracts-l2
mkdir nft-use-case/scripts
cp $HYPERCHAIN_PATH/core-contracts/contracts/helper/simple-nft.clar nft-use-case/contracts/
cp $HYPERCHAIN_PATH/core-contracts/contracts/helper/trait-standards.clar nft-use-case/contracts/
cp $HYPERCHAIN_PATH/core-contracts/contracts/helper/simple-nft-l2.clar nft-use-case/contracts-l2/
cp $HYPERCHAIN_PATH/core-contracts/contracts/helper/trait-standards.clar nft-use-case/contracts-l2/
cp $HYPERCHAIN_PATH/contrib/scripts/nft-use-case/* nft-use-case/scripts/
cd nft-use-case/scripts
```

### Install node and NodeJs libraries

To use the scripts in this demo, you need to install some NodeJS libraries. Before running the following instructions, make sure you have installed [node](https://nodejs.org/en/).

```
npm install
```

The `Devnet.toml` file in the `nft-use-case` directory is responsible for configuring the `clarinet integrate` 
local network. Make the following change in `settings/Devnet.toml` to enable the hyperchain:

```
[devnet]
...
enable_hyperchain_node = true
```

You can now spin up a hyperchain node; however, before calling the hyperchain, make sure you have a working Docker installation running locally by entering the command below.

```
clarinet integrate
```

### Set environment variables

Before publishing any transactions, you will need to set up some environment variables. These environment variables contain the address and private key of the hyperchain miner, two user addresses and private keys, and the RPC URL which we can query for hyperchain state.

Open a separate terminal window, navigate to the directory `nft-use-case/scripts`, and enter the following command:

```
export AUTH_HC_MINER_ADDR=ST3AM1A56AK2C1XAFJ4115ZSV26EB49BVQ10MGCS0
export AUTH_HC_MINER_KEY=7036b29cb5e235e5fd9b09ae3e8eec4404e44906814d5d01cbca968a60ed4bfb01

export USER_ADDR=ST2NEB84ASENDXKYGJPQW86YXQCEFEX2ZQPG87ND
export USER_KEY=f9d7206a47f14d2870c163ebab4bf3e70d18f5d14ce1031f3902fbbc894fe4c701

export ALT_USER_ADDR=ST2REHHS5J3CERCRBEPMGH7921Q6PYKAADT7JP2VB
export ALT_USER_KEY=3eccc5dac8056590432db6a35d52b9896876a3d5cbdea53b72400bc9c2099fe801

export HYPERCHAIN_URL="http://localhost:30443"
```

## Build with Testnet

### 1. Getting Testnet STX

To get the testnet STX, enter the following command:

```bash
./target/debug/blockstack-cli generate-sk --testnet
{ 
  "publicKey": "02c3c7ab279c5637ea5f024f8036c5218b6d1e71518adba0693c3dcc7bead92305",
  "stacksAddress": "STFTX3F4XCY7RS5VRHXP2SED0WC0YRKNWTNXD74P"
}
```

```bash
curl -X POST "https://stacks-node-api.testnet.stacks.co/extended/v1/faucets/stx?address=STFTX3F4XCY7RS5VRHXP2SED0WC0YRKNWTNXD74P&stacking=true"
```

### 2. Spin up testnet `stacks-node`

To spin up the `stacks-node` testnet, enter the command below.

```toml
[node]
working_dir = "/var/testnet-stacks-node"
rpc_bind = "127.0.0.1:20443"
p2p_bind = "0.0.0.0:20444"
bootstrap_node = "047435c194e9b01b3d7f7a2802d6684a3af68d05bbf4ec8f17021980d777691f1d51651f7f1d566532c804da506c117bbf79ad62eea81213ba58f8808b4d9504ad@testnet.stacks.co:20444"

[burnchain]
chain = "bitcoin"
mode = "xenon"
peer_host = "bitcoind.testnet.stacks.co"
username = "blockstack"
password = "blockstacksystem"
rpc_port = 18332
peer_port = 18333

# Used for sending events to a local stacks-blockchain-api service
# [[events_observer]]
# endpoint = "localhost:3700"
# retry_count = 255
# events_keys = ["*"]

[[ustx_balance]]
address = "ST2QKZ4FKHAH1NQKYKYAYZPY440FEPK7GZ1R5HBP2"
amount = 10000000000000000

[[ustx_balance]]
address = "ST319CF5WV77KYR1H3GT0GZ7B8Q4AQPY42ETP1VPF"
amount = 10000000000000000

[[ustx_balance]]
address = "ST221Z6TDTC5E0BYR2V624Q2ST6R0Q71T78WTAX6H"
amount = 10000000000000000

[[ustx_balance]]
address = "ST2TFVBMRPS5SSNP98DQKQ5JNB2B6NZM91C4K3P7B"
amount = 10000000000000000
```


```bash
./target/release/stacks-node start --config=/var/devel/stacks-hyperchains/contrib/conf/stacks-l1-testnet.toml 2>&1 | tee -i /tmp/stacks-testnet-0426-1055.log
```

**_NOTE:_**

You can use an existing testnet chain state if one is available.

In this example, `cp /root/stacks-node/` was used on one of the Hiro deployed xenon followers. Although the first attempt
failed, a subsequent attempt caused the node to boot correctly.

### 3. Launch the contract

Collect the contracts by using the command below:

```bash
mkdir my-hyperchain/
mkdir my-hyperchain/contracts
cp stacks-hyperchains/core-contracts/contracts/hyperchains.clar my-hyperchain/contracts/
cp stacks-hyperchains/core-contracts/contracts/helper/trait-standards.clar my-hyperchain/contracts/
```

You then need to set the miners list to contain the address generated in Step 1 by entering the command:

```bash
sed -ie "s#^(define-constant miners.*#(define-constant miners (list \'STFTX3F4XCY7RS5VRHXP2SED0WC0YRKNWTNXD74P))#" my-hyperchain/contracts/hyperchains.clar
```

Next, make the transactions -- you will need to set the private key of the contract publisher as an env var by using this command:

```bash
export CONTRACT_PUBLISH_KEY=<PRIVATEKEY>
```

This is the private key from the first step.

```bash
mkdir my-hyperchain/scripts
cp stacks-hyperchains/contrib/scripts/* my-hyperchain/scripts/
cd my-hyperchain/scripts/
npm i @stacks/network
npm i @stacks/transactions
mkdir ../transactions/
node ./publish_tx.js trait-standards ../contracts/trait-standards.clar 0 > ../transactions/trait-publish.hex
node ./publish_tx.js hc-alpha ../contracts/hyperchains.clar 1 > ../transactions/hc-publish.hex
```

Submit the transactions:

```bash
$ node ./broadcast_tx.js ../transactions/trait-publish.hex
{
  txid: '93cae889b9382c512e55715e5357b388734c0448643e2cc35d2a1aab90dcf61a'
}

$ node ./broadcast_tx.js ../transactions/hc-publish.hex
{
  txid: '8c457091916a7f57b487162e0692c2cd28e71dd0b2dc9a9dfad73f93babe1dfd'
}
```

### 4. Configure the HC miner

Create a `toml` configuration for the hyperchains miner and set the `contract_identifier` to the contract published in
Steps 3 (e.g., `STFTX3F4XCY7RS5VRHXP2SED0WC0YRKNWTNXD74P.hc-alpha`).

```toml
[node]
working_dir = "/var/my-hyperchain/hc-alpha"
rpc_bind = "127.0.0.1:80443"
p2p_bind = "127.0.0.1:80444"
mining_key = "<FILL HERE>"
miner = true
wait_time_for_microblocks = 50_000

[miner]
first_attempt_time_ms = 60_000
subsequent_attempt_time_ms = 60_000
microblock_attempt_time_ms = 30_000

[burnchain]
chain = "stacks_layer_1"
mode = "xenon"
first_burn_header_height = 46_721
first_burn_header_hash = "9ba2f357115308fb1c503715f3a1b0cb3e8fdbe6baea7e7634635affdf675501"
contract_identifier = "<CONTRACT_NAME_HERE>"
peer_host = "127.0.0.1"
rpc_port = 20443
peer_port = 20444
rpc_ssl = false

[[ustx_balance]]
address = "STFTX3F4XCY7RS5VRHXP2SED0WC0YRKNWTNXD74P"
amount = 10000000000000000
```

You should then add to L1 node configuration by using this command:

```
[[events_observer]]
endpoint = "localhost:50303"
retry_count = 255
events_keys = ["*"]
```

### 5. Start the nodes

The `hyperchain-node` must be started before the `stacks-node`. To start `hyperchain-node`, enter the following command:

```bash
./target/release/hyperchain-node start --config=/var/my-hyperchain/configs/hc-miner.toml 2>&1 | tee /var/my-hyperchain/hc-miner.log
```

The `stacks-node` must be started from a state _before_ the
`first_burn_header_height` and `first_burn_header_hash` configured
in the hyperchain node's TOML.

```bash
./target/release/stacks-node start --config=/var/stacks-hyperchains/contrib/conf/stacks-l1-testnet.toml 2>&1 | tee -i /tmp/stacks-testnet.log
```

## Test Locally

To test subnets locally, follow the setps listed below.

### 1. Start the hyperchain miner

```bash
hyperchain-node start --config=$STACKS_HYPERCHAINS_PATH/contrib/conf/hyperchain-l2.toml 2>&1 | tee -i /tmp/stacks-hc.log
```

### 2. Start a local Stacks network

```bash
stacks-node start --config=$STACKS_HYPERCHAINS_PATH/contrib/conf/stacks-l1-mocknet-local.toml 2>&1 | tee -i /tmp/stacks-mocknet.log
```

### 3. Launch the contract

Collect the contracts:

```bash
mkdir my-hyperchain/
mkdir my-hyperchain/contracts
cp stacks-hyperchains/core-contracts/contracts/hyperchains.clar my-hyperchain/contracts/
cp stacks-hyperchains/core-contracts/contracts/helper/ft-trait-standard.clar my-hyperchain/contracts/
cp stacks-hyperchains/core-contracts/contracts/helper/nft-trait-standard.clar my-hyperchain/contracts/
```

Set the miners list to contain the address generated in Step 1:

```bash
sed -ie "s#^(define-data-var miner (optional principal) none)#(define-data-var miner (optional principal) (some \'ST2GE6HSXT81X9X3ATQ14WPT49X915R8X7FVERMBP))#" my-hyperchain/contracts/hyperchains.clar
```

Make the transactions -- you will need to set the private key of the contract publisher as an env variable using the following command:

```bash
export CONTRACT_PUBLISH_KEY=0916e2eb04b5702e0e946081829cee67d3bb76e1792af506646843db9252ff4101
```

This is the private key from the first step.

```bash
mkdir my-hyperchain/scripts
cp stacks-hyperchains/contrib/scripts/* my-hyperchain/scripts/
cd my-hyperchain/scripts/
npm i @stacks/network
npm i @stacks/transactions
mkdir ../transactions/
node ./publish_tx.js ft-trait-standard ../contracts/ft-trait-standard.clar 0 > ../transactions/ft-publish.hex
node ./publish_tx.js nft-trait-standard ../contracts/nft-trait-standard.clar 1 > ../transactions/nft-publish.hex
node ./publish_tx.js hyperchain ../contracts/hyperchains.clar 2 > ../transactions/hc-publish.hex
```

Submit the transactions:

```bash
for I in `ls ../transactions/`; do node ./broadcast_tx.js "../transactions/$I" http://localhost:20443; done
```

### 4. Deposit some funds to L2

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
   contractName: "hyperchain",
   functionName: "deposit-stx",
   functionArgs: [ transactions.uintCV(100000000000),
                   transactions.standardPrincipalCV("ST18F1AHKW194BWQ3CEFDPWVRARA79RBGFEWSDQR8")],
   fee: 10000,
   postConditionMode: transactions.PostConditionMode.Allow,
});

const depositTxid = await transactions.broadcastTransaction(depositTransaction, layer1);
```

Verify you received the funds in L2:

```js
const layer2 = new network.StacksTestnet();
layer2.coreApiUrl = "http://localhost:19443";
await fetch(layer2.getAccountApiUrl("ST18F1AHKW194BWQ3CEFDPWVRARA79RBGFEWSDQR8")).then(x => x.json()).then(x => parseInt(x.balance));
```

### 5. Submit an L2 transaction


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


### 6. Withdraw

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

Find the withdrawal event in the log:

```bash
cat /tmp/stacks-hc.log | grep "Parsed L2"
```

Perform the withdrawal on layer-1:

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
   contractName: "hyperchain",
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
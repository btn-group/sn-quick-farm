<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://github.com/btn-group">
    <img src="images/logo.png" alt="Logo" height="80">
  </a>

  <h3 align="center">API Key Manager by btn.group</h3>
</div>

<!-- TABLE OF CONTENTS -->
<details>
  <summary>Table of Contents</summary>
  <ol>
    <li>
      <a href="#about-the-project">About The Project</a>
      <ul>
        <li><a href="#built-with">Built With</a></li>
      </ul>
    </li>
    <li>
      <a href="#getting-started">Getting Started</a>
      <ul>
        <li><a href="#prerequisites">Prerequisites</a></li>
        <li><a href="#setting-up-locally">Setting up locally</a></li>
      </ul>
    </li>
    <li><a href="#usage">Usage</a>
      <ul>
        <li><a href="#init">Init</a></li>
        <li><a href="#queries">Queries</a></li>
        <li><a href="#handle-functions">Handle functions</a></li>
      </ul>
    </li>
  </ol>
</details>

<!-- ABOUT THE PROJECT -->
## About The Project

This is a smart contract that allows users to set an api key on a smart contract which an admin can then read.

<p align="right">(<a href="#top">back to top</a>)</p>

### Built With

* [Cargo](https://doc.rust-lang.org/cargo/)
* [Rust](https://www.rust-lang.org/)
* [secret-toolkit](https://github.com/scrtlabs/secret-toolkit)

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- GETTING STARTED -->
## Getting Started

To get a local copy up and running follow these simple example steps.

### Prerequisites

* Download and install secretcli: https://docs.scrt.network/cli/install-cli.html
* Setup developer blockchain and Docker: https://docs.scrt.network/dev/developing-secret-contracts.html#personal-secret-network-for-secret-contract-development

### Setting up locally

Do this on the command line (terminal etc) in this folder.

1. Run chain locally and make sure to note your wallet addresses.

```sh
docker run -it --rm -p 26657:26657 -p 26656:26656 -p 1337:1337 -v $(pwd):/root/code --name secretdev enigmampc/secret-network-sw-dev
```

2. Access container via separate terminal window

```sh
docker exec -it secretdev /bin/bash

# cd into code folder
cd code
```

3. Store contract

```sh
# Store contracts required for test
secretcli tx compute store snip-20-reference-impl.wasm.gz --from a --gas 3000000 -y --keyring-backend test
secretcli tx compute store sn-api-key-manager.wasm.gz --from a --gas 3000000 -y --keyring-backend test

# Get the contract's id
secretcli query compute list-code
```

4. Initiate SNIP-20 contracts and set viewing keys (make sure you substitute the wallet and contract addressses as required)

```sh
# Init BUTTON (BUTT)
CODE_ID=1
INIT='{ "name": "BUTTON", "symbol": "BUTT", "decimals": 6, "initial_balances": [{ "address": "secret1qfup0cj2kcfwzdc3j70790py67g99azy5xk0rg", "amount": "2000000000000000000" }, { "address": "secret1kfpwnj7kn5m96m73m6xemlsyc68e4xtec0tgqg", "amount": "2000000000000000000" }], "prng_seed": "RG9UaGVSaWdodFRoaW5nLg==", "config": { "public_total_supply": true, "enable_deposit": false, "enable_redeem": false, "enable_mint": false, "enable_burn": false } }'
secretcli tx compute instantiate $CODE_ID "$INIT" --from a --label "BUTT" -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt

# Set viewing key for BUTT
secretcli tx compute execute secret1qxxlalvsdjd07p07y3rc5fu6ll8k4tme6e2scc '{"set_viewing_key": {"key": "DoTheRightThing..", "padding": "BUTT2022."}}' --from a -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
secretcli tx compute execute secret1qxxlalvsdjd07p07y3rc5fu6ll8k4tme6e2scc '{"set_viewing_key": {"key": "DoTheRightThing.", "padding": "BUTT2022."}}' --from b -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt

5. Initialize SN API Key Manager

```sh
# Init SN Limit Orders
CODE_ID=2
INIT='{ "butt": {"address": "secret1qxxlalvsdjd07p07y3rc5fu6ll8k4tme6e2scc", "contract_hash": "35F5DB2BC5CD56815D10C7A567D6827BECCB8EAF45BC3FA016930C4A8209EA69"} }'
secretcli tx compute instantiate $CODE_ID "$INIT" --from a --label "SN API Key Manager | btn.group" -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
```

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- USAGE EXAMPLES -->
## Usage

You can decode and encode the msg used in the send functions below via https://www.base64encode.org/

### Queries

1. Query API Key as user

``` sh
secretcli query compute query secret1hqrdl6wstt8qzshwc6mrumpjk9338k0lpsefm3 '{"api_key": { "address": "secret1qfup0cj2kcfwzdc3j70790py67g99azy5xk0rg", "butt_viewing_key": "DoTheRightThing.", "admin": false }}'
```

2. Query API key as admin

``` sh
secretcli query compute query secret1hqrdl6wstt8qzshwc6mrumpjk9338k0lpsefm3 '{"api_key": { "address": "secret1kfpwnj7kn5m96m73m6xemlsyc68e4xtec0tgqg", "butt_viewing_key": "DoTheRightThing..", "admin": true }}'
```

### Handle functions

1. Set API Key

``` sh
secretcli tx compute execute secret1hqrdl6wstt8qzshwc6mrumpjk9338k0lpsefm3 '{"set_api_key": {"api_key": "DoTheRightThing."}}' --from a -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
```

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- MARKDOWN LINKS & IMAGES -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->
[product-screenshot]: images/screenshot.png
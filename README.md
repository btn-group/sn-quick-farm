<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://github.com/btn-group">
    <img src="images/logo.png" alt="Logo" height="80">
  </a>

  <h3 align="center">SN Zap by btn.group</h3>
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
  </ol>
</details>

<!-- ABOUT THE PROJECT -->
## About The Project

This is a smart contract for btn.group's Button Swap ZAP functionality.
The contract:
1. Swaps a single token into BUTT & SWBTC.
2. Provides the tokens as liquidity into the trade contract.
3. Deposits the LP token received for providing liquidity into the BUTT-SWBTC farm pool for the user.

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
# MISSING THE SECRET SWAP CONTRACTS
secretcli tx compute store snip-20-reference-impl.wasm.gz --from a --gas 3000000 -y --keyring-backend test
secretcli tx compute store sn-dex-aggregator-v3.wasm.gz --from a --gas 3000000 -y --keyring-backend test
secretcli tx compute store sn-zap.wasm.gz --from a --gas 3000000 -y --keyring-backend test
secretcli tx compute store yield-optimizer-b.wasm.gz --from a --gas 3000000 -y --keyring-backend test

# Get the contract's id
secretcli query compute list-code
```

<p align="right">(<a href="#top">back to top</a>)</p>

### References
- https://github.com/btn-group/SecretSwap/blob/master/contracts/secretswap_pair/src/contract.rs

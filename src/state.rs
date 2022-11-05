use cosmwasm_std::HumanAddr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub admin: HumanAddr,
    pub dex_aggregator: SecretContract,
    pub butt: SecretContract,
    pub swbtc: SecretContract,
    pub butt_swbtc_trade_pair: SecretContract,
    pub butt_swbtc_lp: SecretContract,
    pub viewing_key: String,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone, JsonSchema)]
pub struct SecretContract {
    pub address: HumanAddr,
    pub contract_hash: String,
}

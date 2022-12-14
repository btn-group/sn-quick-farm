use cosmwasm_std::{HumanAddr, StdResult, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub admin: HumanAddr,
    pub current_user: Option<HumanAddr>,
    pub butt: SecretContract,
    pub swbtc: SecretContract,
    pub butt_swbtc_farm_pool: SecretContract,
    pub butt_swbtc_trade_pair: SecretContract,
    pub butt_swbtc_lp: SecretContract,
    pub swap_to_swbtc_contract_address: Option<HumanAddr>,
    pub butt_amount_to_provide: Option<Uint128>,
    pub swbtc_amount_to_provide: Option<Uint128>,
    pub viewing_key: String,
}
impl Config {
    pub fn with_public_attributes(self) -> StdResult<ConfigPublic> {
        Ok(ConfigPublic {
            admin: self.admin,
            butt: self.butt,
            swbtc: self.swbtc,
            butt_swbtc_farm_pool: self.butt_swbtc_farm_pool,
            butt_swbtc_trade_pair: self.butt_swbtc_trade_pair,
            butt_swbtc_lp: self.butt_swbtc_lp,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigPublic {
    pub admin: HumanAddr,
    pub butt: SecretContract,
    pub swbtc: SecretContract,
    pub butt_swbtc_farm_pool: SecretContract,
    pub butt_swbtc_trade_pair: SecretContract,
    pub butt_swbtc_lp: SecretContract,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone, JsonSchema)]
pub struct SecretContract {
    pub address: HumanAddr,
    pub contract_hash: String,
}

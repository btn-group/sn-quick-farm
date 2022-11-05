use crate::state::SecretContract;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub dex_aggregator: SecretContract,
    pub butt: SecretContract,
    pub swbtc: SecretContract,
    pub butt_swbtc_trade_pair: SecretContract,
    pub butt_swbtc_lp: SecretContract,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    IncreaseAllowanceForPairContract {},
}

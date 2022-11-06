use crate::constants::BLOCK_SIZE;
use crate::state::{Config, SecretContract};
use cosmwasm_std::{Binary, Decimal, HumanAddr, Uint128};
use schemars::JsonSchema;
use secret_toolkit::utils::HandleCallback;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub dex_aggregator: SecretContract,
    pub butt: SecretContract,
    pub swbtc: SecretContract,
    pub butt_swbtc_trade_pair: SecretContract,
    pub butt_swbtc_lp: SecretContract,
    pub viewing_key: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    IncreaseAllowanceForPairContract {},
    RegisterTokens {
        tokens: Vec<SecretContract>,
    },
    Receive {
        sender: HumanAddr,
        from: HumanAddr,
        amount: Uint128,
        msg: Option<Binary>,
    },
    SwapHalfOfSwbtcToButt {
        config: Config,
    },
    SendLpToUser {
        config: Config,
        user_address: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    InitSwapAndProvide {
        first_token_contract_hash: String,
        dex_aggregator_msg: Option<Binary>,
    },
}

// === Secret Swap Pair Contract ===
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    Token {
        contract_addr: HumanAddr,
        token_code_hash: String,
        viewing_key: String,
    },
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum SecretSwapHandleMsg {
    ProvideLiquidity {
        assets: [Asset; 2],
        slippage_tolerance: Option<Decimal>,
    },
}
impl HandleCallback for SecretSwapHandleMsg {
    const BLOCK_SIZE: usize = BLOCK_SIZE;
}

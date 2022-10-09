use crate::state::{ActivityRecord, SecretContract};
use cosmwasm_std::{Binary, HumanAddr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub butt: SecretContract,
    pub execution_fee: Uint128,
    pub sscrt: SecretContract,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive {
        sender: HumanAddr,
        from: HumanAddr,
        amount: Uint128,
        msg: Option<Binary>,
    },
    RescueTokens {
        denom: Option<String>,
        key: Option<String>,
        token_address: Option<HumanAddr>,
    },
    UpdateConfig {
        addresses_allowed_to_fill: Option<Vec<HumanAddr>>,
        execution_fee: Option<Uint128>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer<HumanizedOrder> {
    ActivityRecords {
        activity_records: Vec<ActivityRecord>,
        total: Option<Uint128>,
    },
    Orders {
        orders: Vec<HumanizedOrder>,
        total: Option<Uint128>,
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
    SetExecutionFeeForOrder {},
    CreateOrder {
        butt_viewing_key: String,
        to_amount: Uint128,
        to_token: HumanAddr,
    },
    FillOrder {
        position: Uint128,
    },
}

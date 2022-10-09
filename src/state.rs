use crate::constants::PREFIX_REGISTERED_TOKENS;
use cosmwasm_std::{CanonicalAddr, HumanAddr, StdResult, Storage, Uint128};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use schemars::JsonSchema;
use secret_toolkit::storage::{TypedStore, TypedStoreMut};
use serde::{Deserialize, Serialize};

// For tracking cancelled and filled
// activity (0 => cancelled, 1 => filled)
#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone, JsonSchema)]
pub struct ActivityRecord {
    pub order_position: Uint128,
    pub position: Uint128,
    pub activity: u8,
    pub result_from_amount_filled: Option<Uint128>,
    pub result_net_to_amount_filled: Option<Uint128>,
    pub updated_at_block_height: u64,
    pub updated_at_block_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub admin: HumanAddr,
    pub addresses_allowed_to_fill: Vec<HumanAddr>,
    pub butt: SecretContract,
    pub execution_fee: Uint128,
    pub sscrt: SecretContract,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone, JsonSchema)]
pub struct SecretContract {
    pub address: HumanAddr,
    pub contract_hash: String,
}

// === Registered tokens ===
#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone, JsonSchema)]
pub struct RegisteredToken {
    pub address: HumanAddr,
    pub contract_hash: String,
    pub sum_balance: Uint128,
}

pub fn read_registered_token<S: Storage>(
    storage: &S,
    token_address: &CanonicalAddr,
) -> Option<RegisteredToken> {
    let registered_tokens_storage = ReadonlyPrefixedStorage::new(PREFIX_REGISTERED_TOKENS, storage);
    let registered_tokens_storage = TypedStore::attach(&registered_tokens_storage);
    registered_tokens_storage
        .may_load(token_address.as_slice())
        .unwrap()
}

pub fn write_registered_token<S: Storage>(
    storage: &mut S,
    token_address: &CanonicalAddr,
    registered_token: &RegisteredToken,
) -> StdResult<()> {
    let mut registered_tokens_storage = PrefixedStorage::new(PREFIX_REGISTERED_TOKENS, storage);
    let mut registered_tokens_storage = TypedStoreMut::attach(&mut registered_tokens_storage);
    registered_tokens_storage.store(token_address.as_slice(), registered_token)
}

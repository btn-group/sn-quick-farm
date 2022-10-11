use crate::constants::{
    BLOCK_SIZE, CONFIG_KEY, MOCK_AMOUNT, MOCK_BUTT_ADDRESS, MOCK_TOKEN_ADDRESS, PREFIX_API_KEYS,
};
use crate::msg::{HandleMsg, InitMsg, QueryMsg};
use crate::state::{Config, SecretContract};
use cosmwasm_std::{
    to_binary, Api, Binary, CanonicalAddr, Env, Extern, HandleResponse, HumanAddr, InitResponse,
    Querier, StdResult, Storage, Uint128,
};
use cosmwasm_storage::PrefixedStorage;
use cosmwasm_storage::ReadonlyPrefixedStorage;
use secret_toolkit::snip20;
use secret_toolkit::storage::{TypedStore, TypedStoreMut};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let mut config_store = TypedStoreMut::attach(&mut deps.storage);
    let config: Config = Config {
        admin: env.message.sender,
        butt: msg.butt,
    };
    config_store.store(CONFIG_KEY, &config)?;

    Ok(InitResponse {
        messages: vec![],
        log: vec![],
    })
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::SetApiKey { api_key } => set_api_key(deps, env, api_key),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::ApiKey {
            address,
            butt_viewing_key,
            admin,
        } => api_key(deps, address, butt_viewing_key, admin),
    }
}

fn set_api_key<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    api_key: String,
) -> StdResult<HandleResponse> {
    let user_address_canonical: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;
    let mut prefixed_store = PrefixedStorage::new(PREFIX_API_KEYS, &mut deps.storage);
    let mut api_key_store = TypedStoreMut::<String, _>::attach(&mut prefixed_store);
    api_key_store.store(user_address_canonical.as_slice(), &api_key)?;
    let response = Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: None,
    });
    pad_response(response)
}

fn api_key<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
    butt_viewing_key: String,
    admin: bool,
) -> StdResult<Binary> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    // This is here so that the user can use their viewing key for butt for this
    if admin {
        query_balance_of_token(deps, config.admin, config.butt, butt_viewing_key)?;
    } else {
        query_balance_of_token(deps, address.clone(), config.butt, butt_viewing_key)?;
    }

    let store = ReadonlyPrefixedStorage::new(PREFIX_API_KEYS, &deps.storage);
    // Try to access the storage of orders for the account.
    // If it doesn't exist yet, return an empty list of transfers.
    let store = TypedStore::<String, _>::attach(&store);
    let api_key: Option<String> = store.may_load(&address.as_str().as_bytes())?;
    to_binary(&api_key)
}

fn pad_response(response: StdResult<HandleResponse>) -> StdResult<HandleResponse> {
    response.map(|mut response| {
        response.data = response.data.map(|mut data| {
            space_pad(BLOCK_SIZE, &mut data.0);
            data
        });
        response
    })
}

fn query_balance_of_token<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
    token: SecretContract,
    viewing_key: String,
) -> StdResult<Uint128> {
    if token.address == HumanAddr::from(MOCK_TOKEN_ADDRESS)
        || token.address == HumanAddr::from(MOCK_BUTT_ADDRESS)
    {
        Ok(Uint128(MOCK_AMOUNT))
    } else {
        let balance = snip20::balance_query(
            &deps.querier,
            address,
            viewing_key,
            BLOCK_SIZE,
            token.contract_hash,
            token.address,
        )?;
        Ok(balance.amount)
    }
}

// Take a Vec<u8> and pad it up to a multiple of `block_size`, using spaces at the end.
fn space_pad(block_size: usize, message: &mut Vec<u8>) -> &mut Vec<u8> {
    let len = message.len();
    let surplus = len % block_size;
    if surplus == 0 {
        return message;
    }

    let missing = block_size - surplus;
    message.reserve(missing);
    message.extend(std::iter::repeat(b' ').take(missing));
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SecretContract;
    use cosmwasm_std::from_binary;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage};
    use cosmwasm_std::StdError::NotFound;

    pub const MOCK_ADMIN: &str = "admin";
    pub const MOCK_API_KEY: &str = "mock-api-key";
    pub const MOCK_VIEWING_KEY: &str = "DELIGHTFUL";

    // === HELPERS ===
    fn init_helper() -> (
        StdResult<InitResponse>,
        Extern<MockStorage, MockApi, MockQuerier>,
    ) {
        let env = mock_env(MOCK_ADMIN, &[]);
        let mut deps = mock_dependencies(20, &[]);
        let msg = InitMsg { butt: mock_butt() };
        let init_result = init(&mut deps, env.clone(), msg);
        (init_result, deps)
    }

    fn mock_butt() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_BUTT_ADDRESS),
            contract_hash: "mock-butt-contract-hash".to_string(),
        }
    }

    fn mock_user_address() -> HumanAddr {
        HumanAddr::from("gary")
    }

    // === TESTS ===
    #[test]
    fn test_set_api_key() {
        let (_init_result, mut deps) = init_helper();
        let env = mock_env(mock_user_address(), &[]);

        // when user sets an api key
        let handle_msg = HandleMsg::SetApiKey {
            api_key: MOCK_API_KEY.to_string(),
        };
        handle(&mut deps, env.clone(), handle_msg).unwrap();
        // * it sets the api key for the user
        let store = ReadonlyPrefixedStorage::new(PREFIX_API_KEYS, &deps.storage);
        let store = TypedStore::<String, _>::attach(&store);
        let user_address_canonical: CanonicalAddr =
            deps.api.canonical_address(&mock_user_address()).unwrap();
        let api_key: Option<String> = store.may_load(user_address_canonical.as_slice()).unwrap();
        assert_eq!(api_key, Some(MOCK_API_KEY.to_string()));
    }
}

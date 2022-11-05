use crate::constants::{BLOCK_SIZE, CONFIG_KEY, MOCK_AMOUNT, MOCK_BUTT_ADDRESS, VIEWING_KEY};
use crate::msg::{HandleMsg, InitMsg};
use crate::state::{Config, SecretContract};
use cosmwasm_std::{
    Api, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, InitResponse, Querier, StdResult,
    Storage, Uint128,
};

use secret_toolkit::snip20;
use secret_toolkit::storage::TypedStoreMut;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let mut config_store = TypedStoreMut::attach(&mut deps.storage);
    let config: Config = Config {
        admin: env.message.sender,
        dex_aggregator: msg.dex_aggregator,
        butt: msg.butt,
        swbtc: msg.swbtc,
        butt_swbtc_trade_pair: msg.butt_swbtc_trade_pair,
        butt_swbtc_lp: msg.butt_swbtc_lp,
    };
    config_store.store(CONFIG_KEY, &config)?;

    Ok(InitResponse {
        messages: vec![snip20::set_viewing_key_msg(
            VIEWING_KEY.to_string(),
            None,
            1,
            config.butt_swbtc_lp.contract_hash,
            config.butt_swbtc_lp.address,
        )?],
        log: vec![],
    })
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::IncreaseAllowanceForPairContract {} => {
            increase_allowance_for_pair_contract(deps)
        }
    }
}

fn increase_allowance_for_pair_contract<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
) -> StdResult<HandleResponse> {
    let mut messages: Vec<CosmosMsg> = vec![];
    let config: Config = TypedStoreMut::attach(&mut deps.storage).load(CONFIG_KEY)?;
    messages.push(secret_toolkit::snip20::increase_allowance_msg(
        config.butt_swbtc_trade_pair.address.clone(),
        Uint128(u128::MAX),
        None,
        None,
        BLOCK_SIZE,
        config.butt.contract_hash,
        config.butt.address,
    )?);
    messages.push(secret_toolkit::snip20::increase_allowance_msg(
        config.butt_swbtc_trade_pair.address,
        Uint128(u128::MAX),
        None,
        None,
        BLOCK_SIZE,
        config.swbtc.contract_hash,
        config.swbtc.address,
    )?);

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
    })
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
    if token.address == HumanAddr::from(MOCK_BUTT_ADDRESS) {
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

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::state::SecretContract;
//     use cosmwasm_std::from_binary;
//     use cosmwasm_std::testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage};
//     pub const MOCK_ADMIN: &str = "admin";
//     pub const MOCK_API_KEY: &str = "mock-api-key";
//     pub const MOCK_VIEWING_KEY: &str = "DELIGHTFUL";

//     // === HELPERS ===
//     fn init_helper() -> (
//         StdResult<InitResponse>,
//         Extern<MockStorage, MockApi, MockQuerier>,
//     ) {
//         let env = mock_env(MOCK_ADMIN, &[]);
//         let mut deps = mock_dependencies(20, &[]);
//         let msg = InitMsg { butt: mock_butt() };
//         let init_result = init(&mut deps, env.clone(), msg);
//         (init_result, deps)
//     }

//     fn mock_butt() -> SecretContract {
//         SecretContract {
//             address: HumanAddr::from(MOCK_BUTT_ADDRESS),
//             contract_hash: "mock-butt-contract-hash".to_string(),
//         }
//     }

//     fn mock_user_address() -> HumanAddr {
//         HumanAddr::from("gary")
//     }

//     // === TESTS ===
//     #[test]
//     fn test_api_key() {
//         let (_init_result, mut deps) = init_helper();
//         let env = mock_env(mock_user_address(), &[]);

//         // when user sets an api key
//         let handle_msg = HandleMsg::SetApiKey {
//             api_key: MOCK_API_KEY.to_string(),
//         };
//         handle(&mut deps, env.clone(), handle_msg).unwrap();
//         // = when api key for user is retrieved by the user
//         let res = query(
//             &deps,
//             QueryMsg::ApiKey {
//                 address: mock_user_address(),
//                 butt_viewing_key: MOCK_VIEWING_KEY.to_string(),
//                 admin: false,
//             },
//         );
//         let api_key: String = from_binary(&res.unwrap()).unwrap();
//         // = * it returns the api key for that user
//         assert_eq!(api_key, MOCK_API_KEY.to_string());

//         // = * when api key for user is retrieved by the admin
//         let res = query(
//             &deps,
//             QueryMsg::ApiKey {
//                 address: mock_user_address(),
//                 butt_viewing_key: MOCK_VIEWING_KEY.to_string(),
//                 admin: true,
//             },
//         );
//         let api_key: String = from_binary(&res.unwrap()).unwrap();
//         // = * it returns the api key for that user
//         assert_eq!(api_key, MOCK_API_KEY.to_string());

//         // == when address does not have an api_key
//         // == * it returns none
//         let res = query(
//             &deps,
//             QueryMsg::ApiKey {
//                 address: HumanAddr::from("Jules"),
//                 butt_viewing_key: MOCK_VIEWING_KEY.to_string(),
//                 admin: true,
//             },
//         );
//         let api_key: Option<String> = from_binary(&res.unwrap()).unwrap();
//         // = * it returns the api key for that user
//         assert_eq!(api_key, None);
//     }

//     #[test]
//     fn test_set_api_key() {
//         let (_init_result, mut deps) = init_helper();
//         let env = mock_env(mock_user_address(), &[]);

//         // when user sets an api key
//         let handle_msg = HandleMsg::SetApiKey {
//             api_key: MOCK_API_KEY.to_string(),
//         };
//         handle(&mut deps, env.clone(), handle_msg).unwrap();
//         // * it sets the api key for the user
//         let store = ReadonlyPrefixedStorage::new(PREFIX_API_KEYS, &deps.storage);
//         let store = TypedStore::<String, _>::attach(&store);
//         let user_address_canonical: CanonicalAddr =
//             deps.api.canonical_address(&mock_user_address()).unwrap();
//         let api_key: Option<String> = store.may_load(user_address_canonical.as_slice()).unwrap();
//         assert_eq!(api_key, Some(MOCK_API_KEY.to_string()));
//     }
// }

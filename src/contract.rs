use crate::constants::{
    BLOCK_SIZE, CONFIG_KEY, MOCK_AMOUNT, MOCK_BUTT_ADDRESS, MOCK_TOKEN_ADDRESS, PREFIX_API_KEYS,
};
use crate::msg::{HandleMsg, InitMsg, QueryMsg};
use crate::state::{Config, SecretContract};
use cosmwasm_std::{
    to_binary, Api, Binary, Env, Extern, HandleResponse, HumanAddr, InitResponse, Querier,
    StdResult, Storage, Uint128,
};
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
        HandleMsg::Receive {
            from, amount, msg, ..
        } => receive(deps, env, from, amount, msg),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => {
            let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY)?;
            Ok(to_binary(&config)?)
        }
        QueryMsg::ApiKey {
            address,
            butt_viewing_key,
            admin,
        } => api_key(deps, address, butt_viewing_key, admin),
    }
}

fn receive<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _from: HumanAddr,
    _amount: Uint128,
    _msg: Option<Binary>,
) -> StdResult<HandleResponse> {
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

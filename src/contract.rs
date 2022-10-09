use crate::constants::{BLOCK_SIZE, CONFIG_KEY};
use crate::msg::{HandleMsg, InitMsg, QueryMsg};
use crate::state::Config;
use crate::validations::authorize;
use cosmwasm_std::{
    to_binary, Api, Binary, Env, Extern, HandleResponse, HumanAddr, InitResponse, Querier,
    StdResult, Storage, Uint128,
};

use secret_toolkit::storage::{TypedStore, TypedStoreMut};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let mut config_store = TypedStoreMut::attach(&mut deps.storage);
    let config: Config = Config {
        addresses_allowed_to_fill: vec![env.message.sender.clone(), env.contract.address],
        admin: env.message.sender,
        butt: msg.butt,
        execution_fee: msg.execution_fee,
        sscrt: msg.sscrt,
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
        HandleMsg::UpdateConfig {
            addresses_allowed_to_fill,
            execution_fee,
        } => update_config(deps, &env, addresses_allowed_to_fill, execution_fee),
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

fn pad_response(response: StdResult<HandleResponse>) -> StdResult<HandleResponse> {
    response.map(|mut response| {
        response.data = response.data.map(|mut data| {
            space_pad(BLOCK_SIZE, &mut data.0);
            data
        });
        response
    })
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

fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    addresses_allowed_to_fill: Option<Vec<HumanAddr>>,
    execution_fee: Option<Uint128>,
) -> StdResult<HandleResponse> {
    let mut config_store = TypedStoreMut::attach(&mut deps.storage);
    let mut config: Config = config_store.load(CONFIG_KEY).unwrap();
    authorize(vec![config.admin.clone()], &env.message.sender)?;

    if let Some(addresses_allowed_to_fill_unwrapped) = addresses_allowed_to_fill {
        config.addresses_allowed_to_fill = addresses_allowed_to_fill_unwrapped;
        if !config
            .addresses_allowed_to_fill
            .contains(&env.contract.address)
        {
            config
                .addresses_allowed_to_fill
                .push(env.contract.address.clone())
        }
        if !config
            .addresses_allowed_to_fill
            .contains(&config.admin.clone())
        {
            config.addresses_allowed_to_fill.push(config.admin.clone())
        }
    }
    if let Some(execution_fee_unwrapped) = execution_fee {
        config.execution_fee = execution_fee_unwrapped;
    }
    config_store.store(CONFIG_KEY, &config)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: None,
    })
}

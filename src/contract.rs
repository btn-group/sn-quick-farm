use crate::constants::{
    BLOCK_SIZE, CONFIG_KEY, MOCK_AMOUNT, MOCK_BUTT_ADDRESS, MOCK_TOKEN_ADDRESS,
    PREFIX_CANCEL_RECORDS, PREFIX_CANCEL_RECORDS_COUNT, PREFIX_FILL_RECORDS_COUNT,
};
use crate::msg::{HandleMsg, InitMsg, QueryMsg};
use crate::state::{
    read_registered_token, write_registered_token, ActivityRecord, Config, RegisteredToken,
    SecretContract,
};
use crate::validations::authorize;
use cosmwasm_std::{
    to_binary, Api, BalanceResponse, BankMsg, BankQuery, Binary, CanonicalAddr, Coin, CosmosMsg,
    Env, Extern, HandleResponse, HumanAddr, InitResponse, Querier, QueryRequest, ReadonlyStorage,
    StdError, StdResult, Storage, Uint128,
};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use primitive_types::U256;
use secret_toolkit::snip20;
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
        HandleMsg::RegisterTokens {
            tokens,
            viewing_key,
        } => register_tokens(deps, &env, tokens, viewing_key),
        HandleMsg::RescueTokens {
            denom,
            key,
            token_address,
        } => rescue_tokens(deps, &env, denom, key, token_address),
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

fn prefix_activity_records_count(activity_records_storage_prefix: &[u8]) -> &[u8] {
    if activity_records_storage_prefix == PREFIX_CANCEL_RECORDS {
        PREFIX_CANCEL_RECORDS_COUNT
    } else {
        PREFIX_FILL_RECORDS_COUNT
    }
}

fn append_activity_record<S: Storage>(
    store: &mut S,
    activity_record: &ActivityRecord,
    for_address: &CanonicalAddr,
    storage_prefix: &[u8],
) -> StdResult<()> {
    let mut prefixed_store =
        PrefixedStorage::multilevel(&[storage_prefix, for_address.as_slice()], store);
    let mut activity_record_store = TypedStoreMut::<ActivityRecord, _>::attach(&mut prefixed_store);
    activity_record_store.store(
        &activity_record.position.u128().to_le_bytes(),
        activity_record,
    )?;
    set_count(
        store,
        for_address,
        prefix_activity_records_count(storage_prefix),
        activity_record
            .position
            .u128()
            .checked_add(1)
            .ok_or_else(|| {
                StdError::generic_err(
                    "Reached implementation limit for the number of activity records per address.",
                )
            })?,
    )
}

fn set_count<S: Storage>(
    store: &mut S,
    for_address: &CanonicalAddr,
    storage_prefix: &[u8],
    count: u128,
) -> StdResult<()> {
    let mut prefixed_store = PrefixedStorage::new(storage_prefix, store);
    let mut count_store = TypedStoreMut::<u128, _>::attach(&mut prefixed_store);
    count_store.store(for_address.as_slice(), &count)
}

fn calculate_fee(user_butt_balance: Uint128, to_amount: Uint128) -> Uint128 {
    let user_butt_balance_as_u128: u128 = user_butt_balance.u128();
    let nom = if user_butt_balance_as_u128 >= 100_000_000_000 {
        0
    } else if user_butt_balance_as_u128 >= 50_000_000_000 {
        6
    } else if user_butt_balance_as_u128 >= 25_000_000_000 {
        12
    } else if user_butt_balance_as_u128 >= 12_500_000_000 {
        18
    } else if user_butt_balance_as_u128 >= 6_250_000_000 {
        24
    } else {
        30
    };
    let fee: u128 = if nom == 0 {
        0
    } else {
        (U256::from(to_amount.u128()) * U256::from(nom) / U256::from(10_000)).as_u128()
    };

    Uint128(fee)
}

fn get_activity_records<S: ReadonlyStorage>(
    storage: &S,
    for_address: &CanonicalAddr,
    page: u128,
    page_size: u128,
    storage_prefix: &[u8],
) -> StdResult<(Vec<ActivityRecord>, u128)> {
    let total: u128 = storage_count(
        storage,
        for_address,
        prefix_activity_records_count(storage_prefix),
    )?;
    let offset: u128 = page * page_size;
    let end = total - offset;
    let start = end.saturating_sub(page_size);
    let store =
        ReadonlyPrefixedStorage::multilevel(&[storage_prefix, for_address.as_slice()], storage);
    let mut activity_records: Vec<ActivityRecord> = Vec::new();
    let store = TypedStore::<ActivityRecord, _>::attach(&store);
    for position in (start..end).rev() {
        activity_records.push(store.load(&position.to_le_bytes())?);
    }

    Ok((activity_records, total))
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

fn register_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    tokens: Vec<SecretContract>,
    viewing_key: String,
) -> StdResult<HandleResponse> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    authorize(vec![config.admin], &env.message.sender)?;
    let mut messages = vec![];
    for token in tokens {
        let token_address_canonical = deps.api.canonical_address(&token.address)?;
        let token_details: Option<RegisteredToken> =
            read_registered_token(&deps.storage, &token_address_canonical);
        if token_details.is_none() {
            let token_details: RegisteredToken = RegisteredToken {
                address: token.address.clone(),
                contract_hash: token.contract_hash.clone(),
                sum_balance: Uint128(0),
            };
            write_registered_token(&mut deps.storage, &token_address_canonical, &token_details)?;
            messages.push(snip20::register_receive_msg(
                env.contract_code_hash.clone(),
                None,
                BLOCK_SIZE,
                token.contract_hash.clone(),
                token.address.clone(),
            )?);
        }
        messages.push(snip20::set_viewing_key_msg(
            viewing_key.clone(),
            None,
            BLOCK_SIZE,
            token.contract_hash,
            token.address,
        )?);
    }

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
    })
}

fn rescue_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    denom: Option<String>,
    key: Option<String>,
    token_address: Option<HumanAddr>,
) -> StdResult<HandleResponse> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    authorize(vec![config.admin.clone()], &env.message.sender)?;

    let mut messages: Vec<CosmosMsg> = vec![];
    if let Some(denom_unwrapped) = denom {
        let balance_response: BalanceResponse =
            deps.querier.query(&QueryRequest::Bank(BankQuery::Balance {
                address: env.contract.address.clone(),
                denom: denom_unwrapped,
            }))?;

        let withdrawal_coins: Vec<Coin> = vec![balance_response.amount];
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address.clone(),
            to_address: config.admin.clone(),
            amount: withdrawal_coins,
        }));
    }

    if let Some(token_address_unwrapped) = token_address {
        if let Some(key_unwrapped) = key {
            let registered_token: RegisteredToken = read_registered_token(
                &deps.storage,
                &deps.api.canonical_address(&token_address_unwrapped)?,
            )
            .unwrap();
            let balance: Uint128 = query_balance_of_token(
                deps,
                env.contract.address.clone(),
                SecretContract {
                    address: token_address_unwrapped,
                    contract_hash: registered_token.contract_hash.clone(),
                },
                key_unwrapped,
            )?;
            let sum_balance: Uint128 = registered_token.sum_balance;
            let difference: Uint128 = (balance - sum_balance)?;
            if !difference.is_zero() {
                messages.push(snip20::transfer_msg(
                    config.admin,
                    difference,
                    None,
                    BLOCK_SIZE,
                    registered_token.contract_hash,
                    registered_token.address,
                )?)
            }
        }
    }

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
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

fn storage_count<S: ReadonlyStorage>(
    store: &S,
    for_address: &CanonicalAddr,
    storage_prefix: &[u8],
) -> StdResult<u128> {
    let store = ReadonlyPrefixedStorage::new(storage_prefix, store);
    let store = TypedStore::<u128, _>::attach(&store);
    let position: Option<u128> = store.may_load(for_address.as_slice())?;

    Ok(position.unwrap_or(0))
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

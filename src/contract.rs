use crate::constants::{BLOCK_SIZE, CONFIG_KEY, MOCK_AMOUNT, MOCK_BUTT_SWBTC_LP_ADDRESS};
use crate::msg::{Asset, AssetInfo, HandleMsg, InitMsg, QueryMsg, ReceiveMsg, SecretSwapHandleMsg};
use crate::state::{Config, SecretContract};
use crate::validations::authorize;
use cosmwasm_std::{
    from_binary, to_binary, Api, BankMsg, Binary, Coin, CosmosMsg, Env, Extern, HandleResponse,
    HumanAddr, InitResponse, Querier, QueryResult, StdError, StdResult, Storage, Uint128, WasmMsg,
};
use secret_toolkit::snip20;
use secret_toolkit::storage::{TypedStore, TypedStoreMut};
use secret_toolkit::utils::HandleCallback;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let mut config_store = TypedStoreMut::attach(&mut deps.storage);
    let config: Config = Config {
        admin: env.message.sender,
        current_user: None,
        butt: msg.butt,
        swbtc: msg.swbtc,
        butt_swbtc_farm_pool: msg.butt_swbtc_farm_pool,
        butt_swbtc_trade_pair: msg.butt_swbtc_trade_pair,
        butt_swbtc_lp: msg.butt_swbtc_lp,
        swap_to_swbtc_contract_address: None,
        swbtc_amount_to_provide: None,
        viewing_key: msg.viewing_key,
    };
    config_store.store(CONFIG_KEY, &config)?;

    Ok(InitResponse {
        messages: vec![snip20::set_viewing_key_msg(
            config.viewing_key.clone(),
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
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::IncreaseAllowanceForPairContract {} => {
            increase_allowance_for_pair_contract(deps)
        }
        HandleMsg::Receive {
            from, amount, msg, ..
        } => receive(deps, env, from, amount, msg),
        HandleMsg::RegisterTokens { tokens } => register_tokens(&env, tokens),
        HandleMsg::RescueTokens {
            amount,
            denom,
            token,
        } => rescue_tokens(deps, &env, amount, denom, token),
        HandleMsg::SendLpToUserThenDepositIntoFarmContract {} => {
            send_lp_to_user_then_deposit_into_farm_contract(deps, &env)
        }
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {
        QueryMsg::Config {} => query_config(deps),
    }
}

fn query_config<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> StdResult<Binary> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();

    to_binary(&config.with_public_attributes()?)
}

fn receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    from: HumanAddr,
    amount: Uint128,
    msg: Option<Binary>,
) -> StdResult<HandleResponse> {
    let config: Config = TypedStoreMut::attach(&mut deps.storage)
        .load(CONFIG_KEY)
        .unwrap();
    let response = if let Some(msg_unwrapped) = msg {
        let msg: ReceiveMsg = from_binary(&msg_unwrapped)?;
        match msg {
            ReceiveMsg::InitSwapAndProvide {
                first_token_contract_hash,
                swap_to_swbtc_contract,
                swap_to_swbtc_msg,
            } => init_swap_and_provide(
                deps,
                &env,
                from,
                amount,
                config,
                first_token_contract_hash,
                swap_to_swbtc_contract,
                swap_to_swbtc_msg,
            ),
        }
    } else if env.message.sender == config.swbtc.address {
        swap_half_of_swbtc_to_butt(deps, &env, from, amount, config)
    } else if env.message.sender == config.butt.address {
        provide_liquidity_to_trade_pair(deps, &env, from, amount, config)
    } else {
        return Err(StdError::generic_err(
            "Receive message combination is wrong.",
        ));
    };
    pad_response(response)
}

// No matter what first swap has to return in a swap to swbtc
fn init_swap_and_provide<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    from: HumanAddr,
    amount: Uint128,
    mut config: Config,
    first_token_contract_hash: String,
    swap_to_swbtc_contract: Option<SecretContract>,
    swap_to_swbtc_msg: Option<Binary>,
) -> StdResult<HandleResponse> {
    // 1. Make sure token isn't BUTT
    if config.butt.address == env.message.sender {
        return Err(StdError::generic_err(
            "Token can't be BUTT when ReceiveMsg present.",
        ));
    };
    // 2. Make sure contract isn't being used already
    if config.current_user.is_some() {
        return Err(StdError::generic_err("Contract is already being used."));
    }

    let mut messages: Vec<CosmosMsg> = vec![];
    // 3. Swap token to SWBTC if first token is not SWBTC
    // Or send the SWBTC to the contract again which would simulate the result of a swap to swbtc
    if config.swbtc.address == env.message.sender {
        config.swap_to_swbtc_contract_address = Some(env.contract.address.clone());
        messages.push(snip20::send_msg(
            env.contract.address.clone(),
            amount,
            None,
            None,
            BLOCK_SIZE,
            config.swbtc.contract_hash.clone(),
            config.swbtc.address.clone(),
        )?);
    } else {
        if swap_to_swbtc_msg.is_none() {
            return Err(StdError::generic_err("Swap to SWBTC msg missing."));
        }
        if swap_to_swbtc_contract.is_none() {
            return Err(StdError::generic_err("Swap to SWBTC contract missing."));
        }

        config.swap_to_swbtc_contract_address =
            Some(swap_to_swbtc_contract.clone().unwrap().address);
        messages.push(snip20::send_msg(
            swap_to_swbtc_contract.unwrap().address,
            amount,
            swap_to_swbtc_msg,
            None,
            BLOCK_SIZE,
            first_token_contract_hash,
            env.message.sender.clone(),
        )?);
    }

    // 5. Call function to send lp to user then deposit into farm contract
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone(),
        callback_code_hash: env.contract_code_hash.clone(),
        msg: to_binary(&HandleMsg::SendLpToUserThenDepositIntoFarmContract {})?,
        send: vec![],
    }));

    // 6. Store Config
    config.current_user = Some(from);
    TypedStoreMut::attach(&mut deps.storage).store(CONFIG_KEY, &config)?;

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
    })
}

fn increase_allowance_for_pair_contract<S: Storage, A: Api, Q: Querier>(
    deps: &mut cosmwasm_std::Extern<S, A, Q>,
) -> StdResult<HandleResponse> {
    let mut messages: Vec<CosmosMsg> = vec![];
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY)?;
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

fn rescue_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    amount: Uint128,
    denom: Option<String>,
    token: Option<SecretContract>,
) -> StdResult<HandleResponse> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    authorize(vec![config.admin.clone()], &env.message.sender)?;

    let mut messages: Vec<CosmosMsg> = vec![];
    if let Some(denom_unwrapped) = denom {
        let withdrawal_coin: Vec<Coin> = vec![Coin {
            amount,
            denom: denom_unwrapped,
        }];
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address.clone(),
            to_address: config.admin.clone(),
            amount: withdrawal_coin,
        }));
    }

    if let Some(token_unwrapped) = token {
        messages.push(snip20::transfer_msg(
            config.admin,
            amount,
            None,
            BLOCK_SIZE,
            token_unwrapped.contract_hash,
            token_unwrapped.address,
        )?)
    }

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
    })
}

fn swap_half_of_swbtc_to_butt<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: &Env,
    from: HumanAddr,
    amount: Uint128,
    mut config: Config,
) -> StdResult<HandleResponse> {
    // Test that it's sent from swap_to_swbtc_contract_address
    if config.swap_to_swbtc_contract_address.is_none() {
        return Err(StdError::generic_err("Swap to SWBTC contract missing."));
    }
    authorize(
        [from].to_vec(),
        &config.swap_to_swbtc_contract_address.clone().unwrap(),
    )?;

    let swbtc_amount_to_swap: Uint128 = Uint128(amount.u128() / 2);
    config.swbtc_amount_to_provide = Some((amount - swbtc_amount_to_swap)?);
    TypedStoreMut::attach(&mut deps.storage).store(CONFIG_KEY, &config)?;

    Ok(HandleResponse {
        messages: vec![secret_toolkit::snip20::send_msg(
            config.butt_swbtc_trade_pair.address,
            swbtc_amount_to_swap,
            Some(Binary::from(r#"{ "swap": {} }"#.as_bytes())),
            None,
            BLOCK_SIZE,
            config.swbtc.contract_hash,
            config.swbtc.address,
        )?],
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

fn provide_liquidity_to_trade_pair<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: &Env,
    from: HumanAddr,
    amount: Uint128,
    config: Config,
) -> StdResult<HandleResponse> {
    // Test that the sender is from the trade pair
    authorize([from].to_vec(), &config.butt_swbtc_trade_pair.address)?;

    let butt_amount_to_provide: Uint128 = amount;
    if butt_amount_to_provide.is_zero() {
        return Err(StdError::generic_err(
            "Contract BUTT balance must be greater than zero.",
        ));
    }

    if config.swbtc_amount_to_provide.is_none() {
        return Err(StdError::generic_err("swbtc_amount_to_provide is missing."));
    }

    let swbtc_amount_to_provide: Uint128 = config.swbtc_amount_to_provide.unwrap();
    if swbtc_amount_to_provide.is_zero() {
        return Err(StdError::generic_err(
            "SWBTC amount to provide must be greater than zero.",
        ));
    }

    // Provide liquidity to farm contract
    let provide_liquidity_msg = SecretSwapHandleMsg::ProvideLiquidity {
        assets: [
            Asset {
                amount: swbtc_amount_to_provide,
                info: AssetInfo::Token {
                    contract_addr: config.swbtc.address,
                    token_code_hash: config.swbtc.contract_hash,
                    viewing_key: "SecretSwap".to_string(),
                },
            },
            Asset {
                amount: butt_amount_to_provide,
                info: AssetInfo::Token {
                    contract_addr: config.butt.address,
                    token_code_hash: config.butt.contract_hash,
                    viewing_key: "SecretSwap".to_string(),
                },
            },
        ],
        slippage_tolerance: None,
    };
    let cosmos_msg = provide_liquidity_msg.to_cosmos_msg(
        config.butt_swbtc_trade_pair.contract_hash,
        config.butt_swbtc_trade_pair.address,
        None,
    )?;

    Ok(HandleResponse {
        messages: vec![cosmos_msg],
        log: vec![],
        data: None,
    })
}

fn query_balance_of_token<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
    token: SecretContract,
    viewing_key: String,
) -> StdResult<Uint128> {
    if token.address == HumanAddr::from(MOCK_BUTT_SWBTC_LP_ADDRESS) {
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

fn register_tokens(env: &Env, tokens: Vec<SecretContract>) -> StdResult<HandleResponse> {
    let mut messages = vec![];
    for token in tokens {
        let address = token.address;
        let contract_hash = token.contract_hash;
        messages.push(snip20::register_receive_msg(
            env.contract_code_hash.clone(),
            None,
            BLOCK_SIZE,
            contract_hash.clone(),
            address.clone(),
        )?);
    }

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
    })
}

fn send_lp_to_user_then_deposit_into_farm_contract<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
) -> StdResult<HandleResponse> {
    authorize([env.message.sender.clone()].to_vec(), &env.contract.address)?;
    let mut config: Config = TypedStoreMut::attach(&mut deps.storage)
        .load(CONFIG_KEY)
        .unwrap();
    if let Some(current_user_unwrapped) = config.current_user {
        // Query the contract's SWBTC balance
        let lp_balance_of_contract: Uint128 = query_balance_of_token(
            deps,
            env.contract.address.clone(),
            config.butt_swbtc_lp.clone(),
            config.viewing_key.clone(),
        )
        .unwrap();
        if lp_balance_of_contract.is_zero() {
            return Err(StdError::generic_err(
                "Contract BUTT-SWBTC LP balance must be greater than zero.",
            ));
        }

        config.current_user = None;
        config.swap_to_swbtc_contract_address = None;
        config.swbtc_amount_to_provide = None;
        TypedStoreMut::attach(&mut deps.storage).store(CONFIG_KEY, &config)?;

        Ok(HandleResponse {
            messages: vec![
                snip20::transfer_msg(
                    current_user_unwrapped.clone(),
                    lp_balance_of_contract,
                    None,
                    BLOCK_SIZE,
                    config.butt_swbtc_lp.contract_hash.clone(),
                    config.butt_swbtc_lp.address.clone(),
                )?,
                snip20::send_from_msg(
                    current_user_unwrapped,
                    config.butt_swbtc_farm_pool.address,
                    lp_balance_of_contract,
                    Some(Binary::from(
                        r#"{ "deposit_incentivized_token": {} }"#.as_bytes(),
                    )),
                    None,
                    BLOCK_SIZE,
                    config.butt_swbtc_lp.contract_hash,
                    config.butt_swbtc_lp.address,
                )?,
            ],
            log: vec![],
            data: None,
        })
    } else {
        Err(StdError::generic_err("Contract wasn't called properly."))
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
    use crate::state::{ConfigPublic, SecretContract};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage};
    pub const MOCK_ADMIN: &str = "admin";
    pub const MOCK_BUTT_SWBTC_TRADE_PAIR_CONTRACT_ADDRESS: &str = "mock-swbtc-address";
    pub const MOCK_SWAP_TO_SWBTC_ADDRESS: &str = "mock-swap-to-swbtc-address";
    pub const MOCK_VIEWING_KEY: &str = "DELIGHTFUL";
    pub const MOCK_BUTT_ADDRESS: &str = "mock-butt-address";
    pub const MOCK_SWBTC_ADDRESS: &str = "mock-swbtc-address";

    // === HELPERS ===
    fn init_helper() -> (
        StdResult<InitResponse>,
        Extern<MockStorage, MockApi, MockQuerier>,
    ) {
        let env = mock_env(MOCK_ADMIN, &[]);
        let mut deps = mock_dependencies(20, &[]);
        let msg = InitMsg {
            butt: mock_butt(),
            swbtc: mock_swbtc(),
            butt_swbtc_farm_pool: mock_butt_swbtc_farm_pool(),
            butt_swbtc_trade_pair: mock_butt_swbtc_trade_pair(),
            butt_swbtc_lp: mock_butt_swbtc_lp(),
            viewing_key: MOCK_VIEWING_KEY.to_string(),
        };
        let init_result = init(&mut deps, env.clone(), msg);
        (init_result, deps)
    }

    fn mock_butt() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_BUTT_ADDRESS),
            contract_hash: "mock-butt-contract-hash".to_string(),
        }
    }

    fn mock_butt_swbtc_farm_pool() -> SecretContract {
        SecretContract {
            address: HumanAddr::from("mock-butt-swbtc-farm-pool-address"),
            contract_hash: "mock-butt-swbtc-farm-pool-contract-hash".to_string(),
        }
    }

    fn mock_butt_swbtc_lp() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_BUTT_SWBTC_LP_ADDRESS),
            contract_hash: "mock-butt-swbtc-lp-contract-hash".to_string(),
        }
    }

    fn mock_butt_swbtc_trade_pair() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_BUTT_SWBTC_TRADE_PAIR_CONTRACT_ADDRESS),
            contract_hash: "mock-butt-swbtc-trade-pair-contract-hash".to_string(),
        }
    }

    fn mock_swbtc() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_SWBTC_ADDRESS),
            contract_hash: "mock-swbtc-contract-hash".to_string(),
        }
    }

    fn mock_swap_to_swbtc_contract() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_SWAP_TO_SWBTC_ADDRESS),
            contract_hash: "mock-swbtc-contract-hash".to_string(),
        }
    }

    fn mock_user_address() -> HumanAddr {
        HumanAddr::from("gary")
    }

    // === TESTS ===
    #[test]
    fn test_init() {
        let (init_result, deps) = init_helper();

        // * it stores the correct config
        let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        assert_eq!(
            config,
            Config {
                admin: HumanAddr::from(MOCK_ADMIN),
                current_user: None,
                butt: mock_butt(),
                swbtc: mock_swbtc(),
                butt_swbtc_farm_pool: mock_butt_swbtc_farm_pool(),
                butt_swbtc_trade_pair: mock_butt_swbtc_trade_pair(),
                butt_swbtc_lp: mock_butt_swbtc_lp(),
                swap_to_swbtc_contract_address: None,
                swbtc_amount_to_provide: None,
                viewing_key: MOCK_VIEWING_KEY.to_string(),
            }
        );

        // * it sets the viewing key for BUTT, SWBTC & BUTT-SWBTC LP
        assert_eq!(
            init_result.unwrap().messages,
            vec![snip20::set_viewing_key_msg(
                MOCK_VIEWING_KEY.to_string(),
                None,
                1,
                mock_butt_swbtc_lp().contract_hash,
                mock_butt_swbtc_lp().address,
            )
            .unwrap(),]
        );
    }

    // === QUERY ===
    #[test]
    fn test_query_config() {
        let (_init_result, deps) = init_helper();
        let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        let config_from_query: ConfigPublic =
            from_binary(&query(&deps, QueryMsg::Config {}).unwrap()).unwrap();
        assert_eq!(config.with_public_attributes().unwrap(), config_from_query);
    }

    // === HANDLE ===
    #[test]
    fn test_increase_allowance_for_pair_contract() {
        let (_init_result, mut deps) = init_helper();

        // context when called by anyone
        let env = mock_env(mock_user_address(), &[]);
        // = * it increases the allowance for butt and swbtc
        let handle_msg = HandleMsg::IncreaseAllowanceForPairContract {};
        let handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        let handle_result_unwrapped = handle_result.unwrap();
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![
                secret_toolkit::snip20::increase_allowance_msg(
                    mock_butt_swbtc_trade_pair().address,
                    Uint128(u128::MAX),
                    None,
                    None,
                    BLOCK_SIZE,
                    mock_butt().contract_hash,
                    mock_butt().address,
                )
                .unwrap(),
                secret_toolkit::snip20::increase_allowance_msg(
                    mock_butt_swbtc_trade_pair().address,
                    Uint128(u128::MAX),
                    None,
                    None,
                    BLOCK_SIZE,
                    mock_swbtc().contract_hash,
                    mock_swbtc().address,
                )
                .unwrap()
            ]
        );
    }

    #[test]
    fn test_init_swap_and_provide() {
        let (_init_result, mut deps) = init_helper();
        let amount: Uint128 = Uint128(2);
        let mut config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        let swap_to_swbtc_msg: Option<Binary> = Some(to_binary(&123).unwrap());
        let mut receive_msg = ReceiveMsg::InitSwapAndProvide {
            swap_to_swbtc_contract: Some(mock_swap_to_swbtc_contract()),
            swap_to_swbtc_msg: swap_to_swbtc_msg.clone(),
            first_token_contract_hash: mock_butt().contract_hash,
        };
        // when token sent in is butt
        let mut env = mock_env(mock_butt().address, &[]);
        let mut handle_msg = HandleMsg::Receive {
            sender: mock_user_address(),
            from: mock_user_address(),
            amount,
            msg: Some(to_binary(&receive_msg).unwrap()),
        };
        let mut handle_result = handle(&mut deps, env, handle_msg.clone());
        // * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Token can't be BUTT when ReceiveMsg present.")
        );

        // when token sent in is swbtc
        env = mock_env(mock_swbtc().address, &[]);
        // * it sends the swbtc to itself
        // * it calls the function to read balance of LP and send to user
        handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        let mut handle_result_unwrapped = handle_result.unwrap();
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![
                snip20::send_msg(
                    env.contract.address.clone(),
                    amount,
                    None,
                    None,
                    BLOCK_SIZE,
                    config.swbtc.contract_hash.clone(),
                    config.swbtc.address.clone(),
                )
                .unwrap(),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.clone(),
                    callback_code_hash: env.contract_code_hash.clone(),
                    msg: to_binary(&HandleMsg::SendLpToUserThenDepositIntoFarmContract {}).unwrap(),
                    send: vec![],
                })
            ]
        );
        // * it updates config current user
        // * it updates the config's swap_to_swbtc_contract_address to the contract address
        config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        assert_eq!(config.current_user, Some(mock_user_address()));
        assert_eq!(
            config.swap_to_swbtc_contract_address,
            Some(env.contract.address)
        );

        // when token sent in is not swbtc or butt
        env = mock_env(mock_butt_swbtc_lp().address, &[]);
        // NEED TO RESET config.current_user set from previous test
        config.current_user = None;
        TypedStoreMut::attach(&mut deps.storage)
            .store(CONFIG_KEY, &config)
            .unwrap();
        // = when swap_to_swbtc_msg is present
        // = * it sends token to a contract to be swapped to swbtc
        // = * it calls the function to read balance of LP and send to user
        handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        handle_result_unwrapped = handle_result.unwrap();
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![
                snip20::send_msg(
                    mock_swap_to_swbtc_contract().address.clone(),
                    amount,
                    swap_to_swbtc_msg.clone(),
                    None,
                    BLOCK_SIZE,
                    mock_butt().contract_hash,
                    env.message.sender.clone(),
                )
                .unwrap(),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.clone(),
                    callback_code_hash: env.contract_code_hash.clone(),
                    msg: to_binary(&HandleMsg::SendLpToUserThenDepositIntoFarmContract {}).unwrap(),
                    send: vec![],
                })
            ]
        );
        // * it updates config current user
        // * it updates the config's swap_to_swbtc_contract_address to the contract address
        config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        assert_eq!(config.current_user, Some(mock_user_address()));
        assert_eq!(
            config.swap_to_swbtc_contract_address,
            Some(mock_swap_to_swbtc_contract().address)
        );

        // = when swap_to_swbtc_msg is missing
        // NEED TO RESET config.current_user set from previous test
        config.current_user = None;
        TypedStoreMut::attach(&mut deps.storage)
            .store(CONFIG_KEY, &config)
            .unwrap();
        receive_msg = ReceiveMsg::InitSwapAndProvide {
            swap_to_swbtc_contract: Some(mock_swap_to_swbtc_contract()),
            swap_to_swbtc_msg: None,
            first_token_contract_hash: mock_butt_swbtc_lp().contract_hash,
        };
        handle_msg = HandleMsg::Receive {
            sender: mock_user_address(),
            from: mock_user_address(),
            amount,
            msg: Some(to_binary(&receive_msg).unwrap()),
        };
        // = * it raises an error
        handle_result = handle(&mut deps, env.clone(), handle_msg);
        // * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Swap to SWBTC msg missing.")
        );

        // == when swap_to_swbtc_msg is present
        // === when swap_to_swbtc_contract is missing
        receive_msg = ReceiveMsg::InitSwapAndProvide {
            swap_to_swbtc_contract: None,
            swap_to_swbtc_msg,
            first_token_contract_hash: mock_butt_swbtc_lp().contract_hash,
        };
        handle_msg = HandleMsg::Receive {
            sender: mock_user_address(),
            from: mock_user_address(),
            amount,
            msg: Some(to_binary(&receive_msg).unwrap()),
        };
        // === * it raises an error
        handle_result = handle(&mut deps, env.clone(), handle_msg);
        // * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Swap to SWBTC contract missing.")
        );
    }

    #[test]
    fn test_provide_liquidity_to_trade_pair() {
        let (_init_result, mut deps) = init_helper();
        let butt_amount: Uint128 = Uint128(5);
        let mut config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();

        // = when called by BUTT
        let env: Env = mock_env(mock_butt().address, &[]);
        // == when called from non butt_swbtc_trade_pair
        let handle_msg = HandleMsg::Receive {
            sender: config.butt_swbtc_lp.address.clone(),
            from: config.butt_swbtc_lp.address.clone(),
            amount: butt_amount,
            msg: None,
        };
        let handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        // == * it raises an unauthorized error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::Unauthorized { backtrace: None }
        );
        // == when called from butt_swbtc_trade_pair
        // === when swbtc_amount_to_provide is none
        let handle_msg = HandleMsg::Receive {
            sender: config.butt_swbtc_trade_pair.address.clone(),
            from: config.butt_swbtc_trade_pair.address.clone(),
            amount: butt_amount,
            msg: None,
        };
        let handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        // === * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("swbtc_amount_to_provide is missing.")
        );
        // === when swbtc_amount_to_provide is zero
        config.swbtc_amount_to_provide = Some(Uint128(0));
        TypedStoreMut::attach(&mut deps.storage)
            .store(CONFIG_KEY, &config)
            .unwrap();
        // === * it raises an error
        let handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("SWBTC amount to provide must be greater than zero.")
        );
        // === when swbtc_amount_to_provide is zero
        config.swbtc_amount_to_provide = Some(Uint128(10));
        TypedStoreMut::attach(&mut deps.storage)
            .store(CONFIG_KEY, &config)
            .unwrap();

        // === * it provides the balance of BUTT and SWBTC of contract to trade pair contract
        let handle_result = handle(&mut deps, env, handle_msg.clone());
        let handle_result_unwrapped = handle_result.unwrap();
        let provide_liquidity_msg = SecretSwapHandleMsg::ProvideLiquidity {
            assets: [
                Asset {
                    amount: Uint128(10),
                    info: AssetInfo::Token {
                        contract_addr: config.swbtc.address,
                        token_code_hash: config.swbtc.contract_hash,
                        viewing_key: "SecretSwap".to_string(),
                    },
                },
                Asset {
                    amount: butt_amount,
                    info: AssetInfo::Token {
                        contract_addr: config.butt.address,
                        token_code_hash: config.butt.contract_hash.clone(),
                        viewing_key: "SecretSwap".to_string(),
                    },
                },
            ],
            slippage_tolerance: None,
        };
        let cosmos_msg = provide_liquidity_msg
            .to_cosmos_msg(
                mock_butt_swbtc_trade_pair().contract_hash,
                mock_butt_swbtc_trade_pair().address,
                None,
            )
            .unwrap();
        assert_eq!(handle_result_unwrapped.messages, vec![cosmos_msg]);
    }

    #[test]
    fn test_register_tokens() {
        let (_init_result, mut deps) = init_helper();
        let env = mock_env(mock_user_address(), &[]);

        // When tokens are in the parameter
        let handle_msg = HandleMsg::RegisterTokens {
            tokens: vec![mock_butt(), mock_swbtc()],
        };
        let handle_result = handle(&mut deps, env.clone(), handle_msg);
        let handle_result_unwrapped = handle_result.unwrap();
        // * it sends a message to register receive for the token
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![
                snip20::register_receive_msg(
                    env.contract_code_hash.clone(),
                    None,
                    BLOCK_SIZE,
                    mock_butt().contract_hash,
                    mock_butt().address,
                )
                .unwrap(),
                snip20::register_receive_msg(
                    env.contract_code_hash,
                    None,
                    BLOCK_SIZE,
                    mock_swbtc().contract_hash,
                    mock_swbtc().address,
                )
                .unwrap(),
            ]
        );
    }

    #[test]
    fn test_rescue_tokens() {
        let (_init_result, mut deps) = init_helper();
        let denom: String = "uscrt".to_string();
        let mut handle_msg = HandleMsg::RescueTokens {
            amount: Uint128(MOCK_AMOUNT),
            denom: Some(denom.clone()),
            token: Some(mock_butt()),
        };
        // = when called by a non-admin
        // = * it raises an Unauthorized error
        let mut env: Env = mock_env(mock_user_address(), &[]);
        let handle_result = handle(&mut deps, env, handle_msg.clone());
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::Unauthorized { backtrace: None }
        );

        // = when called by the admin
        env = mock_env(MOCK_ADMIN, &[]);
        // == when only denom is specified
        handle_msg = HandleMsg::RescueTokens {
            amount: Uint128(MOCK_AMOUNT),
            denom: Some(denom.clone()),
            token: None,
        };
        // === * it sends the amount specified of the coin of the denom to the admin
        let handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        let handle_result_unwrapped = handle_result.unwrap();
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![CosmosMsg::Bank(BankMsg::Send {
                from_address: env.contract.address,
                to_address: HumanAddr(MOCK_ADMIN.to_string()),
                amount: vec![Coin {
                    denom: denom,
                    amount: Uint128(MOCK_AMOUNT)
                }],
            })]
        );

        // == when only token is specified
        handle_msg = HandleMsg::RescueTokens {
            amount: Uint128(MOCK_AMOUNT),
            denom: None,
            token: Some(mock_butt()),
        };
        // == * it sends the amount specified of the token to the admin
        let handle_result = handle(&mut deps, mock_env(MOCK_ADMIN, &[]), handle_msg.clone());
        let handle_result_unwrapped = handle_result.unwrap();
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![snip20::transfer_msg(
                HumanAddr::from(MOCK_ADMIN),
                Uint128(MOCK_AMOUNT),
                None,
                BLOCK_SIZE,
                mock_butt().contract_hash,
                mock_butt().address,
            )
            .unwrap()]
        );
    }

    #[test]
    fn test_send_lp_to_user_then_deposit_into_farm_contract() {
        let (_init_result, mut deps) = init_helper();
        let mut config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        let handle_msg = HandleMsg::SendLpToUserThenDepositIntoFarmContract {};

        // when called by non-contract
        let mut env = mock_env(MOCK_ADMIN, &[]);
        // = * it raises an unauthorized error
        let mut handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::Unauthorized { backtrace: None }
        );

        // when called by contract
        env = mock_env(env.contract.address, &[]);
        // = when config current_user is missing
        // = * it raises an error
        handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Contract wasn't called properly.")
        );

        // = when config current_user is present
        config.current_user = Some(mock_user_address());
        TypedStoreMut::attach(&mut deps.storage)
            .store(CONFIG_KEY, &config)
            .unwrap();
        // == when contract's balance of butt-swbtc-lp is zero
        // == * it raises an error
        // handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        // assert_eq!(
        //     handle_result.unwrap_err(),
        //     StdError::generic_err("Result BUTT-SWBTC LP must be greater than zero.",)
        // );
        // == when contract's balance of butt-swbtc-lp is greater than zero
        // == * it sends the balance of the toke to the user
        handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        let handle_result_unwrapped = handle_result.unwrap();
        // * it sends a message to register receive for the token
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![
                snip20::transfer_msg(
                    mock_user_address(),
                    Uint128(MOCK_AMOUNT),
                    None,
                    BLOCK_SIZE,
                    config.butt_swbtc_lp.contract_hash.clone(),
                    config.butt_swbtc_lp.address.clone(),
                )
                .unwrap(),
                snip20::send_from_msg(
                    mock_user_address(),
                    config.butt_swbtc_farm_pool.address,
                    Uint128(MOCK_AMOUNT),
                    Some(Binary::from(
                        r#"{ "deposit_incentivized_token": {} }"#.as_bytes(),
                    )),
                    None,
                    BLOCK_SIZE,
                    config.butt_swbtc_lp.contract_hash,
                    config.butt_swbtc_lp.address,
                )
                .unwrap()
            ]
        );
    }

    #[test]
    fn test_swap_half_of_swbtc_to_butt() {
        let (_init_result, mut deps) = init_helper();
        let swbtc_amount: Uint128 = Uint128(5);

        // = when called by SWBTC
        let env: Env = mock_env(mock_swbtc().address, &[]);
        // == when swap_to_swbtc_contract_address is missing
        let mut config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        let handle_msg = HandleMsg::Receive {
            sender: config.swbtc.address.clone(),
            from: config.swbtc.address.clone(),
            amount: swbtc_amount,
            msg: None,
        };
        let handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        // == * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Swap to SWBTC contract missing.")
        );
        // == when swap_to_swbtc_contract_address is present
        config.swap_to_swbtc_contract_address = Some(env.contract.address.clone());
        // === when called from an address that is not the swap_to_swbtc_contract_address
        TypedStoreMut::attach(&mut deps.storage)
            .store(CONFIG_KEY, &config)
            .unwrap();
        // === * it raises an error
        let mut handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::Unauthorized { backtrace: None }
        );
        // === when called from the address that is the swap_to_swbtc_contract_address
        let handle_msg = HandleMsg::Receive {
            sender: env.contract.address.clone(),
            from: env.contract.address.clone(),
            amount: swbtc_amount,
            msg: None,
        };
        // === * it sends half the balance of swbtc to swap
        handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        let handle_result_unwrapped = handle_result.unwrap();
        let amount_to_swap = Uint128(swbtc_amount.u128() / 2);
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![secret_toolkit::snip20::send_msg(
                config.butt_swbtc_trade_pair.address,
                Uint128(swbtc_amount.u128() / 2),
                Some(Binary::from(r#"{ "swap": {} }"#.as_bytes())),
                None,
                BLOCK_SIZE,
                config.swbtc.contract_hash,
                config.swbtc.address,
            )
            .unwrap()]
        );
        // === * it stores the other half in config as swbtc_amount_to_provide
        config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        assert_eq!(
            config.swbtc_amount_to_provide,
            Some((swbtc_amount - amount_to_swap).unwrap())
        );
    }
}

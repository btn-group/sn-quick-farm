use crate::constants::{
    BLOCK_SIZE, CONFIG_KEY, MOCK_AMOUNT, MOCK_AMOUNT_TWO, MOCK_BUTT_ADDRESS, MOCK_SWBTC_ADDRESS,
};
use crate::msg::{Asset, AssetInfo, HandleMsg, InitMsg, QueryMsg, ReceiveMsg, SecretSwapHandleMsg};
use crate::state::{Config, SecretContract};
use crate::validations::authorize;
use cosmwasm_std::{
    from_binary, to_binary, Api, Binary, CosmosMsg, Env, Extern, HandleResponse, HumanAddr,
    InitResponse, Querier, QueryResult, StdError, StdResult, Storage, Uint128, WasmMsg,
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
        dex_aggregator: msg.dex_aggregator,
        butt: msg.butt,
        swbtc: msg.swbtc,
        butt_swbtc_trade_pair: msg.butt_swbtc_trade_pair,
        butt_swbtc_lp: msg.butt_swbtc_lp,
        viewing_key: msg.viewing_key,
    };
    config_store.store(CONFIG_KEY, &config)?;

    Ok(InitResponse {
        messages: vec![
            snip20::set_viewing_key_msg(
                config.viewing_key.clone(),
                None,
                1,
                config.butt.contract_hash,
                config.butt.address,
            )?,
            snip20::set_viewing_key_msg(
                config.viewing_key.clone(),
                None,
                1,
                config.swbtc.contract_hash,
                config.swbtc.address,
            )?,
            snip20::set_viewing_key_msg(
                config.viewing_key.clone(),
                None,
                1,
                config.butt_swbtc_lp.contract_hash,
                config.butt_swbtc_lp.address,
            )?,
        ],
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
        HandleMsg::ProvideLiquidityToTradePair { config } => {
            provide_liquidity_to_trade_pair(deps, &env, config)
        }
        HandleMsg::RegisterTokens { tokens } => register_tokens(&env, tokens),
        HandleMsg::SendLpToUser {
            config,
            user_address,
        } => send_lp_to_user(deps, &env, config, user_address),
        HandleMsg::SwapHalfOfSwbtcToButt { config } => {
            swap_half_of_swbtc_to_butt(deps, &env, config)
        }
        HandleMsg::UpdateDexAggregator { new_dex_aggregator } => {
            update_dex_aggregator(deps, &env, new_dex_aggregator)
        }
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {
        QueryMsg::Config { admin_viewing_key } => query_config(deps, admin_viewing_key),
    }
}

fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    admin_viewing_key: String,
) -> StdResult<Binary> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    // This is here to check the admin's viewing key
    query_balance_of_token(
        deps,
        config.admin.clone(),
        config.butt.clone(),
        admin_viewing_key,
    )?;

    to_binary(&config)
}

fn receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    from: HumanAddr,
    amount: Uint128,
    msg: Binary,
) -> StdResult<HandleResponse> {
    let msg: ReceiveMsg = from_binary(&msg)?;
    let response = match msg {
        ReceiveMsg::InitSwapAndProvide {
            dex_aggregator_msg,
            first_token_contract_hash,
        } => init_swap_and_provide(
            deps,
            &env,
            from,
            amount,
            dex_aggregator_msg,
            first_token_contract_hash,
        ),
    };
    pad_response(response)
}

fn init_swap_and_provide<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    from: HumanAddr,
    amount: Uint128,
    dex_aggregator_msg: Option<Binary>,
    first_token_contract_hash: String,
) -> StdResult<HandleResponse> {
    // 1. Make sure token isn't BUTT
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    if config.butt.address == env.message.sender {
        return Err(StdError::generic_err("First token can't be BUTT."));
    };

    let mut messages: Vec<CosmosMsg> = vec![];
    // 2. Swap to DEX aggregator if first token is not butt
    if config.swbtc.address != env.message.sender {
        if dex_aggregator_msg.is_none() {
            return Err(StdError::generic_err(
                "DEX aggregator msg must be present when first token is not SWBTC.",
            ));
        }

        messages.push(snip20::send_msg(
            config.dex_aggregator.address.clone(),
            amount,
            dex_aggregator_msg,
            None,
            BLOCK_SIZE,
            first_token_contract_hash,
            env.message.sender.clone(),
        )?);
    }

    // 3. Call function to swap half balance of SWBTC to BUTT, if the first token isn't swbtc
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone(),
        callback_code_hash: env.contract_code_hash.clone(),
        msg: to_binary(&HandleMsg::SwapHalfOfSwbtcToButt {
            config: config.clone(),
        })?,
        send: vec![],
    }));

    // 4. Call function to provide liquidity to trade pair
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone(),
        callback_code_hash: env.contract_code_hash.clone(),
        msg: to_binary(&HandleMsg::ProvideLiquidityToTradePair {
            config: config.clone(),
        })?,
        send: vec![],
    }));

    // 5. Call function to read balance of LP and send to user
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone(),
        callback_code_hash: env.contract_code_hash.clone(),
        msg: to_binary(&HandleMsg::SendLpToUser {
            config,
            user_address: from,
        })?,
        send: vec![],
    }));

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

fn swap_half_of_swbtc_to_butt<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    config: Config,
) -> StdResult<HandleResponse> {
    authorize([env.message.sender.clone()].to_vec(), &env.contract.address)?;

    let mut messages: Vec<CosmosMsg> = vec![];
    // Query the contract's SWBTC balance
    let swbtc_balance_of_contract: Uint128 = query_balance_of_token(
        deps,
        env.contract.address.clone(),
        config.swbtc.clone(),
        config.viewing_key,
    )
    .unwrap();
    // Swap half to BUTT
    messages.push(secret_toolkit::snip20::send_msg(
        config.butt_swbtc_trade_pair.address,
        Uint128(swbtc_balance_of_contract.u128() / 2),
        Some(Binary::from(r#"{ "swap": {} }"#.as_bytes())),
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

fn provide_liquidity_to_trade_pair<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    config: Config,
) -> StdResult<HandleResponse> {
    authorize([env.message.sender.clone()].to_vec(), &env.contract.address)?;

    // Query the contract's SWBTC balance
    let swbtc_balance_of_contract: Uint128 = query_balance_of_token(
        deps,
        env.contract.address.clone(),
        config.swbtc.clone(),
        config.viewing_key.clone(),
    )
    .unwrap();
    let butt_balance_of_contract: Uint128 = query_balance_of_token(
        deps,
        env.contract.address.clone(),
        config.butt.clone(),
        config.viewing_key,
    )
    .unwrap();
    // Provide liquidity to farm contract
    let provide_liquidity_msg = SecretSwapHandleMsg::ProvideLiquidity {
        assets: [
            Asset {
                amount: swbtc_balance_of_contract,
                info: AssetInfo::Token {
                    contract_addr: config.swbtc.address,
                    token_code_hash: config.swbtc.contract_hash,
                    viewing_key: "SecretSwap".to_string(),
                },
            },
            Asset {
                amount: butt_balance_of_contract,
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
    if token.address == HumanAddr::from(MOCK_BUTT_ADDRESS) {
        Ok(Uint128(MOCK_AMOUNT))
    } else if token.address == HumanAddr::from(MOCK_SWBTC_ADDRESS) {
        Ok(Uint128(MOCK_AMOUNT_TWO))
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

fn send_lp_to_user<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    config: Config,
    user_address: HumanAddr,
) -> StdResult<HandleResponse> {
    authorize([env.message.sender.clone()].to_vec(), &env.contract.address)?;

    // Query the contract's SWBTC balance
    let lb_balance_of_contract: Uint128 = query_balance_of_token(
        deps,
        env.contract.address.clone(),
        config.butt_swbtc_lp.clone(),
        config.viewing_key,
    )
    .unwrap();

    Ok(HandleResponse {
        messages: vec![snip20::transfer_msg(
            user_address,
            lb_balance_of_contract,
            None,
            BLOCK_SIZE,
            config.butt_swbtc_lp.contract_hash,
            config.butt_swbtc_lp.address,
        )?],
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

fn update_dex_aggregator<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    new_dex_aggregator: SecretContract,
) -> StdResult<HandleResponse> {
    let mut config_store = TypedStoreMut::attach(&mut deps.storage);
    let mut config: Config = config_store.load(CONFIG_KEY)?;
    authorize([env.message.sender.clone()].to_vec(), &config.admin)?;

    config.dex_aggregator = new_dex_aggregator;
    config_store.store(CONFIG_KEY, &config)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SecretContract;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage};
    pub const MOCK_ADMIN: &str = "admin";
    pub const MOCK_DEX_AGGREGATOR_ADDRESS: &str = "mock-dex-aggregator-address";
    pub const MOCK_BUTT_SWBTC_TRADE_PAIR_CONTRACT_ADDRESS: &str = "mock-swbtc-address";
    pub const MOCK_VIEWING_KEY: &str = "DELIGHTFUL";

    // === HELPERS ===
    fn init_helper() -> (
        StdResult<InitResponse>,
        Extern<MockStorage, MockApi, MockQuerier>,
    ) {
        let env = mock_env(MOCK_ADMIN, &[]);
        let mut deps = mock_dependencies(20, &[]);
        let msg = InitMsg {
            butt: mock_butt(),
            dex_aggregator: mock_dex_aggregator(),
            swbtc: mock_swbtc(),
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

    fn mock_butt_swbtc_lp() -> SecretContract {
        SecretContract {
            address: HumanAddr::from("mock-butt-swbtc-lp-address"),
            contract_hash: "mock-butt-swbtc-lp-contract-hash".to_string(),
        }
    }

    fn mock_butt_swbtc_trade_pair() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_BUTT_SWBTC_TRADE_PAIR_CONTRACT_ADDRESS),
            contract_hash: "mock-butt-swbtc-trade-pair-contract-hash".to_string(),
        }
    }

    fn mock_dex_aggregator() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_DEX_AGGREGATOR_ADDRESS),
            contract_hash: "mock-dex-aggregator-contract-hash".to_string(),
        }
    }

    fn mock_swbtc() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_SWBTC_ADDRESS),
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
                dex_aggregator: mock_dex_aggregator(),
                butt: mock_butt(),
                swbtc: mock_swbtc(),
                butt_swbtc_trade_pair: mock_butt_swbtc_trade_pair(),
                butt_swbtc_lp: mock_butt_swbtc_lp(),
                viewing_key: MOCK_VIEWING_KEY.to_string(),
            }
        );

        // * it sets the viewing key for BUTT, SWBTC & BUTT-SWBTC LP
        assert_eq!(
            init_result.unwrap().messages,
            vec![
                snip20::set_viewing_key_msg(
                    MOCK_VIEWING_KEY.to_string(),
                    None,
                    1,
                    mock_butt().contract_hash,
                    mock_butt().address,
                )
                .unwrap(),
                snip20::set_viewing_key_msg(
                    MOCK_VIEWING_KEY.to_string(),
                    None,
                    1,
                    mock_swbtc().contract_hash,
                    mock_swbtc().address,
                )
                .unwrap(),
                snip20::set_viewing_key_msg(
                    MOCK_VIEWING_KEY.to_string(),
                    None,
                    1,
                    mock_butt_swbtc_lp().contract_hash,
                    mock_butt_swbtc_lp().address,
                )
                .unwrap(),
            ]
        );
    }

    // === QUERY ===
    #[test]
    fn test_query_config() {
        let (_init_result, deps) = init_helper();
        let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();

        // when admin has a butt viewing key
        // = when admin submit the wrong butt viewing key, this will just have to be tested live

        // = when admin submits the right butt viewing key
        // =  it returns the config
        let config_from_query: Config = from_binary(
            &query(
                &deps,
                QueryMsg::Config {
                    admin_viewing_key: MOCK_VIEWING_KEY.to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(config, config_from_query);
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
        let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        let mut dex_aggregator_msg: Option<Binary> = Some(to_binary(&123).unwrap());
        let mut receive_msg = ReceiveMsg::InitSwapAndProvide {
            dex_aggregator_msg: dex_aggregator_msg.clone(),
            first_token_contract_hash: mock_butt().contract_hash,
        };
        // when token sent in is butt
        let mut env = mock_env(mock_butt().address, &[]);
        let mut handle_msg = HandleMsg::Receive {
            sender: mock_user_address(),
            from: mock_user_address(),
            amount,
            msg: to_binary(&receive_msg).unwrap(),
        };
        let mut handle_result = handle(&mut deps, env, handle_msg.clone());
        // * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("First token can't be BUTT.")
        );

        // when token sent in is swbtc
        env = mock_env(mock_swbtc().address, &[]);
        // * it calls the functions to swap half of SWBTC to BUTT
        // * it calls the function to provide liquidity to trade pair
        // * it calls the function to read balance of LP and send to user
        handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        let mut handle_result_unwrapped = handle_result.unwrap();
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.clone(),
                    callback_code_hash: env.contract_code_hash.clone(),
                    msg: to_binary(&HandleMsg::SwapHalfOfSwbtcToButt {
                        config: config.clone(),
                    })
                    .unwrap(),
                    send: vec![],
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.clone(),
                    callback_code_hash: env.contract_code_hash.clone(),
                    msg: to_binary(&HandleMsg::ProvideLiquidityToTradePair {
                        config: config.clone(),
                    })
                    .unwrap(),
                    send: vec![],
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.clone(),
                    callback_code_hash: env.contract_code_hash.clone(),
                    msg: to_binary(&HandleMsg::SendLpToUser {
                        config: config.clone(),
                        user_address: mock_user_address(),
                    })
                    .unwrap(),
                    send: vec![],
                })
            ]
        );

        // when token sent in is not swbtc or butt
        env = mock_env(mock_butt_swbtc_lp().address, &[]);
        // = when dex_aggregator_msg is present
        // = * it calls the function to swap to DEX aggregator
        // = * it calls the functions to swap half of SWBTC to BUTT
        // = * it calls the function to provide liquidity to trade pair
        // = * it calls the function to read balance of LP and send to user
        handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        handle_result_unwrapped = handle_result.unwrap();
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![
                snip20::send_msg(
                    config.dex_aggregator.address.clone(),
                    amount,
                    dex_aggregator_msg,
                    None,
                    BLOCK_SIZE,
                    mock_butt().contract_hash,
                    env.message.sender.clone(),
                )
                .unwrap(),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.clone(),
                    callback_code_hash: env.contract_code_hash.clone(),
                    msg: to_binary(&HandleMsg::SwapHalfOfSwbtcToButt {
                        config: config.clone(),
                    })
                    .unwrap(),
                    send: vec![],
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.clone(),
                    callback_code_hash: env.contract_code_hash.clone(),
                    msg: to_binary(&HandleMsg::ProvideLiquidityToTradePair {
                        config: config.clone(),
                    })
                    .unwrap(),
                    send: vec![],
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: env.contract.address.clone(),
                    callback_code_hash: env.contract_code_hash.clone(),
                    msg: to_binary(&HandleMsg::SendLpToUser {
                        config,
                        user_address: mock_user_address(),
                    })
                    .unwrap(),
                    send: vec![],
                })
            ]
        );

        // = when dex_aggregator_msg is missing
        dex_aggregator_msg = None;
        receive_msg = ReceiveMsg::InitSwapAndProvide {
            dex_aggregator_msg,
            first_token_contract_hash: mock_butt_swbtc_lp().contract_hash,
        };
        handle_msg = HandleMsg::Receive {
            sender: mock_user_address(),
            from: mock_user_address(),
            amount,
            msg: to_binary(&receive_msg).unwrap(),
        };
        // = * it raises an error
        handle_result = handle(&mut deps, env.clone(), handle_msg);
        // * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err(
                "DEX aggregator msg must be present when first token is not SWBTC."
            )
        );
    }

    #[test]
    fn test_provide_liquidity_to_trade_pair() {
        let (_init_result, mut deps) = init_helper();
        let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        let add_liquidity_to_pair_contract_msg = HandleMsg::ProvideLiquidityToTradePair {
            config: config.clone(),
        };

        // when called by non-contract
        let mut env = mock_env(MOCK_ADMIN, &[]);
        // = * it raises an unauthorized error
        let mut handle_result = handle(
            &mut deps,
            env.clone(),
            add_liquidity_to_pair_contract_msg.clone(),
        );
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::Unauthorized { backtrace: None }
        );

        // when called by contract
        env = mock_env(env.contract.address, &[]);
        // = * it provides the balance of BUTT and SWBTC of contract to trade pair contract
        handle_result = handle(
            &mut deps,
            env.clone(),
            add_liquidity_to_pair_contract_msg.clone(),
        );
        let handle_result_unwrapped = handle_result.unwrap();
        let provide_liquidity_msg = SecretSwapHandleMsg::ProvideLiquidity {
            assets: [
                Asset {
                    amount: Uint128(MOCK_AMOUNT_TWO),
                    info: AssetInfo::Token {
                        contract_addr: config.swbtc.address,
                        token_code_hash: config.swbtc.contract_hash,
                        viewing_key: "SecretSwap".to_string(),
                    },
                },
                Asset {
                    amount: Uint128(MOCK_AMOUNT),
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
}

use cosmwasm_std::{HumanAddr, StdError, StdResult};

pub fn authorize(allowed: Vec<HumanAddr>, received: &HumanAddr) -> StdResult<()> {
    if !allowed.contains(received) {
        return Err(StdError::Unauthorized { backtrace: None });
    }

    Ok(())
}

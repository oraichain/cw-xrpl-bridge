use crate::contract::{CONTRACT_NAME, CONTRACT_VERSION};
use crate::error::ContractError;

use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Env, Response, StdError};

use cw2::set_contract_version;

use crate::msg::MigrateMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let ver = cw2::get_contract_version(deps.storage)?;
    if ver.contract != CONTRACT_NAME {
        return Err(StdError::generic_err("Can only upgrade from same contract type").into());
    }
    // TODO Add migration logic, and version validation
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

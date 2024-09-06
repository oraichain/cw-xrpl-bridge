use crate::contract::{CONTRACT_NAME, CONTRACT_VERSION};
use crate::error::ContractError;
use crate::state::{BridgeState, Config, CONFIG};

use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Env, Response, StdError};

use cw2::set_contract_version;

use crate::msg::MigrateMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    let config = Config {
        relayers: msg.relayers,
        evidence_threshold: msg.evidence_threshold,
        used_ticket_sequence_threshold: msg.used_ticket_sequence_threshold,
        trust_set_limit_amount: msg.trust_set_limit_amount,
        bridge_xrpl_address: msg.bridge_xrpl_address,
        bridge_state: BridgeState::Active,
        xrpl_base_fee: msg.xrpl_base_fee,
        token_factory_addr: msg.token_factory_addr,
        rate_limit_addr: msg.rate_limit_addr,
        osor_entry_point: msg.osor_entry_point,
    };

    CONFIG.save(deps.storage, &config)?;
    let ver = cw2::get_contract_version(deps.storage)?;
    if ver.contract != CONTRACT_NAME {
        return Err(StdError::generic_err("Can only upgrade from same contract type").into());
    }
    // TODO Add migration logic, and version validation
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

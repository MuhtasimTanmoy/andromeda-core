#[cfg(feature = "vanilla")]
use crate::state::{
    read_code_id, store_code_id, ACTION_FEES, ADO_TYPE, CODE_ID, LATEST_VERSION, PUBLISHER,
    VERSION_CODE_ID,
};
use andromeda_std::ado_base::InstantiateMsg as BaseInstantiateMsg;
use andromeda_std::ado_contract::ADOContract;
use andromeda_std::common::encode_binary;
use andromeda_std::error::{from_semver, ContractError};
use andromeda_std::os::adodb::{
    ADOMetadata, ADOVersion, ActionFee, AndrQuery, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};
use cosmwasm_std::{
    attr, ensure, entry_point, from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo,
    Reply, Response, StdError, Storage,
};
use cw2::{get_contract_version, set_contract_version};
use semver::Version;
#[cfg(feature = "secret")]
use {
    crate::state_secret::{
        read_code_id, store_code_id, ACTION_FEES, ADO_TYPE, CODE_ID, LATEST_VERSION, PUBLISHER,
        VERSION_CODE_ID,
    },
    secret_std::{
        entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError,
        StdResult, Uint128,
    },
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-adodb";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "adodb".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.result.is_err() {
        return Err(ContractError::Std(StdError::generic_err(
            msg.result.unwrap_err(),
        )));
    }

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateCodeId {
            code_id_key,
            code_id,
        } => add_update_code_id(deps, env, info, code_id_key, code_id),
        ExecuteMsg::Publish {
            code_id,
            ado_type,
            action_fees,
            version,
            publisher,
        } => publish(
            deps,
            env,
            info,
            code_id,
            ado_type,
            version,
            action_fees,
            publisher,
        ),
        ExecuteMsg::UpdateActionFees {
            action_fees,
            ado_type,
        } => execute_update_action_fees(deps, info, ado_type, action_fees),
        ExecuteMsg::RemoveActionFees { ado_type, actions } => {
            execute_remove_actions(deps, info, ado_type, actions)
        }
        ExecuteMsg::UpdatePublisher {
            ado_type,
            publisher,
        } => execute_update_publisher(deps, info, ado_type, publisher),
    }
}

pub fn add_update_code_id(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    code_id_key: String,
    code_id: u64,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    store_code_id(
        deps.storage,
        &ADOVersion::from_string(code_id_key.clone()),
        code_id,
    )?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "add_update_code_id"),
        attr("code_id_key", code_id_key),
        attr("code_id", code_id.to_string()),
    ]))
}

#[cfg(feature = "vanilla")]
pub fn update_action_fees(
    storage: &mut dyn Storage,
    ado_type: String,
    fees: Vec<ActionFee>,
) -> Result<(), ContractError> {
    for action_fee in fees {
        ACTION_FEES.save(
            storage,
            (ado_type.clone(), action_fee.clone().action),
            &action_fee,
        )?;
    }

    Ok(())
}

#[cfg(feature = "secret")]
pub fn update_action_fees(
    storage: &mut dyn Storage,
    ado_type: String,
    fees: Vec<ActionFee>,
) -> Result<(), ContractError> {
    for action_fee in fees {
        ACTION_FEES.insert(
            storage,
            (&ado_type.clone(), &action_fee.clone().action),
            &action_fee,
        )?;
    }

    Ok(())
}

#[cfg(feature = "vanilla")]
#[allow(clippy::too_many_arguments)]
pub fn publish(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    code_id: u64,
    ado_type: String,
    version: String,
    action_fees: Option<Vec<ActionFee>>,
    publisher: Option<String>,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    let current_ado_version = LATEST_VERSION.may_load(deps.storage, ado_type.clone())?;
    if let Some(ado_version) = current_ado_version {
        let new_version = semver::Version::parse(&version).unwrap();
        let current_version = semver::Version::parse(&ado_version).unwrap();
        ensure!(
            new_version > current_version,
            ContractError::InvalidADOVersion {
                msg: Some("Version must be newer than the current version".to_string())
            }
        );
    }

    //TODO: Get Code ID info with cosmwasm 1.2

    let version = ADOVersion::from_type(ado_type).with_version(version);
    ensure!(
        version.validate(),
        ContractError::InvalidADOVersion { msg: None }
    );

    // Ensure version is not already published
    let curr_code_id =
        VERSION_CODE_ID.may_load(deps.storage, (version.get_type(), version.get_version()))?;
    ensure!(
        curr_code_id.is_none(),
        ContractError::InvalidADOVersion {
            msg: Some(String::from("Version already published"))
        }
    );

    store_code_id(deps.storage, &version, code_id)?;
    PUBLISHER.save(
        deps.storage,
        version.get_type(),
        &publisher.clone().unwrap_or(info.sender.to_string()),
    )?;

    if let Some(fees) = action_fees {
        update_action_fees(deps.storage, version.get_type(), fees)?;
    }

    Ok(Response::default().add_attributes(vec![
        attr("action", "publish_ado"),
        attr("ado_type", version.into_string()),
        attr("code_id", code_id.to_string()),
        attr("publisher", publisher.unwrap_or(info.sender.to_string())),
    ]))
}

#[cfg(feature = "secret")]
#[allow(clippy::too_many_arguments)]
pub fn publish(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    code_id: u64,
    ado_type: String,
    version: String,
    action_fees: Option<Vec<ActionFee>>,
    publisher: Option<String>,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_owner_or_operator(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    let current_ado_version = LATEST_VERSION.get(deps.storage, &ado_type.clone())?;
    if let Some(ado_version) = current_ado_version {
        let new_version = semver::Version::parse(&version).unwrap();
        let current_version = semver::Version::parse(&ado_version).unwrap();
        ensure!(
            new_version > current_version,
            ContractError::InvalidADOVersion {
                msg: Some("Version must be newer than the current version".to_string())
            }
        );
    }

    //TODO: Get Code ID info with cosmwasm 1.2

    let version = ADOVersion::from_type(ado_type).with_version(version);
    ensure!(
        version.validate(),
        ContractError::InvalidADOVersion { msg: None }
    );

    // Ensure version is not already published
    let curr_code_id =
        VERSION_CODE_ID.get(deps.storage, (&version.get_type(), &version.get_version()))?;
    ensure!(
        curr_code_id.is_none(),
        ContractError::InvalidADOVersion {
            msg: Some(String::from("Version already published"))
        }
    );

    store_code_id(deps.storage, &version, code_id)?;
    PUBLISHER.insert(
        deps.storage,
        &version.get_type(),
        &publisher.clone().unwrap_or(info.sender.to_string()),
    )?;

    if let Some(fees) = action_fees {
        update_action_fees(deps.storage, version.get_type(), fees)?;
    }

    Ok(Response::default().add_attributes(vec![
        attr("action", "publish_ado"),
        attr("ado_type", version.into_string()),
        attr("code_id", code_id.to_string()),
        attr("publisher", publisher.unwrap_or(info.sender.to_string())),
    ]))
}

#[cfg(feature = "vanilla")]
fn execute_update_action_fees(
    deps: DepsMut,
    info: MessageInfo,
    ado_type: String,
    action_fees: Vec<ActionFee>,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    let ado_type_exists = CODE_ID.may_load(deps.storage, &ado_type)?;
    ensure!(
        ado_type_exists.is_some(),
        ContractError::InvalidADOVersion {
            msg: Some("ADO type does not exist".to_string())
        }
    );

    update_action_fees(deps.storage, ado_type.clone(), action_fees)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "update_action_fees"),
        attr("ado_type", ado_type),
    ]))
}

#[cfg(feature = "secret")]
fn execute_update_action_fees(
    deps: DepsMut,
    info: MessageInfo,
    ado_type: String,
    action_fees: Vec<ActionFee>,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    let ado_type_exists = CODE_ID.get(deps.storage, &ado_type)?;
    ensure!(
        ado_type_exists.is_some(),
        ContractError::InvalidADOVersion {
            msg: Some("ADO type does not exist".to_string())
        }
    );

    update_action_fees(deps.storage, ado_type.clone(), action_fees)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "update_action_fees"),
        attr("ado_type", ado_type),
    ]))
}

#[cfg(feature = "vanilla")]
fn execute_remove_actions(
    deps: DepsMut,
    info: MessageInfo,
    ado_type: String,
    actions: Vec<String>,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    let ado_type_exists = CODE_ID.may_load(deps.storage, &ado_type)?;
    ensure!(
        ado_type_exists.is_some(),
        ContractError::InvalidADOVersion {
            msg: Some("ADO type does not exist".to_string())
        }
    );

    let mut res = Response::default().add_attributes(vec![
        attr("action", "remove_actions"),
        attr("ado_type", ado_type.clone()),
    ]);

    for action in actions {
        ACTION_FEES.remove(deps.storage, (ado_type.clone(), action.clone()));
        res = res.add_attribute("action_fee_removed", action);
    }

    Ok(res)
}

#[cfg(feature = "secret")]
fn execute_remove_actions(
    deps: DepsMut,
    info: MessageInfo,
    ado_type: String,
    actions: Vec<String>,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );
    let ado_type_exists = CODE_ID.get(deps.storage, &ado_type)?;
    ensure!(
        ado_type_exists.is_some(),
        ContractError::InvalidADOVersion {
            msg: Some("ADO type does not exist".to_string())
        }
    );

    let mut res = Response::default().add_attributes(vec![
        attr("action", "remove_actions"),
        attr("ado_type", ado_type.clone()),
    ]);

    for action in actions {
        ACTION_FEES.remove(deps.storage, &(ado_type.clone(), action.clone()));
        res = res.add_attribute("action_fee_removed", action);
    }

    Ok(res)
}

#[cfg(feature = "vanilla")]
fn execute_update_publisher(
    deps: DepsMut,
    info: MessageInfo,
    ado_type: String,
    publisher: String,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let ado_type_exists = CODE_ID.may_load(deps.storage, &ado_type)?;
    ensure!(
        ado_type_exists.is_some(),
        ContractError::InvalidADOVersion {
            msg: Some("ADO type does not exist".to_string())
        }
    );

    PUBLISHER.save(deps.storage, ado_type.clone(), &publisher)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "update_publisher"),
        attr("ado_type", ado_type),
        attr("publisher", publisher),
    ]))
}

#[cfg(feature = "secret")]
fn execute_update_publisher(
    deps: DepsMut,
    info: MessageInfo,
    ado_type: String,
    publisher: String,
) -> Result<Response, ContractError> {
    ensure!(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let ado_type_exists = CODE_ID.get(deps.storage, &ado_type)?;
    ensure!(
        ado_type_exists.is_some(),
        ContractError::InvalidADOVersion {
            msg: Some("ADO type does not exist".to_string())
        }
    );

    PUBLISHER.insert(deps.storage, &ado_type.clone(), &publisher)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "update_publisher"),
        attr("ado_type", ado_type),
        attr("publisher", publisher),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // New version
    let version: Version = CONTRACT_VERSION.parse().map_err(from_semver)?;

    // Old version
    let stored = get_contract_version(deps.storage)?;
    let storage_version: Version = stored.version.parse().map_err(from_semver)?;

    let contract = ADOContract::default();

    ensure!(
        stored.contract == CONTRACT_NAME,
        ContractError::CannotMigrate {
            previous_contract: stored.contract,
        }
    );

    // New version has to be newer/greater than the old version
    ensure!(
        storage_version < version,
        ContractError::CannotMigrate {
            previous_contract: stored.version,
        }
    );

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Update the ADOContract's version
    contract.execute_update_version(deps)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::CodeId { key } => encode_binary(&query_code_id(deps, key)?),
        QueryMsg::ADOType { code_id } => encode_binary(&query_ado_type(deps, code_id)?),
        QueryMsg::ADOMetadata { ado_type } => encode_binary(&query_ado_metadata(deps, ado_type)?),
        QueryMsg::ActionFee { ado_type, action } => {
            encode_binary(&query_action_fee(deps, ado_type, action)?)
        }
        QueryMsg::ActionFeeByCodeId { code_id, action } => {
            encode_binary(&query_action_fee_by_code_id(deps, code_id, action)?)
        }
        QueryMsg::AndrQuery(query) => temp_query_andr(deps, query),
    }
}

fn temp_query_andr(deps: Deps, query: AndrQuery) -> Result<Binary, ContractError> {
    match query {
        AndrQuery::Get(key) => temp_query_get(deps, key),
    }
}

fn temp_query_get(deps: Deps, msg: Option<Binary>) -> Result<Binary, ContractError> {
    if let Some(msg) = msg {
        let ado_key: String = from_binary(&msg)?;

        Ok(to_binary(&query_code_id(deps, ado_key)?)?)
    } else {
        Err(ContractError::InvalidQuery {})
    }
}

fn query_code_id(deps: Deps, key: String) -> Result<u64, ContractError> {
    let code_id = read_code_id(deps.storage, &key)?;
    Ok(code_id)
}

#[cfg(feature = "vanilla")]
fn query_ado_type(deps: Deps, code_id: u64) -> Result<Option<String>, ContractError> {
    Ok(ADO_TYPE.may_load(deps.storage, &code_id.to_string())?)
}

#[cfg(feature = "secret")]
fn query_ado_type(deps: Deps, code_id: u64) -> Result<Option<String>, ContractError> {
    Ok(Some(ADO_TYPE.get(deps.storage, &code_id.to_string())?))
}

#[cfg(feature = "vanilla")]
fn query_ado_metadata(deps: Deps, ado_type: String) -> Result<ADOMetadata, ContractError> {
    let publisher = PUBLISHER.load(deps.storage, ado_type.clone())?;
    let latest_version = LATEST_VERSION.load(deps.storage, ado_type)?;

    Ok(ADOMetadata {
        publisher,
        latest_version,
    })
}

#[cfg(feature = "secret")]
fn query_ado_metadata(deps: Deps, ado_type: String) -> Result<ADOMetadata, ContractError> {
    let publisher = PUBLISHER.get(deps.storage, &ado_type.clone())?;
    let latest_version = LATEST_VERSION.get(deps.storage, &ado_type)?;

    Ok(ADOMetadata {
        publisher,
        latest_version,
    })
}

#[cfg(feature = "vanilla")]
fn query_action_fee(
    deps: Deps,
    ado_type: String,
    action: String,
) -> Result<Option<ActionFee>, ContractError> {
    Ok(ACTION_FEES.may_load(deps.storage, (ado_type, action))?)
}

#[cfg(feature = "secret")]
fn query_action_fee(
    deps: Deps,
    ado_type: String,
    action: String,
) -> Result<Option<ActionFee>, ContractError> {
    Ok(Some(ACTION_FEES.get(deps.storage, &(ado_type, action))?))
}

#[cfg(feature = "vanilla")]
fn query_action_fee_by_code_id(
    deps: Deps,
    code_id: u64,
    action: String,
) -> Result<Option<ActionFee>, ContractError> {
    let ado_type = ADO_TYPE.load(deps.storage, &code_id.to_string())?;
    Ok(ACTION_FEES.may_load(deps.storage, (ado_type, action))?)
}

#[cfg(feature = "secret")]
fn query_action_fee_by_code_id(
    deps: Deps,
    code_id: u64,
    action: String,
) -> Result<Option<ActionFee>, ContractError> {
    let ado_type = ADO_TYPE.get(deps.storage, &code_id.to_string())?;
    Ok(Some(ACTION_FEES.get(deps.storage, &(ado_type, action))?))
}

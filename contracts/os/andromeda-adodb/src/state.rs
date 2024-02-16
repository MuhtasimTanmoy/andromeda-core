use andromeda_std::{
    error::ContractError,
    os::adodb::{ADOVersion, ActionFee},
};
use cosmwasm_std::{ensure, Api, StdResult, Storage};
use cw_storage_plus::{Item, Map};

/// Stores a mapping from an ADO type/version to its code ID
pub const CODE_ID: Map<&str, u64> = Map::new("code_id");
/// Stores unpublished code IDs to prevent resubmission of malicious contracts
pub const UNPUBLISHED_CODE_IDS: Item<Vec<u64>> = Item::new("unpublished_code_ids");
/// Stores the ado types with their corresponding unpublished version(s)
pub const UNPUBLISHED_VERSIONS: Map<&str, Vec<String>> = Map::new("unpublished_versions");
/// Stores the latest version for a given ADO
pub const LATEST_VERSION: Map<&str, (String, u64)> = Map::new("latest_version");
/// Stores a mapping from code ID to ADO
pub const ADO_TYPE: Map<u64, String> = Map::new("ado_type");
/// Stores a mapping from ADO to its publisher
pub const PUBLISHER: Map<&str, String> = Map::new("publisher");
/// Stores a mapping from an (ADO,Action) to its action fees
pub const ACTION_FEES: Map<&(String, String), ActionFee> = Map::new("action_fees");

pub fn store_code_id(
    storage: &mut dyn Storage,
    ado_version: &ADOVersion,
    code_id: u64,
) -> Result<(), ContractError> {
    let curr_type = ADO_TYPE.may_load(storage, code_id)?;
    ensure!(
        curr_type.is_none() || curr_type.unwrap() == ado_version.get_type(),
        ContractError::Unauthorized {}
    );
    ADO_TYPE
        .save(storage, code_id, &ado_version.get_type())
        .unwrap();
    LATEST_VERSION
        .save(
            storage,
            &ado_version.get_type(),
            &(ado_version.get_version(), code_id),
        )
        .unwrap();
    CODE_ID
        .save(storage, ado_version.as_str(), &code_id)
        .unwrap();

    // Check if there is any default ado set for this ado type. Defaults do not have versions appended to them.
    let default_ado = ADOVersion::from_type(ado_version.get_type());
    let default_code_id = read_code_id(storage, &default_ado);

    // There is no default, add one default for this
    if default_code_id.is_err() {
        CODE_ID
            .save(storage, default_ado.as_str(), &code_id)
            .unwrap();
    }
    Ok(())
}

pub fn remove_code_id(
    storage: &mut dyn Storage,
    ado_version: &ADOVersion,
    code_id: u64,
) -> Result<(), ContractError> {
    let curr_type = ADO_TYPE.may_load(storage, code_id)?;
    ensure!(
        curr_type.is_none() || curr_type.unwrap() == ado_version.get_type(),
        ContractError::Unauthorized {}
    );
    ADO_TYPE.remove(storage, code_id);
    LATEST_VERSION.remove(storage, &ado_version.get_type());
    CODE_ID.remove(storage, ado_version.as_str());

    // Check if there is any default ado set for this ado type. Defaults do not have versions appended to them.
    let default_ado = ADOVersion::from_type(ado_version.get_type());
    let default_code_id = read_code_id(storage, &default_ado);

    if default_code_id.is_ok() {
        CODE_ID.remove(storage, default_ado.as_str());
    }
    Ok(())
}

pub fn read_code_id(storage: &dyn Storage, ado_version: &ADOVersion) -> StdResult<u64> {
    if ado_version.get_version() == "latest" {
        let (_version, code_id) = read_latest_code_id(storage, ado_version.get_type())?;
        Ok(code_id)
    } else {
        CODE_ID.load(storage, ado_version.as_str())
    }
}

pub fn read_latest_code_id(storage: &dyn Storage, ado_type: String) -> StdResult<(String, u64)> {
    LATEST_VERSION.load(storage, &ado_type)
}

pub fn save_action_fees(
    storage: &mut dyn Storage,
    api: &dyn Api,
    ado_version: &ADOVersion,
    fees: Vec<ActionFee>,
) -> Result<(), ContractError> {
    for action_fee in fees {
        action_fee.validate_asset(api)?;
        ACTION_FEES.save(
            storage,
            &(ado_version.get_type(), action_fee.clone().action),
            &action_fee.clone(),
        )?;
    }

    Ok(())
}

pub fn remove_action_fees(
    storage: &mut dyn Storage,
    api: &dyn Api,
    ado_version: &ADOVersion,
    fees: Vec<ActionFee>,
) -> Result<(), ContractError> {
    for action_fee in fees {
        action_fee.validate_asset(api)?;
        ACTION_FEES.remove(
            storage,
            &(ado_version.get_type(), action_fee.clone().action),
        );
    }

    Ok(())
}

// pub fn read_all_ado_types(storage: &dyn Storage) -> StdResult<Vec<String>> {
//     let ado_types = CODE_ID
//         .keys(storage, None, None, Order::Ascending)
//         .flatten()
//         .collect();
//     Ok(ado_types)
// }

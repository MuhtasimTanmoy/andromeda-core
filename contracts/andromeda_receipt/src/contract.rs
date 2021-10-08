use crate::state::{
    can_mint_receipt, increment_num_receipt, read_receipt, store_config, store_receipt, CONFIG,
};
use andromeda_protocol::{
    receipt::{
        Config, ContractInfoResponse, ExecuteMsg, InstantiateMsg, QueryMsg, Receipt,
        ReceiptResponse,
    },
    require::require,
};
use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    store_config(
        deps.storage,
        &Config {
            owner: info.sender.to_string(), // token contract address
            minter: msg.minter,
            moderators: match msg.moderators {
                Some(moderators) => moderators,
                None => vec![],
            },
        },
    )?;
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::StoreReceipt { receipt } => execute_store_receipt(deps, info, receipt),
        ExecuteMsg::EditReceipt {
            receipt,
            receipt_id,
        } => execute_edit_receipt(deps, info, receipt_id, receipt),
    }
}

fn execute_store_receipt(
    deps: DepsMut,
    info: MessageInfo,
    receipt: Receipt,
) -> StdResult<Response> {
    require(
        can_mint_receipt(deps.storage, &info.sender.to_string())?,
        StdError::generic_err(
            "Only the contract owner, the assigned minter or a moderator can mint a receipt",
        ),
    )?;
    let receipt_id = increment_num_receipt(deps.storage)?;
    store_receipt(deps.storage, receipt_id, &receipt)?;
    Ok(Response::new().add_attribute("receipt_id", receipt_id.to_string()))
}

fn execute_edit_receipt(
    deps: DepsMut,
    info: MessageInfo,
    receipt_id: Uint128,
    receipt: Receipt,
) -> StdResult<Response> {
    require(
        can_mint_receipt(deps.storage, &info.sender.to_string())?,
        StdError::generic_err(
            "Only the contract owner, the assigned minter or a moderator can edit a receipt",
        ),
    )?;
    read_receipt(deps.storage, receipt_id)?;
    store_receipt(deps.storage, receipt_id, &receipt)?;

    Ok(Response::new()
        .add_attribute("receipt_id", receipt_id.to_string())
        .add_attribute("receipt_edited_by", info.sender.to_string()))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Receipt { receipt_id } => Ok(to_binary(&query_receipt(deps, receipt_id)?)?),
        QueryMsg::ContractInfo {} => Ok(to_binary(&query_config(deps)?)?),
    }
}

fn query_receipt(deps: Deps, receipt_id: Uint128) -> StdResult<ReceiptResponse> {
    let receipt = read_receipt(deps.storage, receipt_id)?;
    Ok(ReceiptResponse { receipt })
}

fn query_config(deps: Deps) -> StdResult<ContractInfoResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ContractInfoResponse { config })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    #[test]
    fn test_instantiate() {
        let owner = "creator";
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info(owner, &[]);
        let msg = InstantiateMsg {
            minter: owner.to_string(),
            moderators: None,
        };
        let res = instantiate(deps.as_mut(), env, info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn test_store_receipt() {
        let owner = "creator";
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info(owner, &[]);
        let unauth_info = mock_info("anyone", &[]);
        let config = Config {
            minter: owner.to_string(),
            owner: owner.to_string(),
            moderators: vec![],
        };
        store_config(deps.as_mut().storage, &config).unwrap();

        let msg = ExecuteMsg::StoreReceipt {
            receipt: Receipt { events: vec![] },
        };

        let res_unauth =
            execute(deps.as_mut(), env.clone(), unauth_info.clone(), msg.clone()).unwrap_err();
        assert_eq!(
            res_unauth,
            StdError::generic_err(
                "Only the contract owner, the assigned minter or a moderator can mint a receipt"
            )
        );

        //add address for registered moderator
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        assert_eq!(Response::new().add_attribute("receipt_id", "1"), res);
    }

    #[test]
    fn test_edit_receipt() {
        let owner = "creator";
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let info = mock_info(owner, &[]);
        let unauth_info = mock_info("anyone", &[]);
        let config = Config {
            minter: owner.to_string(),
            owner: owner.to_string(),
            moderators: vec![],
        };
        store_config(deps.as_mut().storage, &config).unwrap();

        let store_msg = ExecuteMsg::StoreReceipt {
            receipt: Receipt { events: vec![] },
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), store_msg.clone()).unwrap();
        assert_eq!(Response::new().add_attribute("receipt_id", "1"), res);

        let msg = ExecuteMsg::EditReceipt {
            receipt_id: Uint128::from(1 as u128),
            receipt: Receipt { events: vec![] },
        };

        let res_unauth =
            execute(deps.as_mut(), env.clone(), unauth_info.clone(), msg.clone()).unwrap_err();
        assert_eq!(
            res_unauth,
            StdError::generic_err(
                "Only the contract owner, the assigned minter or a moderator can edit a receipt"
            )
        );

        execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

        // let query_msg = QueryMsg::Receipt {
        //     receipt_id: Uint128::from(1 as u128),
        // };
        // let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
        // let val: ReceiptResponse = from_binary(&res).unwrap();
    }
}
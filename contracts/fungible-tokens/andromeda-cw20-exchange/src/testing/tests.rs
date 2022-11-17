use andromeda_fungible_tokens::cw20_exchange::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, Sale, SaleResponse, TokenAddressResponse,
};
use common::{ado_base::AndromedaQuery, app::AndrAddress, error::ContractError};
use cosmwasm_std::{
    attr, coins, from_binary,
    testing::{mock_dependencies, mock_env, mock_info},
    to_binary, wasm_execute, Addr, BankMsg, CosmosMsg, Empty, SubMsg, Uint128,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_asset::AssetInfo;

use crate::{
    contract::{execute, instantiate, query},
    state::{SALE, TOKEN_ADDRESS},
};

#[test]
pub fn test_instantiate() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let info = mock_info(owner.as_str(), &[]);
    let token_address = Addr::unchecked("cw20");

    instantiate(
        deps.as_mut(),
        env,
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let saved_token_address = TOKEN_ADDRESS.load(deps.as_ref().storage).unwrap();

    assert_eq!(saved_token_address.identifier, token_address.to_string())
}

#[test]
pub fn test_start_sale_invalid_token() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let info = mock_info(owner.as_str(), &[]);
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset,
        exchange_rate: Uint128::from(10u128),
    };
    // Owner set as Cw20ReceiveMsg sender to ensure that this message will error even if a malicious user
    // sends the message directly with the owner address provided
    let receive_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: Uint128::from(100u128),
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(
        err,
        ContractError::InvalidFunds {
            msg: "Incorrect CW20 provided for sale".to_string()
        }
    )
}

#[test]
pub fn test_start_sale_unauthorised() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let info = mock_info(owner.as_str(), &[]);
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset,
        exchange_rate: Uint128::from(10u128),
    };
    let receive_msg = Cw20ReceiveMsg {
        sender: "not_owner".to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: Uint128::from(100u128),
    };
    let msg = ExecuteMsg::Receive(receive_msg);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(err, ContractError::Unauthorized {})
}

#[test]
pub fn test_start_sale_zero_amount() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let info = mock_info(owner.as_str(), &[]);
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset,
        exchange_rate: Uint128::from(10u128),
    };
    let receive_msg = Cw20ReceiveMsg {
        sender: "not_owner".to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: Uint128::zero(),
    };
    let msg = ExecuteMsg::Receive(receive_msg);
    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(
        err,
        ContractError::InvalidFunds {
            msg: "Cannot send a 0 amount".to_string()
        }
    )
}

#[test]
pub fn test_start_sale() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);
    let token_info = mock_info(token_address.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    let sale_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset.clone(),
        exchange_rate,
    };
    let receive_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: sale_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    execute(deps.as_mut(), env, token_info, msg).unwrap();

    let sale = SALE
        .load(deps.as_ref().storage, &exchange_asset.to_string())
        .unwrap();

    assert_eq!(sale.exchange_rate, exchange_rate);
    assert_eq!(sale.amount, sale_amount)
}

#[test]
pub fn test_start_sale_ongoing() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);
    let token_info = mock_info(token_address.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    let sale_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset,
        exchange_rate,
    };
    let receive_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: sale_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    execute(deps.as_mut(), env.clone(), token_info.clone(), msg.clone()).unwrap();

    let err = execute(deps.as_mut(), env, token_info, msg).unwrap_err();

    assert_eq!(err, ContractError::SaleNotEnded {})
}

#[test]
pub fn test_start_sale_zero_exchange_rate() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);
    let token_info = mock_info(token_address.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::zero();
    let sale_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::StartSale {
        asset: exchange_asset,
        exchange_rate,
    };
    let receive_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: sale_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, token_info, msg).unwrap_err();

    assert_eq!(err, ContractError::InvalidZeroAmount {})
}

#[test]
pub fn test_purchase_no_sale() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let purchaser = Addr::unchecked("purchaser");
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);
    let token_info = mock_info("invalid_token", &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    // Purchase Tokens
    let purchase_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::Purchase { recipient: None };
    let receive_msg = Cw20ReceiveMsg {
        sender: purchaser.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: purchase_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, token_info, msg).unwrap_err();

    assert_eq!(err, ContractError::NoOngoingSale {});
}

#[test]
pub fn test_purchase_not_enough_sent() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let purchaser = Addr::unchecked("purchaser");
    let token_address = Addr::unchecked("cw20");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    SALE.save(
        deps.as_mut().storage,
        &exchange_asset.to_string(),
        &Sale {
            amount: Uint128::from(100u128),
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    let exchange_info = mock_info("exchanged_asset", &[]);
    let purchase_amount = Uint128::from(1u128);
    let hook = Cw20HookMsg::Purchase { recipient: None };
    let receive_msg = Cw20ReceiveMsg {
        sender: purchaser.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: purchase_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, exchange_info, msg).unwrap_err();

    assert_eq!(
        err,
        ContractError::InvalidFunds {
            msg: "Not enough funds sent to purchase a token".to_string()
        }
    );
}

#[test]
pub fn test_purchase_no_tokens_left() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let purchaser = Addr::unchecked("purchaser");
    let token_address = Addr::unchecked("cw20");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    SALE.save(
        deps.as_mut().storage,
        &exchange_asset.to_string(),
        &Sale {
            amount: Uint128::zero(),
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    let exchange_info = mock_info("exchanged_asset", &[]);
    let purchase_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::Purchase { recipient: None };
    let receive_msg = Cw20ReceiveMsg {
        sender: purchaser.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: purchase_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, exchange_info, msg).unwrap_err();

    assert_eq!(err, ContractError::NotEnoughTokens {});
}

#[test]
pub fn test_purchase_not_enough_tokens() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let purchaser = Addr::unchecked("purchaser");
    let token_address = Addr::unchecked("cw20");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    SALE.save(
        deps.as_mut().storage,
        &exchange_asset.to_string(),
        &Sale {
            amount: Uint128::one(),
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    let exchange_info = mock_info("exchanged_asset", &[]);
    let purchase_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::Purchase { recipient: None };
    let receive_msg = Cw20ReceiveMsg {
        sender: purchaser.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: purchase_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let err = execute(deps.as_mut(), env, exchange_info, msg).unwrap_err();

    assert_eq!(err, ContractError::NotEnoughTokens {});
}

#[test]
pub fn test_purchase() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let purchaser = Addr::unchecked("purchaser");
    let token_address = Addr::unchecked("cw20");
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    let sale_amount = Uint128::from(100u128);
    SALE.save(
        deps.as_mut().storage,
        &exchange_asset.to_string(),
        &Sale {
            amount: sale_amount,
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    let exchange_info = mock_info("exchanged_asset", &[]);
    let purchase_amount = Uint128::from(100u128);
    let hook = Cw20HookMsg::Purchase { recipient: None };
    let receive_msg = Cw20ReceiveMsg {
        sender: purchaser.to_string(),
        msg: to_binary(&hook).unwrap(),
        amount: purchase_amount,
    };
    let msg = ExecuteMsg::Receive(receive_msg);

    let res = execute(deps.as_mut(), env, exchange_info, msg).unwrap();

    // Check transfer
    let msg = res.messages.first().unwrap();
    let expected_wasm: CosmosMsg<Empty> = CosmosMsg::Wasm(
        wasm_execute(
            token_address.to_string(),
            &Cw20ExecuteMsg::Transfer {
                recipient: purchaser.to_string(),
                amount: Uint128::from(10u128),
            },
            vec![],
        )
        .unwrap(),
    );
    let expected = SubMsg::reply_on_error(expected_wasm, 2);
    assert_eq!(msg, &expected);

    // Check sale amount updated
    let sale = SALE
        .load(deps.as_mut().storage, &exchange_asset.to_string())
        .unwrap();

    assert_eq!(
        sale.amount,
        sale_amount.checked_sub(Uint128::from(10u128)).unwrap()
    )
}

#[test]
pub fn test_purchase_no_sale_native() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    // Purchase Tokens
    let purchase_amount = coins(100, "test");
    let msg = ExecuteMsg::Purchase { recipient: None };
    let info = mock_info("purchaser", &purchase_amount);

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(err, ContractError::NoOngoingSale {});
}

#[test]
pub fn test_purchase_not_enough_sent_native() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    SALE.save(
        deps.as_mut().storage,
        "native:test",
        &Sale {
            amount: Uint128::from(100u128),
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    let purchase_amount = coins(1, "test");
    let msg = ExecuteMsg::Purchase { recipient: None };
    let info = mock_info("purchaser", &purchase_amount);

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(
        err,
        ContractError::InvalidFunds {
            msg: "Not enough funds sent to purchase a token".to_string()
        }
    );
}

#[test]
pub fn test_purchase_no_tokens_left_native() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    SALE.save(
        deps.as_mut().storage,
        "native:test",
        &Sale {
            amount: Uint128::zero(),
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    let purchase_amount = coins(100, "test");
    let msg = ExecuteMsg::Purchase { recipient: None };
    let info = mock_info("purchaser", &purchase_amount);

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(err, ContractError::NotEnoughTokens {});
}

#[test]
pub fn test_purchase_not_enough_tokens_native() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    SALE.save(
        deps.as_mut().storage,
        "native:test",
        &Sale {
            amount: Uint128::from(1u128),
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    let purchase_amount = coins(100, "test");
    let msg = ExecuteMsg::Purchase { recipient: None };
    let info = mock_info("purchaser", &purchase_amount);

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(err, ContractError::NotEnoughTokens {});
}

#[test]
pub fn test_purchase_native() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let purchaser = Addr::unchecked("purchaser");
    let token_address = Addr::unchecked("cw20");
    let exchange_asset = AssetInfo::Native("test".to_string());
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    let sale_amount = Uint128::from(100u128);
    SALE.save(
        deps.as_mut().storage,
        &exchange_asset.to_string(),
        &Sale {
            amount: sale_amount,
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    // Purchase Tokens
    let purchase_amount = coins(100, "test");
    let msg = ExecuteMsg::Purchase { recipient: None };
    let info = mock_info("purchaser", &purchase_amount);

    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    // Check transfer
    let msg = res.messages.first().unwrap();
    let expected_wasm: CosmosMsg<Empty> = CosmosMsg::Wasm(
        wasm_execute(
            token_address.to_string(),
            &Cw20ExecuteMsg::Transfer {
                recipient: purchaser.to_string(),
                amount: Uint128::from(10u128),
            },
            vec![],
        )
        .unwrap(),
    );
    let expected = SubMsg::reply_on_error(expected_wasm, 2);
    assert_eq!(msg, &expected);

    // Check sale amount updated
    let sale = SALE
        .load(deps.as_mut().storage, &exchange_asset.to_string())
        .unwrap();

    assert_eq!(
        sale.amount,
        sale_amount.checked_sub(Uint128::from(10u128)).unwrap()
    )
}

#[test]
pub fn test_purchase_refund() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    SALE.save(
        deps.as_mut().storage,
        "native:test",
        &Sale {
            amount: Uint128::from(100u128),
            exchange_rate,
        },
    )
    .unwrap();

    // Purchase Tokens
    let purchase_amount = coins(105, "test");
    let msg = ExecuteMsg::Purchase { recipient: None };
    let info = mock_info("purchaser", &purchase_amount);

    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap();
    let refund_attribute = res.attributes.first().unwrap();
    let refund_message = res.messages.first().unwrap();

    assert_eq!(refund_attribute, attr("refunded_amount", "5"));
    assert_eq!(
        refund_message,
        &SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(5u128, "test")
        }))
    )
}

#[test]
pub fn test_cancel_sale_unauthorised() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));

    instantiate(
        deps.as_mut(),
        env.clone(),
        info,
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    let sale_amount = Uint128::from(100u128);
    SALE.save(
        deps.as_mut().storage,
        &exchange_asset.to_string(),
        &Sale {
            amount: sale_amount,
            exchange_rate,
        },
    )
    .unwrap();

    let msg = ExecuteMsg::CancelSale {
        asset: exchange_asset,
    };
    let unauthorised_info = mock_info("anyone", &[]);

    let err = execute(deps.as_mut(), env, unauthorised_info, msg).unwrap_err();

    assert_eq!(err, ContractError::Unauthorized {})
}

#[test]
pub fn test_cancel_sale_no_sale() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let msg = ExecuteMsg::CancelSale {
        asset: exchange_asset,
    };

    let err = execute(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(err, ContractError::NoOngoingSale {})
}

#[test]
pub fn test_cancel_sale() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let exchange_rate = Uint128::from(10u128);
    let sale_amount = Uint128::from(100u128);
    SALE.save(
        deps.as_mut().storage,
        &exchange_asset.to_string(),
        &Sale {
            amount: sale_amount,
            exchange_rate,
        },
    )
    .unwrap();

    let msg = ExecuteMsg::CancelSale {
        asset: exchange_asset.clone(),
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    // Ensure sale has been removed
    let sale_opt = SALE
        .may_load(deps.as_mut().storage, &exchange_asset.to_string())
        .unwrap();
    assert!(sale_opt.is_none());

    // Ensure any remaining funds are returned
    let message = res.messages.first().unwrap();
    let expected_message = SubMsg::reply_on_error(
        CosmosMsg::Wasm(
            wasm_execute(
                "exchanged_asset",
                &Cw20ExecuteMsg::Transfer {
                    recipient: owner.to_string(),
                    amount: sale_amount,
                },
                vec![],
            )
            .unwrap(),
        ),
        1,
    );
    assert_eq!(message, &expected_message)
}

#[test]
fn test_query_sale() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));

    let msg = QueryMsg::Sale {
        asset: exchange_asset.clone(),
    };
    let not_found_response: SaleResponse =
        from_binary(&query(deps.as_ref(), env.clone(), msg.clone()).unwrap()).unwrap();

    assert!(not_found_response.sale.is_none());

    let exchange_rate = Uint128::from(10u128);
    let sale_amount = Uint128::from(100u128);
    let sale = Sale {
        amount: sale_amount,
        exchange_rate,
    };
    SALE.save(deps.as_mut().storage, &exchange_asset.to_string(), &sale)
        .unwrap();

    let found_response: SaleResponse =
        from_binary(&query(deps.as_ref(), env, msg).unwrap()).unwrap();

    assert_eq!(found_response.sale, Some(sale));
}

#[test]
fn test_query_token_address() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let owner = Addr::unchecked("owner");
    let token_address = Addr::unchecked("cw20");
    let info = mock_info(owner.as_str(), &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            token_address: AndrAddress::from_string(token_address.to_string()),
        },
    )
    .unwrap();

    let msg = QueryMsg::TokenAddress {};
    let resp: TokenAddressResponse = from_binary(&query(deps.as_ref(), env, msg).unwrap()).unwrap();

    assert_eq!(resp.address, token_address.to_string())
}

#[test]
fn test_andr_query() {
    let env = mock_env();
    let mut deps = mock_dependencies();
    let exchange_asset = AssetInfo::Cw20(Addr::unchecked("exchanged_asset"));

    let exchange_rate = Uint128::from(10u128);
    let sale_amount = Uint128::from(100u128);
    let sale = Sale {
        amount: sale_amount,
        exchange_rate,
    };
    SALE.save(deps.as_mut().storage, &exchange_asset.to_string(), &sale)
        .unwrap();

    let msg = QueryMsg::AndrQuery(AndromedaQuery::Get(Some(
        to_binary(&QueryMsg::Sale {
            asset: exchange_asset.clone(),
        })
        .unwrap(),
    )));
    let query_msg_response: SaleResponse =
        from_binary(&query(deps.as_ref(), env.clone(), msg).unwrap()).unwrap();

    assert_eq!(query_msg_response.sale, Some(sale.clone()));

    let key_msg = QueryMsg::AndrQuery(AndromedaQuery::Get(Some(
        to_binary(&exchange_asset.to_string()).unwrap(),
    )));
    let key_response: SaleResponse =
        from_binary(&query(deps.as_ref(), env, key_msg).unwrap()).unwrap();

    assert_eq!(key_response.sale, Some(sale));
}

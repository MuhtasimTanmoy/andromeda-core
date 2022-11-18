use ado_base::state::ADOContract;
use andromeda_fungible_tokens::cw20_exchange::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, Sale, SaleAssetsResponse,
    SaleResponse, TokenAddressResponse,
};
use common::{
    ado_base::{AndromedaMsg, AndromedaQuery, InstantiateMsg as BaseInstantiateMsg},
    error::ContractError,
    parse_message_safe,
};
use cosmwasm_std::{
    attr, coin, ensure, entry_point, from_binary, to_binary, wasm_execute, BankMsg, Binary,
    CosmosMsg, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError, SubMsg, Uint128,
};
use cw2::{get_contract_version, set_contract_version};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_asset::AssetInfo;
use cw_storage_plus::Bound;
use cw_utils::{nonpayable, one_coin};
use semver::Version;

use crate::state::{SALE, TOKEN_ADDRESS};

pub struct ExecuteEnv<'a> {
    deps: DepsMut<'a>,
    env: Env,
    info: MessageInfo,
}

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-cw20-exchange";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// ID used for any refund sub messgaes
const REFUND_REPLY_ID: u64 = 1;
/// ID used for any purchased token transfer sub messages
const PURCHASE_REPLY_ID: u64 = 2;
/// ID used for transfer to sale recipient
const RECIPIENT_REPLY_ID: u64 = 3;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    TOKEN_ADDRESS.save(deps.storage, &msg.token_address)?;

    ADOContract::default().instantiate(
        deps.storage,
        env,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "cw20-exchange".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            modules: None,
            primitive_contract: None,
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
    let execute_env = ExecuteEnv { deps, env, info };
    let contract = ADOContract::default();

    // Do this before the hooks get fired off to ensure that there are no errors from the app
    // address not being fully setup yet.
    if let ExecuteMsg::AndrReceive(andr_msg) = msg.clone() {
        if let AndromedaMsg::UpdateAppContract { address } = andr_msg {
            let token_address = TOKEN_ADDRESS.load(execute_env.deps.storage)?;
            return contract.execute_update_app_contract(
                execute_env.deps,
                execute_env.info,
                address,
                Some(vec![token_address]),
            );
        } else if let AndromedaMsg::UpdateOwner { address } = andr_msg {
            return contract.execute_update_owner(execute_env.deps, execute_env.info, address);
        }
    }

    match msg {
        ExecuteMsg::CancelSale { asset } => execute_cancel_sale(execute_env, asset),
        ExecuteMsg::Purchase { recipient } => execute_purchase_native(execute_env, recipient),
        ExecuteMsg::Receive(cw20_msg) => execute_receive(execute_env, cw20_msg),
        ExecuteMsg::AndrReceive(msg) => ADOContract::default().execute(
            execute_env.deps,
            execute_env.env,
            execute_env.info,
            msg,
            execute,
        ),
    }
}

pub fn execute_receive(
    execute_env: ExecuteEnv,
    receive_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    nonpayable(&execute_env.info)?;

    let asset_sent = AssetInfo::Cw20(execute_env.info.sender.clone());
    let amount_sent = receive_msg.amount;
    let sender = receive_msg.sender;

    ensure!(
        !amount_sent.is_zero(),
        ContractError::InvalidFunds {
            msg: "Cannot send a 0 amount".to_string()
        }
    );

    match from_binary(&receive_msg.msg)? {
        Cw20HookMsg::StartSale {
            asset,
            exchange_rate,
            recipient,
        } => execute_start_sale(
            execute_env,
            amount_sent,
            asset,
            exchange_rate,
            sender,
            recipient,
        ),
        Cw20HookMsg::Purchase { recipient } => execute_purchase(
            execute_env,
            amount_sent,
            asset_sent,
            recipient.unwrap_or_else(|| sender.to_string()).as_str(),
            &sender,
        ),
    }
}

pub fn execute_start_sale(
    execute_env: ExecuteEnv,
    amount: Uint128,
    asset: AssetInfo,
    exchange_rate: Uint128,
    // The original sender of the CW20::Send message
    sender: String,
    // The recipient of the sale proceeds
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    let app_contract = ADOContract::default().get_app_contract(execute_env.deps.storage)?;
    let token_addr = TOKEN_ADDRESS.load(execute_env.deps.storage)?.get_address(
        execute_env.deps.api,
        &execute_env.deps.querier,
        app_contract,
    )?;

    ensure!(
        !exchange_rate.is_zero(),
        ContractError::InvalidZeroAmount {}
    );
    ensure!(
        ADOContract::default().is_contract_owner(execute_env.deps.storage, &sender)?,
        ContractError::Unauthorized {}
    );
    // Message sender in this case should be the token address
    ensure!(
        execute_env.info.sender == token_addr,
        ContractError::InvalidFunds {
            msg: "Incorrect CW20 provided for sale".to_string()
        }
    );

    // Do not allow duplicate sales
    let current_sale = SALE.may_load(execute_env.deps.storage, &asset.to_string())?;
    ensure!(current_sale.is_none(), ContractError::SaleNotEnded {});

    let sale = Sale {
        amount,
        exchange_rate,
        recipient: recipient.unwrap_or(sender),
    };
    SALE.save(execute_env.deps.storage, &asset.to_string(), &sale)?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "start_sale"),
        attr("asset", asset.to_string()),
        attr("rate", exchange_rate),
        attr("amount", amount),
    ]))
}

/// Generates a transfer message given an asset and an amount
fn generate_transfer_message(
    asset: AssetInfo,
    amount: Uint128,
    recipient: String,
    id: u64,
) -> Result<SubMsg, ContractError> {
    match asset.clone() {
        AssetInfo::Native(denom) => {
            let bank_msg = BankMsg::Send {
                to_address: recipient,
                amount: vec![coin(amount.u128(), denom)],
            };

            Ok(SubMsg::reply_on_error(CosmosMsg::Bank(bank_msg), id))
        }
        AssetInfo::Cw20(addr) => {
            let transfer_msg = Cw20ExecuteMsg::Transfer { recipient, amount };
            let wasm_msg = wasm_execute(addr, &transfer_msg, vec![])?;
            Ok(SubMsg::reply_on_error(CosmosMsg::Wasm(wasm_msg), id))
        }
        // Does not support 1155 currently
        _ => Err(ContractError::InvalidAsset {
            asset: asset.to_string(),
        }),
    }
}

pub fn execute_purchase(
    execute_env: ExecuteEnv,
    amount_sent: Uint128,
    asset_sent: AssetInfo,
    recipient: &str,
    // For refund purposes
    sender: &str,
) -> Result<Response, ContractError> {
    execute_env.deps.api.addr_validate(recipient)?;
    let mut resp = Response::default();

    let Some(mut sale) = SALE.may_load(execute_env.deps.storage, &asset_sent.to_string())? else {
        return Err(ContractError::NoOngoingSale {  })
    };

    let purchased = amount_sent.checked_div(sale.exchange_rate).unwrap();
    let remainder = amount_sent.checked_sub(purchased.checked_mul(sale.exchange_rate)?)?;

    ensure!(
        !purchased.is_zero(),
        ContractError::InvalidFunds {
            msg: "Not enough funds sent to purchase a token".to_string()
        }
    );
    ensure!(sale.amount >= purchased, ContractError::NotEnoughTokens {});

    // If purchase was rounded down return funds to purchaser
    if !remainder.is_zero() {
        resp = resp
            .add_submessage(generate_transfer_message(
                asset_sent.clone(),
                remainder,
                sender.to_string(),
                REFUND_REPLY_ID,
            )?)
            .add_attribute("refunded_amount", remainder);
    }

    // Transfer tokens to purchaser recipient
    let token_addr = TOKEN_ADDRESS.load(execute_env.deps.storage)?.get_address(
        execute_env.deps.api,
        &execute_env.deps.querier,
        ADOContract::default().get_app_contract(execute_env.deps.storage)?,
    )?;
    let transfer_msg = Cw20ExecuteMsg::Transfer {
        recipient: recipient.to_string(),
        amount: purchased,
    };
    let wasm_msg = wasm_execute(token_addr, &transfer_msg, vec![])?;
    resp = resp.add_submessage(SubMsg::reply_on_error(
        CosmosMsg::Wasm(wasm_msg),
        PURCHASE_REPLY_ID,
    ));

    // Update sale amount remaining
    sale.amount = sale.amount.checked_sub(purchased)?;
    SALE.save(execute_env.deps.storage, &asset_sent.to_string(), &sale)?;

    // Transfer exchanged asset to recipient
    resp = resp.add_submessage(generate_transfer_message(
        asset_sent.clone(),
        amount_sent,
        sale.recipient.clone(),
        RECIPIENT_REPLY_ID,
    )?);

    Ok(resp.add_attributes(vec![
        attr("action", "purchase"),
        attr("purchaser", sender),
        attr("recipient", recipient),
        attr("amount", purchased),
        attr("purchase_asset", asset_sent.to_string()),
        attr("purchase_asset_amount_send", amount_sent),
        attr("recipient", sale.recipient),
    ]))
}

pub fn execute_purchase_native(
    execute_env: ExecuteEnv,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    // Default to sender as recipient
    let recipient = recipient.unwrap_or_else(|| execute_env.info.sender.to_string());
    execute_env.deps.api.addr_validate(&recipient)?;
    let sender = execute_env.info.sender.to_string();

    // Only allow one coin for purchasing
    one_coin(&execute_env.info)?;

    let payment = execute_env.info.funds.first().unwrap();
    let asset = AssetInfo::Native(payment.denom.to_string());
    let amount = payment.amount;

    execute_purchase(execute_env, amount, asset, &recipient, &sender)
}

pub fn execute_cancel_sale(
    execute_env: ExecuteEnv,
    asset: AssetInfo,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    ensure!(
        contract.is_contract_owner(execute_env.deps.storage, execute_env.info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    let Some(sale) = SALE.may_load(execute_env.deps.storage, &asset.to_string())? else {
        return Err(ContractError::NoOngoingSale {  })
    };

    let mut resp = Response::default();

    // Refund any remaining amount
    if !sale.amount.is_zero() {
        resp = resp
            .add_submessage(generate_transfer_message(
                asset.clone(),
                sale.amount,
                execute_env.info.sender.to_string(),
                REFUND_REPLY_ID,
            )?)
            .add_attribute("refunded_amount", sale.amount);
    }

    // Sale can now be removed
    SALE.remove(execute_env.deps.storage, &asset.to_string());

    Ok(resp.add_attributes(vec![
        attr("action", "cancel_sale"),
        attr("asset", asset.to_string()),
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

fn from_semver(err: semver::Error) -> StdError {
    StdError::generic_err(format!("Semver: {}", err))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Sale { asset } => query_sale(deps, asset),
        QueryMsg::TokenAddress {} => query_token_address(deps),
        QueryMsg::SaleAssets { start_after, limit } => {
            query_sale_assets(deps, start_after.as_ref().map(|x| &**x), limit)
        }
        QueryMsg::AndrQuery(andr_msg) => handle_andromeda_query(deps, env, andr_msg),
    }
}

fn query_sale(deps: Deps, asset: impl ToString) -> Result<Binary, ContractError> {
    let sale = SALE.may_load(deps.storage, &asset.to_string())?;

    Ok(to_binary(&SaleResponse { sale })?)
}

fn query_token_address(deps: Deps) -> Result<Binary, ContractError> {
    let address = TOKEN_ADDRESS.load(deps.storage)?.get_address(
        deps.api,
        &deps.querier,
        ADOContract::default().get_app_contract(deps.storage)?,
    )?;

    Ok(to_binary(&TokenAddressResponse { address })?)
}

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 100;

fn query_sale_assets(
    deps: Deps,
    start_after: Option<&str>,
    limit: Option<u32>,
) -> Result<Binary, ContractError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let assets: Vec<String> = SALE
        .keys(deps.storage, start, None, cosmwasm_std::Order::Ascending)
        .take(limit)
        .collect::<Result<Vec<String>, StdError>>()?;

    Ok(to_binary(&SaleAssetsResponse { assets })?)
}

fn handle_andromeda_query(
    deps: Deps,
    env: Env,
    msg: AndromedaQuery,
) -> Result<Binary, ContractError> {
    match msg {
        AndromedaQuery::Get(data_opt) => {
            // Message must contain data
            let Some(data) = data_opt else {
                return Err(ContractError::MissingRequiredMessageData {  });
            };

            // Try to determine if data is an asset or a message
            // If message decodes to string assume it is a key and query sale for given key
            let Some(key) = parse_message_safe::<String>(&data)? else {
                // If the data is not a string then try to decode to a query message
                let Some(message) = parse_message_safe::<QueryMsg>(&data)? else {
                    return Err(ContractError::MissingRequiredMessageData {  });
                };

                return query(deps, env, message);
            };

            query_sale(deps, key)
        }
        _ => ADOContract::default().query(deps, env, msg, query),
    }
}

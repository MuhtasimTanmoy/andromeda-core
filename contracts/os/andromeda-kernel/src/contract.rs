use andromeda_std::ado_base::InstantiateMsg as BaseInstantiateMsg;
use andromeda_std::ado_contract::ADOContract;
use andromeda_std::amp::addresses::AndrAddr;
use andromeda_std::amp::messages::{AMPMsg, AMPMsgConfig, AMPPkt};
use andromeda_std::amp::ADO_DB_KEY;
use andromeda_std::common::encode_binary;
use andromeda_std::error::ContractError;
use andromeda_std::ibc::message_bridge::ExecuteMsg as IBCBridgeExecMsg;
use andromeda_std::os::aos_querier::AOSQuerier;
use andromeda_std::os::kernel::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use cosmwasm_std::{
    attr, ensure, entry_point, to_binary, wasm_execute, Addr, BankMsg, Binary, CosmosMsg, Deps,
    DepsMut, Env, MessageInfo, Reply, ReplyOn, Response, StdError, SubMsg, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version};
use semver::Version;

use crate::state::{parse_path_direct, parse_path_direct_no_ctx, IBC_BRIDGE, KERNEL_ADDRESSES};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andromeda-kernel";
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
        env.clone(),
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "kernel".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            kernel_address: env.contract.address.to_string(),
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

pub struct ExecuteEnv<'a> {
    deps: DepsMut<'a>,
    pub env: Env,
    pub info: MessageInfo,
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let execute_env = ExecuteEnv { deps, env, info };

    match msg {
        ExecuteMsg::AMPReceive(packet) => handle_amp_packet(execute_env, packet),
        ExecuteMsg::AMPDirect {
            recipient,
            message,
            reply_on,
            exit_at_error,
            gas_limit,
        } => handle_amp_direct(
            execute_env.deps,
            execute_env.env,
            execute_env.info,
            recipient,
            message,
            reply_on,
            exit_at_error,
            gas_limit,
        ),
        ExecuteMsg::AMPDirectNoCtx { recipient, message } => handle_amp_direct_no_ctx(
            execute_env.deps,
            execute_env.env,
            execute_env.info,
            recipient,
            message,
        ),
        ExecuteMsg::AMPMessage { message } => {
            handle_amp_message(execute_env.deps, execute_env.env, execute_env.info, message)
        }
        ExecuteMsg::UpsertKeyAddress { key, value } => upsert_key_address(execute_env, key, value),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_amp_direct(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: AndrAddr,
    message: Binary,
    reply_on: Option<ReplyOn>,
    exit_at_error: Option<bool>,
    gas_limit: Option<u64>,
) -> Result<Response, ContractError> {
    let origin = info.clone().sender;
    let previous_sender = env.contract.address;

    let parsed_path = parse_path_direct(
        recipient.clone(),
        message.clone(),
        info.funds.clone(),
        deps.storage,
        reply_on.clone(),
        exit_at_error,
        gas_limit,
    )?;
    // If parsed path yields a SubMsg, it means that the recipient is on another chain
    if let Some(msg) = parsed_path {
        Ok(Response::default()
            .add_submessage(msg)
            .add_attribute("action", "handle_amp_direct")
            .add_attribute("recipient", recipient)
            .add_attribute("message", message.to_string()))
    } else {
        let amp_pkt = AMPPkt::new(
            origin,
            previous_sender,
            vec![
                AMPMsg::new(recipient.clone(), message.clone(), Some(info.clone().funds))
                    .with_config(AMPMsgConfig::new(reply_on, exit_at_error, gas_limit)),
            ],
        );
        Ok(Response::default()
            .add_submessage(SubMsg::new(WasmMsg::Execute {
                contract_addr: recipient.clone().into(),
                msg: to_binary(&ExecuteMsg::AMPReceive(amp_pkt))?,
                funds: info.funds,
            }))
            .add_attribute("action", "handle_amp_direct")
            .add_attribute("recipient", recipient)
            .add_attribute("message", message.to_string()))
    }
}

pub fn handle_amp_message(
    _deps: DepsMut,
    env: Env,
    info: MessageInfo,
    message: AMPMsg,
) -> Result<Response, ContractError> {
    let origin = info.clone().sender;
    let previous_sender = env.contract.address;

    let amp_pkt = AMPPkt::new(origin.clone(), previous_sender, vec![message.clone()]);
    Ok(Response::default()
        .add_submessage(SubMsg::new(WasmMsg::Execute {
            contract_addr: message.clone().recipient.into(),
            msg: to_binary(&ExecuteMsg::AMPReceive(amp_pkt))?,
            funds: info.funds,
        }))
        .add_attribute("action", "handle_amp_message")
        .add_attribute("recipient", message.recipient)
        .add_attribute("message", message.message.to_string())
        .add_attribute("message", message.message.to_string())
        .add_attribute("origin", origin.to_string()))
}

pub fn handle_amp_direct_no_ctx(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: AndrAddr,
    message: Binary,
) -> Result<Response, ContractError> {
    let origin = info.clone().sender;
    let previous_sender = env.contract.address;

    let parsed_path = parse_path_direct_no_ctx(
        recipient.clone(),
        message.clone(),
        info.funds.clone(),
        deps.storage,
    )?;
    // If parsed path yields a SubMsg, it means that the recipient is on another chain
    if let Some(msg) = parsed_path {
        Ok(Response::default()
            .add_submessage(msg)
            .add_attribute("action", "handle_amp_direct_no_ctx")
            .add_attribute("recipient", recipient)
            .add_attribute("message", message.to_string()))
    } else {
        let amp_pkt = AMPPkt::new(
            origin,
            previous_sender,
            vec![AMPMsg::new(
                recipient.clone(),
                message.clone(),
                Some(info.clone().funds),
            )],
        );
        Ok(Response::default()
            .add_submessage(SubMsg::new(WasmMsg::Execute {
                contract_addr: recipient.clone().into(),
                msg: to_binary(&ExecuteMsg::AMPReceive(amp_pkt))?,
                funds: info.funds,
            }))
            .add_attribute("action", "handle_amp_direct_no_ctx")
            .add_attribute("recipient", recipient)
            .add_attribute("message", message.to_string()))
    }
}

pub fn handle_amp_packet(
    execute_env: ExecuteEnv,
    packet: AMPPkt,
) -> Result<Response, ContractError> {
    ensure!(
        query_verify_address(
            execute_env.deps.as_ref(),
            execute_env.info.sender.to_string(),
        )? || packet.ctx.get_origin() == execute_env.info.sender,
        ContractError::Unauthorized {}
    );
    ensure!(
        packet.ctx.id == 0,
        ContractError::InvalidPacket {
            error: Some("Packet ID cannot be provided from outside the Kernel".into())
        }
    );

    let mut res = Response::default();
    ensure!(
        !packet.messages.is_empty(),
        ContractError::InvalidPacket {
            error: Some("No messages supplied".to_string())
        }
    );
    for message in packet.messages {
        if let Some(protocol) = message.recipient.get_protocol() {
            match protocol {
                "ibc" => {
                    let bridge_addr =
                        KERNEL_ADDRESSES.may_load(execute_env.deps.storage, IBC_BRIDGE)?;
                    if let Some(bridge_addr) = bridge_addr {
                        if let Some(chain) = message.recipient.get_chain() {
                            let msg = IBCBridgeExecMsg::SendMessage {
                                chain: chain.to_string(),
                                recipient: AndrAddr::from_string(message.recipient.get_raw_path()),
                                message: message.message.clone(),
                            };
                            let cosmos_msg =
                                wasm_execute(bridge_addr.clone(), &msg, message.funds.clone())?;
                            res = res
                                .add_submessage(SubMsg::reply_always(cosmos_msg, 1))
                                .add_attribute("action", "handle_amp_packet")
                                .add_attribute("recipient", message.recipient)
                                .add_attribute("message", message.message.to_string());
                        } else {
                            return Err(ContractError::InvalidPacket {
                                error: Some("Chain not provided".to_string()),
                            });
                        }
                    } else {
                        return Err(ContractError::InvalidPacket {
                            error: Some("IBC not enabled in kernel".to_string()),
                        });
                    }
                }
                &_ => panic!("Invalid protocol"),
            }
        } else {
            let recipient_addr = message
                .recipient
                .get_raw_address(&execute_env.deps.as_ref())?;
            let msg = message.message;
            if Binary::default() == msg {
                ensure!(
                    !message.funds.is_empty(),
                    ContractError::InvalidPacket {
                        error: Some("No message or funds supplied".to_string())
                    }
                );

                // The message is a bank message
                let sub_msg = BankMsg::Send {
                    to_address: recipient_addr.to_string(),
                    amount: message.funds.clone(),
                };

                let origin = packet.ctx.get_origin();
                let previous_sender = execute_env.env.contract.address.to_string();

                let amp_msg = AMPMsg::new(
                    recipient_addr.clone(),
                    to_binary(&sub_msg)?,
                    Some(vec![message.funds[0].clone()]),
                );

                let new_packet = AMPPkt::new(origin, previous_sender, vec![amp_msg]);

                let msg = to_binary(&ExecuteMsg::AMPReceive(new_packet))?;

                res = res
                    .add_submessage(SubMsg::reply_on_error(
                        CosmosMsg::Wasm(WasmMsg::Execute {
                            contract_addr: recipient_addr.to_string(),
                            msg,
                            funds: vec![message.funds[0].clone()],
                        }),
                        1,
                    ))
                    // .add_submessage(SubMsg::reply_on_error(CosmosMsg::Bank(sub_msg), 1))
                    .add_attributes(vec![
                        attr("recipient", recipient_addr),
                        attr("bank_send_amount", message.funds[0].to_string()),
                    ]);
            } else {
                let sub_msg = WasmMsg::Execute {
                    contract_addr: recipient_addr.to_string(),
                    msg,
                    funds: message.funds.clone(),
                };

                let origin = packet.ctx.get_origin();
                let previous_sender = execute_env.env.contract.address.to_string();

                let amp_msg = AMPMsg::new(
                    recipient_addr.clone(),
                    to_binary(&sub_msg)?,
                    Some(vec![message.funds[0].clone()]),
                );

                let new_packet = AMPPkt::new(origin, previous_sender, vec![amp_msg]);

                let msg = to_binary(&ExecuteMsg::AMPReceive(new_packet))?;

                // TODO: ADD ID
                res = res
                    .add_submessage(SubMsg::reply_on_error(
                        CosmosMsg::Wasm(WasmMsg::Execute {
                            contract_addr: recipient_addr.to_string(),
                            msg,
                            funds: vec![message.funds[0].clone()],
                        }),
                        1,
                    ))
                    // .add_submessage(SubMsg::reply_on_error(CosmosMsg::Wasm(sub_msg), 1))
                    .add_attributes(vec![attr("recipient", recipient_addr)]);
            }
        }
    }

    Ok(res.add_attribute("action", "handle_amp_packet"))
}

fn upsert_key_address(
    execute_env: ExecuteEnv,
    key: String,
    value: String,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    ensure!(
        contract.is_contract_owner(execute_env.deps.storage, execute_env.info.sender.as_str())?,
        ContractError::Unauthorized {}
    );

    // Updates to new value
    if KERNEL_ADDRESSES.has(execute_env.deps.storage, &key) {
        KERNEL_ADDRESSES.remove(execute_env.deps.storage, &key)
    }

    KERNEL_ADDRESSES.save(
        execute_env.deps.storage,
        &key,
        &execute_env.deps.api.addr_validate(&value)?,
    )?;

    Ok(Response::default().add_attributes(vec![
        attr("action", "upsert_key_address"),
        attr("key", key),
        attr("value", value),
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
    StdError::generic_err(format!("Semver: {err}"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::KeyAddress { key } => encode_binary(&query_key_address(deps, key)?),
        QueryMsg::VerifyAddress { address } => encode_binary(&query_verify_address(deps, address)?),
    }
}

fn query_key_address(deps: Deps, key: String) -> Result<Addr, ContractError> {
    Ok(KERNEL_ADDRESSES.load(deps.storage, &key)?)
}

fn query_verify_address(deps: Deps, address: String) -> Result<bool, ContractError> {
    let db_address = KERNEL_ADDRESSES.load(deps.storage, ADO_DB_KEY)?;
    let contract_info = deps.querier.query_wasm_contract_info(address)?;

    let ado_type = AOSQuerier::ado_type_getter(&deps.querier, &db_address, contract_info.code_id)?;
    Ok(ado_type.is_some())
}
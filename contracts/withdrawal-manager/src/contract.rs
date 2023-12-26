use cosmwasm_std::{
    attr, ensure_eq, entry_point, to_json_binary, Attribute, BankMsg, Binary, Coin, CosmosMsg,
    Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;
use cw721::NftInfoResponse;
use lido_staking_base::{
    helpers::answer::response,
    msg::{
        withdrawal_manager::{ExecuteMsg, InstantiateMsg, QueryMsg},
        withdrawal_voucher::Extension,
    },
    state::{
        core::{UnbondBatch, UnbondBatchStatus},
        withdrawal_manager::{Config, Cw721ReceiveMsg, CONFIG},
    },
};
use neutron_sdk::bindings::{msg::NeutronMsg, query::NeutronQuery};

use crate::error::{ContractError, ContractResult};
const CONTRACT_NAME: &str = concat!("crates.io:lido-staking__", env!("CARGO_PKG_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response<NeutronMsg>> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let attrs: Vec<Attribute> = vec![
        attr("action", "instantiate"),
        attr("owner", &msg.owner),
        attr("core_contract", &msg.core_contract),
        attr("voucher_contract", &msg.voucher_contract),
        attr("base_denom", &msg.base_denom),
    ];
    CONFIG.save(
        deps.storage,
        &Config {
            core_contract: msg.core_contract,
            withdrawal_voucher_contract: msg.voucher_contract,
            base_denom: msg.base_denom,
            owner: msg.owner,
        },
    )?;
    Ok(response("instantiate", CONTRACT_NAME, attrs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps<NeutronQuery>, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&CONFIG.load(deps.storage)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut<NeutronQuery>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response<NeutronMsg>> {
    match msg {
        ExecuteMsg::UpdateConfig {
            owner,
            core_contract,
            voucher_contract,
        } => execute_update_config(deps, info, owner, core_contract, voucher_contract),
        ExecuteMsg::ReceiveNft(msg) => execute_receive_nft(deps, env, info, msg),
    }
}

fn execute_update_config(
    deps: DepsMut<NeutronQuery>,
    info: MessageInfo,
    owner: Option<String>,
    core_contract: Option<String>,
    voucher_contract: Option<String>,
) -> ContractResult<Response<NeutronMsg>> {
    let mut config = CONFIG.load(deps.storage)?;
    let mut attrs: Vec<Attribute> = vec![attr("action", "update_config")];
    if let Some(owner) = owner {
        if info.sender != config.owner {
            return Err(ContractError::Unauthorized {});
        }
        attrs.push(attr("owner", &owner));
        config.owner = owner;
    }
    if let Some(core_contract) = core_contract {
        attrs.push(attr("core_contract", &core_contract));
        config.core_contract = core_contract;
    }
    if let Some(voucher_contract) = voucher_contract {
        attrs.push(attr("voucher_contract", &voucher_contract));
        config.withdrawal_voucher_contract = voucher_contract;
    }
    CONFIG.save(deps.storage, &config)?;
    Ok(response("update_config", CONTRACT_NAME, attrs))
}

fn execute_receive_nft(
    deps: DepsMut<NeutronQuery>,
    _env: Env,
    info: MessageInfo,
    msg: Cw721ReceiveMsg,
) -> ContractResult<Response<NeutronMsg>> {
    let mut attrs = vec![attr("action", "receive_nft")];
    let config = CONFIG.load(deps.storage)?;
    ensure_eq!(
        config.withdrawal_voucher_contract,
        info.sender,
        ContractError::Unauthorized {}
    );
    let voucher: NftInfoResponse<Extension> = deps.querier.query_wasm_smart(
        config.withdrawal_voucher_contract,
        &lido_staking_base::msg::withdrawal_voucher::QueryMsg::NftInfo {
            token_id: msg.token_id.clone(),
        },
    )?;
    let voucher_extention = voucher.extension.ok_or_else(|| ContractError::InvalidNFT {
        reason: "extension is not set".to_string(),
    })?;

    let batch_id =
        voucher_extention
            .batch_id
            .parse::<u128>()
            .map_err(|_| ContractError::InvalidNFT {
                reason: "invalid batch_id".to_string(),
            })?;

    let unbond_batch: UnbondBatch = deps.querier.query_wasm_smart(
        &config.core_contract,
        &lido_staking_base::msg::core::QueryMsg::UnbondBatch {
            batch_id: batch_id.into(),
        },
    )?;
    ensure_eq!(
        unbond_batch.status,
        UnbondBatchStatus::Unbonded,
        ContractError::BatchIsNotUnbonded {}
    );
    let payout_amount = unbond_batch
        .slashing_effect
        .ok_or(ContractError::BatchSlashingEffectIsEmpty {})?
        * voucher_extention.expected_amount;

    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: msg.sender,
        amount: vec![Coin {
            denom: config.base_denom,
            amount: payout_amount,
        }],
    });
    attrs.push(attr("batch_id", batch_id.to_string()));
    attrs.push(attr("payout_amount", payout_amount.to_string()));
    Ok(response("execute-receive_nft", CONTRACT_NAME, attrs).add_message(msg))
}
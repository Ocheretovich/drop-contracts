use crate::contract::Puppeteer;
use cosmwasm_std::{
    testing::{mock_env, mock_info},
    Addr, Binary, Coin, CosmosMsg, DepsMut, Event, Response, SubMsg, Uint128,
};
use drop_helpers::testing::mock_dependencies;
use drop_puppeteer_base::state::{PuppeteerBase, ReplyMsg};
use drop_staking_base::state::puppeteer::{Config, KVQueryType};
use drop_staking_base::{msg::puppeteer::InstantiateMsg, state::puppeteer::ConfigOptional};
use neutron_sdk::bindings::{
    msg::{IbcFee, NeutronMsg},
    query::NeutronQuery,
};
use prost::Message;

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: Some("owner".to_string()),
        connection_id: "connection_id".to_string(),
        port_id: "port_id".to_string(),
        update_period: 60u64,
        remote_denom: "remote_denom".to_string(),
        allowed_senders: vec!["allowed_sender".to_string()],
        transfer_channel_id: "transfer_channel_id".to_string(),
        sdk_version: "0.45.0".to_string(),
    };
    let env = mock_env();
    let res =
        crate::contract::instantiate(deps.as_mut(), env, mock_info("sender", &[]), msg).unwrap();
    assert_eq!(res, Response::new());
    let puppeteer_base = Puppeteer::default();
    let config = puppeteer_base.config.load(deps.as_ref().storage).unwrap();
    assert_eq!(config, get_base_config());
}

#[test]
fn test_update_config() {
    let mut deps = mock_dependencies(&[]);
    let puppeteer_base = Puppeteer::default();
    puppeteer_base
        .config
        .save(deps.as_mut().storage, &get_base_config())
        .unwrap();
    let msg = drop_staking_base::msg::puppeteer::ExecuteMsg::UpdateConfig {
        new_config: ConfigOptional {
            update_period: Some(121u64),
            remote_denom: Some("new_remote_denom".to_string()),
            allowed_senders: Some(vec![Addr::unchecked("new_allowed_sender")]),
            transfer_channel_id: Some("new_transfer_channel_id".to_string()),
            connection_id: Some("new_connection_id".to_string()),
            port_id: Some("new_port_id".to_string()),
            proxy_address: Some(Addr::unchecked("new_proxy_address")),
            sdk_version: Some("0.47.0".to_string()),
        },
    };
    let deps_mut = deps.as_mut();
    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some("owner")).unwrap();
    let env = mock_env();
    let res =
        crate::contract::execute(deps.as_mut(), env, mock_info("owner", &[]), msg.clone()).unwrap();
    assert_eq!(
        res,
        Response::new().add_event(
            Event::new("crates.io:drop-neutron-contracts__drop-puppeteer-config_update")
                .add_attributes(vec![
                    ("proxy_address", "new_proxy_address"),
                    ("remote_denom", "new_remote_denom"),
                    ("connection_id", "new_connection_id"),
                    ("port_id", "new_port_id"),
                    ("update_period", "121"),
                    ("allowed_senders", "1"),
                    ("transfer_channel_id", "new_transfer_channel_id"),
                    ("sdk_version", "0.47.0"),
                ])
        )
    );
    let config = puppeteer_base.config.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        config,
        Config {
            port_id: "new_port_id".to_string(),
            connection_id: "new_connection_id".to_string(),
            update_period: 121u64,
            remote_denom: "new_remote_denom".to_string(),
            allowed_senders: vec![Addr::unchecked("new_allowed_sender")],
            transfer_channel_id: "new_transfer_channel_id".to_string(),
            sdk_version: "0.47.0".to_string(),
            proxy_address: Some(Addr::unchecked("new_proxy_address")),
        }
    );
}

#[test]
fn test_execute_delegate() {
    let mut deps = mock_dependencies(&[]);
    let pupeteer_base = base_init(&mut deps.as_mut());
    let msg = drop_staking_base::msg::puppeteer::ExecuteMsg::Delegate {
        items: vec![("valoper1".to_string(), Uint128::from(1000u128))],
        reply_to: "some_reply_to".to_string(),
        timeout: Some(100u64),
    };
    let env = mock_env();
    let res = crate::contract::execute(
        deps.as_mut(),
        env,
        mock_info("not_allowed_sender", &[]),
        msg.clone(),
    );
    assert_eq!(
        res.unwrap_err(),
        drop_puppeteer_base::error::ContractError::Std(cosmwasm_std::StdError::GenericErr {
            msg: "Sender is not allowed".to_string()
        })
    );
    let res = crate::contract::execute(
        deps.as_mut(),
        mock_env(),
        mock_info("allowed_sender", &[]),
        msg,
    )
    .unwrap();
    let msg = cosmos_sdk_proto::cosmos::staking::v1beta1::MsgDelegate {
        delegator_address: "ica_address".to_string(),
        validator_address: "valoper1".to_string(),
        amount: Some(cosmos_sdk_proto::cosmos::base::v1beta1::Coin {
            denom: "remote_denom".to_string(),
            amount: "1000".to_string(),
        }),
    };
    let mut buf = Vec::with_capacity(msg.encoded_len());
    msg.encode(&mut buf).unwrap();
    let any_msg = neutron_sdk::bindings::types::ProtobufAny {
        type_url: "/cosmos.staking.v1beta1.MsgDelegate".to_string(),
        value: Binary::from(buf),
    };
    assert_eq!(
        res,
        Response::new().add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Custom(NeutronMsg::submit_tx(
                "connection_id".to_string(),
                "DROP".to_string(),
                vec![any_msg],
                "".to_string(),
                100u64,
                get_standard_fees()
            )),
            ReplyMsg::SudoPayload.to_reply_id()
        ))
    );
    let tx_state = pupeteer_base.tx_state.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        tx_state,
        drop_puppeteer_base::state::TxState {
            seq_id: None,
            status: drop_puppeteer_base::state::TxStateStatus::InProgress,
            reply_to: Some("some_reply_to".to_string()),
            transaction: Some(drop_puppeteer_base::msg::Transaction::Delegate {
                interchain_account_id: "DROP".to_string(),
                denom: "remote_denom".to_string(),
                items: vec![("valoper1".to_string(), Uint128::from(1000u128))]
            })
        }
    );
}

#[test]
fn test_execute_undelegate() {
    let mut deps = mock_dependencies(&[]);
    let puppeteer_base = base_init(&mut deps.as_mut());
    let msg = drop_staking_base::msg::puppeteer::ExecuteMsg::Undelegate {
        batch_id: 0u128,
        items: vec![("valoper1".to_string(), Uint128::from(1000u128))],
        reply_to: "some_reply_to".to_string(),
        timeout: Some(100u64),
    };
    let env = mock_env();
    let res = crate::contract::execute(
        deps.as_mut(),
        env,
        mock_info("not_allowed_sender", &[]),
        msg.clone(),
    );
    assert_eq!(
        res.unwrap_err(),
        drop_puppeteer_base::error::ContractError::Std(cosmwasm_std::StdError::GenericErr {
            msg: "Sender is not allowed".to_string()
        })
    );
    let res = crate::contract::execute(
        deps.as_mut(),
        mock_env(),
        mock_info("allowed_sender", &[]),
        msg,
    )
    .unwrap();
    let msg = cosmos_sdk_proto::cosmos::staking::v1beta1::MsgUndelegate {
        delegator_address: "ica_address".to_string(),
        validator_address: "valoper1".to_string(),
        amount: Some(cosmos_sdk_proto::cosmos::base::v1beta1::Coin {
            denom: "remote_denom".to_string(),
            amount: "1000".to_string(),
        }),
    };
    let mut buf = Vec::with_capacity(msg.encoded_len());
    msg.encode(&mut buf).unwrap();
    let any_msg = neutron_sdk::bindings::types::ProtobufAny {
        type_url: "/cosmos.staking.v1beta1.MsgUndelegate".to_string(),
        value: Binary::from(buf),
    };
    assert_eq!(
        res,
        Response::new().add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Custom(NeutronMsg::submit_tx(
                "connection_id".to_string(),
                "DROP".to_string(),
                vec![any_msg],
                "".to_string(),
                100u64,
                get_standard_fees()
            )),
            ReplyMsg::SudoPayload.to_reply_id()
        ))
    );
    let tx_state = puppeteer_base.tx_state.load(deps.as_ref().storage).unwrap();
    assert_eq!(
        tx_state,
        drop_puppeteer_base::state::TxState {
            seq_id: None,
            status: drop_puppeteer_base::state::TxStateStatus::InProgress,
            reply_to: Some("some_reply_to".to_string()),
            transaction: Some(drop_puppeteer_base::msg::Transaction::Undelegate {
                batch_id: 0u128,
                interchain_account_id: "DROP".to_string(),
                denom: "remote_denom".to_string(),
                items: vec![("valoper1".to_string(), Uint128::from(1000u128))]
            })
        }
    );
}

fn get_base_config() -> Config {
    Config {
        port_id: "port_id".to_string(),
        connection_id: "connection_id".to_string(),
        update_period: 60u64,
        remote_denom: "remote_denom".to_string(),
        allowed_senders: vec![Addr::unchecked("allowed_sender")],
        transfer_channel_id: "transfer_channel_id".to_string(),
        sdk_version: "0.45.0".to_string(),
        proxy_address: None,
    }
}

fn base_init(
    deps_mut: &mut DepsMut<NeutronQuery>,
) -> PuppeteerBase<'static, drop_staking_base::state::puppeteer::Config, KVQueryType> {
    let puppeteer_base = Puppeteer::default();
    cw_ownable::initialize_owner(deps_mut.storage, deps_mut.api, Some("owner")).unwrap();
    puppeteer_base
        .config
        .save(deps_mut.storage, &get_base_config())
        .unwrap();
    puppeteer_base
        .ica
        .set_address(deps_mut.storage, "ica_address")
        .unwrap();
    puppeteer_base
        .ibc_fee
        .save(deps_mut.storage, &get_standard_fees())
        .unwrap();
    puppeteer_base
}

fn get_standard_fees() -> IbcFee {
    IbcFee {
        recv_fee: vec![Coin::new(1000u128, "local_denom")],
        ack_fee: vec![Coin::new(2000u128, "local_denom")],
        timeout_fee: vec![Coin::new(3000u128, "local_denom")],
    }
}
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

use crate::state::{CodeIds, RemoteOpts, State};

#[cw_serde]
pub struct InstantiateMsg {
    pub code_ids: CodeIds,
    pub remote_opts: RemoteOpts,
    pub salt: String,
    pub subdenom: String,
}

#[cw_serde]
pub enum CallbackMsg {
    PostInit {},
}
#[cw_serde]
pub struct CoreParams {
    pub idle_min_interval: u64,
    pub puppeteer_timeout: u64,
    pub unbonding_period: u64,
    pub unbonding_safe_period: u64,
    pub unbond_batch_switch_time: u64,
}

#[cw_serde]
pub struct FeesMsg {
    pub recv_fee: Uint128,
    pub ack_fee: Uint128,
    pub timeout_fee: Uint128,
    pub register_fee: Uint128,
}

#[cw_serde]
pub enum UpdateConfigMsg {
    Core(lido_staking_base::state::core::ConfigOptional),
    ValidatorsSet(lido_staking_base::state::validatorset::ConfigOptional),
    PuppeteerFees(FeesMsg),
}

#[cw_serde]
pub enum ProxyMsg {
    ValidatorSet(ValidatorSetMsg),
}

#[cw_serde]
pub enum ValidatorSetMsg {
    UpdateValidators {
        validators: Vec<lido_staking_base::msg::validatorset::ValidatorData>,
    },
    UpdateValidator {
        validator: lido_staking_base::msg::validatorset::ValidatorData,
    },
    UpdateValidatorInfo {
        validators: Vec<lido_staking_base::msg::validatorset::ValidatorInfoUpdate>,
    },
}

#[cw_serde]
pub enum ExecuteMsg {
    Init {
        base_denom: String,
        core_params: CoreParams,
    },
    Callback(CallbackMsg),
    UpdateConfig(UpdateConfigMsg),
    Proxy(ProxyMsg),
}
#[cw_serde]
pub enum MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(State)]
    State {},
}

#![cfg_attr(not(feature = "std"), no_std)]
use concordium_std::*;

use crate::common::{Error, Receiver, WithdrawParams};

#[derive(DeserialWithState, Serial)]
#[concordium(state_parameter = "S")]
pub struct State<S: HasStateApi> {
    balances: StateMap<Address, Amount, S>,
}

impl<S: HasStateApi> State<S> {
    fn new(state_builder: &mut StateBuilder<S>) -> Self {
        Self {
            balances: state_builder.new_map(),
        }
    }

    fn get_view(&self) -> Vec<(Address, Amount)> {
        self.balances
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

#[init(contract = "reentrance", parameter = "()")]
fn init<S: HasStateApi>(
    _ctx: &impl HasInitContext,
    state_builder: &mut StateBuilder<S>,
) -> InitResult<State<S>> {
    let state = State::new(state_builder);
    Ok(state)
}

#[receive(
    contract = "reentrance",
    name = "deposit",
    parameter = "()",
    mutable,
    payable
)]
fn contract_deposit<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State<S>, StateApiType = S>,
    amount: Amount,
) -> Result<(), Error> {
    let sender = ctx.sender();
    let state = host.state_mut();
    let mut sender_balance = state
        .balances
        .entry(sender)
        .or_insert_with(|| Amount::zero());
    *sender_balance += amount;

    Ok(())
}

#[receive(
    contract = "reentrance",
    name = "view",
    parameter = "()",
    return_value = "Vec<(Address, Amount)>"
)]
fn contract_view<S: HasStateApi>(
    _ctx: &impl HasReceiveContext,
    host: &impl HasHost<State<S>, StateApiType = S>,
) -> Result<Vec<(Address, Amount)>, Error> {
    Ok(host.state().get_view())
}

#[receive(
    contract = "reentrance",
    name = "withdraw",
    parameter = "WithdrawParams",
    error = "Error",
    mutable
)]
fn contract_withdraw<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State<S>, StateApiType = S>,
) -> Result<(), Error> {
    let params: WithdrawParams = ctx.parameter_cursor().get()?;
    let state = host.state();
    let address = params.get_address();

    let deposited = state
        .balances
        .get(&address)
        .ok_or(Error::NothingDeposited)?;

    let amount_to_transfer = deposited.to_owned();

    match params.receiver {
        Receiver::Account(address) => host.invoke_transfer(&address, amount_to_transfer)?,
        Receiver::Contract(address, function) => {
            host.invoke_contract_raw(
                &address,
                Parameter::empty(),
                function.as_entrypoint_name(),
                amount_to_transfer,
            )?;
        }
    };

    host.state_mut().balances.remove(&address);

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::common::tests::*;
    use anyhow::Result;

    #[test]
    fn test_reentrance_deposit_from_account() -> Result<()> {
        reentrance_deposit_validation(Victim::Reentrance)?;
        Ok(())
    }

    #[test]
    fn test_reentrance_withdraw() -> Result<()> {
        reentrance_withdraw_validation(Victim::Reentrance)?;
        Ok(())
    }
}

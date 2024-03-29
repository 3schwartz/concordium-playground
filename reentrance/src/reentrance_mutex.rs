#![cfg_attr(not(feature = "std"), no_std)]
use concordium_std::*;

use crate::common::{Error, Receiver, WithdrawParams};

#[derive(DeserialWithState, Serial)]
#[concordium(state_parameter = "S")]
pub struct State<S = StateApi> {
    balances: StateMap<Address, Amount, S>,
    lock: bool,
}

impl State {
    fn new(state_builder: &mut StateBuilder) -> Self {
        Self {
            balances: state_builder.new_map(),
            lock: false,
        }
    }

    fn get_view(&self) -> Vec<(Address, Amount)> {
        self.balances
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

#[init(contract = "reentrance_mutex", parameter = "()")]
fn init(_ctx: &impl HasInitContext, state_builder: &mut StateBuilder) -> InitResult<State> {
    let state = State::new(state_builder);
    Ok(state)
}

#[receive(
    contract = "reentrance_mutex",
    name = "deposit",
    parameter = "()",
    mutable,
    payable
)]
fn contract_deposit(
    ctx: &ReceiveContext,
    host: &mut Host<State>,
    amount: Amount,
) -> Result<(), Error> {
    let sender = ctx.sender();
    let state = host.state_mut();
    state
        .balances
        .entry(sender)
        .and_modify(|bal| *bal += amount)
        .or_insert(amount);

    Ok(())
}

#[receive(
    contract = "reentrance_mutex",
    name = "view",
    parameter = "()",
    return_value = "Vec<(Address, Amount)>"
)]
fn contract_view(
    _ctx: &ReceiveContext,
    host: &Host<State>,
) -> Result<Vec<(Address, Amount)>, Error> {
    Ok(host.state().get_view())
}

#[receive(
    contract = "reentrance_mutex",
    name = "withdraw",
    parameter = "WithdrawParams",
    error = "Error",
    mutable
)]
fn contract_withdraw(ctx: &ReceiveContext, host: &mut Host<State>) -> Result<(), Error> {
    ensure!(!host.state().lock, Error::LockError);

    let params: WithdrawParams = ctx.parameter_cursor().get()?;
    let state = host.state();
    let address = params.get_address();

    let deposited = state
        .balances
        .get(&address)
        .ok_or(Error::NothingDeposited)?;

    let amount_to_transfer = deposited.to_owned();

    host.state_mut().lock = true;

    match params.receiver {
        Receiver::Account(address) => host.invoke_transfer(&address, amount_to_transfer)?,
        Receiver::Contract(address, function) => {
            host.invoke_contract_raw_read_only(
                &address,
                Parameter::empty(),
                function.as_entrypoint_name(),
                amount_to_transfer,
            )?;
        }
    };

    host.state_mut().lock = false;

    host.state_mut().balances.remove(&address);

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::common::tests::*;
    use anyhow::Result;

    #[test]
    fn test_reentrance_deposit_from_account() -> Result<()> {
        reentrance_deposit_validation(Victim::RentranceMutex)?;
        Ok(())
    }

    #[test]
    fn test_reentrance_withdraw() -> Result<()> {
        reentrance_withdraw_validation(Victim::RentranceMutex)?;
        Ok(())
    }
}

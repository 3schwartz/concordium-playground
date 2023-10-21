#![cfg_attr(not(feature = "std"), no_std)]
use concordium_std::*;

use crate::common::{Error, Receiver, WithdrawParams};

#[derive(Serialize, SchemaType)]
pub struct SaintState {
    other: ContractAddress,
}

#[init(contract = "saint", parameter = "ContractAddress")]
fn contract_saint(ctx: &InitContext, _state_builder: &mut StateBuilder) -> InitResult<SaintState> {
    let other: ContractAddress = ctx.parameter_cursor().get()?;
    Ok(SaintState { other })
}

#[receive(
    contract = "saint",
    name = "deposit",
    parameter = "()",
    mutable,
    payable
)]
fn contract_saint_deposit(
    ctx: &ReceiveContext,
    host: &mut Host<SaintState>,
    amount: Amount,
) -> Result<(), Error> {
    ensure!(
        ctx.sender().matches_account(&ctx.owner()),
        Error::OwnerError
    );
    let other = host.state().other;

    host.invoke_contract_raw(
        &other,
        Parameter::empty(),
        EntrypointName::new_unchecked("deposit"),
        amount,
    )?;
    Ok(())
}

#[receive(
    contract = "saint",
    name = "withdraw",
    parameter = "()",
    error = "Error",
    mutable
)]
fn contract_saint_attack(ctx: &ReceiveContext, host: &mut Host<SaintState>) -> Result<(), Error> {
    ensure!(
        ctx.sender().matches_account(&ctx.owner()),
        Error::OwnerError
    );
    let other = host.state().other;

    let params = WithdrawParams {
        receiver: Receiver::Contract(
            ctx.self_address(),
            OwnedEntrypointName::new_unchecked("receive".to_string()),
        ),
    };

    host.invoke_contract_raw(
        &other,
        Parameter::new_unchecked(&to_bytes(&params)),
        EntrypointName::new_unchecked("withdraw"),
        Amount::zero(),
    )?;
    Ok(())
}

#[receive(
    contract = "saint",
    name = "receive",
    parameter = "()",
    error = "Error",
    mutable,
    payable
)]
fn contract_saint_receive(
    ctx: &ReceiveContext,
    host: &mut Host<SaintState>,
    _amount: Amount,
) -> Result<(), Error> {
    ensure!(
        ctx.sender().matches_contract(&host.state().other),
        Error::WrongVictimAddressErrror
    );

    Ok(())
}

#[receive(
    contract = "saint",
    name = "transfer",
    parameter = "()",
    error = "Error",
    mutable
)]
fn contract_saint_transfer(ctx: &ReceiveContext, host: &mut Host<SaintState>) -> Result<(), Error> {
    ensure!(
        ctx.sender().matches_account(&ctx.owner()),
        Error::OwnerError
    );

    host.invoke_transfer(&&ctx.owner(), host.self_balance())?;

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::common::tests::*;
    use anyhow::Result;
    use concordium_smart_contract_testing::*;

    #[test]
    fn test_receive_reentrance_mutex() -> Result<()> {
        test_reentrance_skeleton(Victim::RentranceMutex)?;
        Ok(())
    }

    #[test]
    fn test_receive_reentrance_checks_effects_interactions() -> Result<()> {
        test_reentrance_skeleton(Victim::ReentranceChecksEffectsInteractions)?;
        Ok(())
    }

    #[test]
    fn test_receive_reentrance_readonly() -> Result<()> {
        test_reentrance_skeleton(Victim::ReentraceReadonly)?;
        Ok(())
    }

    #[test]
    fn test_receive_reentrance() -> Result<()> {
        test_reentrance_skeleton(Victim::Reentrance)?;
        Ok(())
    }

    fn test_reentrance_skeleton(victim: Victim) -> Result<()> {
        let (mut chain, contracts) = setup_with_victim(victim)?;
        let reentrance_contract = contracts.reentrance;
        let saint = contracts.saint;

        let contract_name = victim.name();

        const TO_TRANSFER: Amount = Amount::from_ccd(42);
        // deposit from ACC other
        let _ = reentrace_deposit(
            contract_name,
            ACC_ADDR_OTHER,
            reentrance_contract.contract_address,
            TO_TRANSFER,
            &mut chain,
        )?;
        // deposit from ACC attacker
        let _ = reentrace_deposit(
            "saint",
            ACC_ADDR_OTHER,
            saint.contract_address,
            TO_TRANSFER,
            &mut chain,
        )?;

        // Act
        let withdraw_update = chain.contract_update(
            Signer::with_one_key(),
            ACC_ADDR_OTHER,
            Address::from(ACC_ADDR_OTHER),
            Energy::from(42_000),
            UpdateContractPayload {
                amount: Amount::zero(),
                address: saint.contract_address,
                receive_name: OwnedReceiveName::new_unchecked("saint.withdraw".to_string()),
                message: OwnedParameter::empty(),
            },
        )?;

        let saint_balance_before_transfer = chain.account_balance(ACC_ADDR_OTHER).unwrap();

        let transfer_update = chain.contract_update(
            Signer::with_one_key(),
            ACC_ADDR_OTHER,
            Address::from(ACC_ADDR_OTHER),
            Energy::from(42_000),
            UpdateContractPayload {
                amount: Amount::zero(),
                address: saint.contract_address,
                receive_name: OwnedReceiveName::new_unchecked("saint.transfer".to_string()),
                message: OwnedParameter::empty(),
            },
        )?;

        // Assert
        assert_eq!(
            chain
                .contract_balance(reentrance_contract.contract_address)
                .unwrap(),
            TO_TRANSFER
        );

        let saint_balance_after_transfer = chain.account_balance(ACC_ADDR_OTHER).unwrap();
        assert_eq!(
            saint_balance_after_transfer.available(),
            saint_balance_before_transfer.available() + TO_TRANSFER
                - transfer_update.transaction_fee
        );
        // This is only used later to get the energy used.
        println!(
            "Energy used withdraw - {} - {}",
            contract_name, withdraw_update.energy_used
        );
        Ok(())
    }
}

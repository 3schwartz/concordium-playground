#![cfg_attr(not(feature = "std"), no_std)]
use concordium_std::*;
use core::fmt::Debug;

#[derive(Deserial, Debug, PartialEq, Eq, Reject, Serial, SchemaType)]
pub enum Error {
    #[from(ParseError)]
    ParseParams,
    NothingDeposited,
    InvokeTransferError,
    InvokeContractError,
    OwnerError,
    WrongVictimAddressErrror,
    QueryContractBalanceError,
    LogError,
    LockError,
}

impl From<TransferError> for Error {
    fn from(_te: TransferError) -> Self {
        Self::InvokeTransferError
    }
}

impl<T> From<CallContractError<T>> for Error {
    fn from(_cce: CallContractError<T>) -> Self {
        Self::InvokeContractError
    }
}

impl From<QueryContractBalanceError> for Error {
    fn from(_value: QueryContractBalanceError) -> Self {
        Self::QueryContractBalanceError
    }
}

impl From<LogError> for Error {
    fn from(_value: LogError) -> Self {
        Self::LogError
    }
}

#[derive(Debug, Serialize, Clone, SchemaType)]
pub enum Receiver {
    Account(AccountAddress),
    Contract(ContractAddress, OwnedEntrypointName),
}

#[derive(Debug, Serialize, SchemaType)]
pub struct WithdrawParams {
    pub receiver: Receiver,
}

impl WithdrawParams {
    pub fn get_address(&self) -> Address {
        match self.receiver {
            Receiver::Account(account) => Address::from(account),
            Receiver::Contract(contract, _) => Address::from(contract),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use anyhow::Result;
    use concordium_smart_contract_testing::*;

    use crate::common::{WithdrawParams, Receiver};

    pub const REENTRANCE: &str = "reentrance";
    pub const REENTRANCE_READONLY: &str = "reentrance_readonly";
    pub const REENTRANCE_CHECKS_EFFECTS_INTERACTIONS: &str = "reentrance_checks_effects_interactions";
    pub const REENTRANCE_MUTEX: &str = "reentrance_mutex";

    pub const ACC_ADDR_OWNER: AccountAddress = AccountAddress([0u8; 32]);
    pub const ACC_ADDR_OTHER: AccountAddress = AccountAddress([1u8; 32]);
    pub const ACC_ADDR_ATTACKER: AccountAddress = AccountAddress([2u8; 32]);

    pub const ACC_INITIAL_BALANCE: Amount = Amount::from_ccd(42_000);

    pub struct Contracts {
        pub reentrance: ContractInitSuccess,
        pub attacker: ContractInitSuccess,
        pub saint: ContractInitSuccess,
    }

    #[derive(Clone, Copy)]
    pub enum Victim {
        Reentrance,
        ReentraceReadonly,
        ReentranceChecksEffectsInteractions,
        RentranceMutex,
    }

    impl Victim {
        pub fn name(&self) -> &str {
            match self {
                Victim::Reentrance => REENTRANCE,
                Victim::ReentraceReadonly => REENTRANCE_READONLY,
                Victim::ReentranceChecksEffectsInteractions => REENTRANCE_CHECKS_EFFECTS_INTERACTIONS,
                Victim::RentranceMutex => REENTRANCE_MUTEX,
            }
        }
    }

    pub fn setup_with_victim(victim: Victim) -> Result<(Chain, Contracts)> {
        let mut chain = Chain::new();
        let module = module_load_v1("concordium-out/module.wasm.v1").unwrap();
        chain.create_account(Account::new(ACC_ADDR_OWNER, ACC_INITIAL_BALANCE));
        chain.create_account(Account::new(ACC_ADDR_OTHER, ACC_INITIAL_BALANCE));
        chain.create_account(Account::new(ACC_ADDR_ATTACKER, ACC_INITIAL_BALANCE));

        let deployment = chain.module_deploy_v1(Signer::with_one_key(), ACC_ADDR_OWNER, module)?;
        let contract_name = victim.name();

        let reentrance = chain.contract_init(
            Signer::with_one_key(),
            ACC_ADDR_OWNER,
            Energy::from(10_000),
            InitContractPayload {
                amount: Amount::zero(),
                mod_ref: deployment.module_reference,
                init_name: OwnedContractName::new_unchecked(format!("init_{}", contract_name)),
                param: OwnedParameter::empty(),
            },
        )?;

        let attacker = chain.contract_init(
            Signer::with_one_key(),
            ACC_ADDR_ATTACKER,
            Energy::from(10_000),
            InitContractPayload {
                amount: Amount::zero(),
                mod_ref: deployment.module_reference,
                init_name: OwnedContractName::new_unchecked("init_attacker".to_string()),
                param: OwnedParameter::from_serial(&reentrance.contract_address)?,
            },
        )?;

        let saint = chain.contract_init(
            Signer::with_one_key(),
            ACC_ADDR_OTHER,
            Energy::from(10_000),
            InitContractPayload {
                amount: Amount::zero(),
                mod_ref: deployment.module_reference,
                init_name: OwnedContractName::new_unchecked("init_saint".to_string()),
                param: OwnedParameter::from_serial(&reentrance.contract_address)?,
            },
        )?;        

        return Ok((
            chain,
            Contracts {
                reentrance,
                attacker,
                saint,
            },
        ));
    }

    pub fn get_view(
        victim: Victim,
        chain: &Chain,
        contract: &ContractInitSuccess,
    ) -> Result<Vec<(Address, Amount)>> {
        let contract_name = victim.name();

        let view = chain.contract_invoke(
            ACC_ADDR_OWNER,
            Address::from(ACC_ADDR_OWNER),
            Energy::from(10_000),
            UpdateContractPayload {
                amount: Amount::zero(),
                address: contract.contract_address,
                receive_name: OwnedReceiveName::new_unchecked(format!("{}.view", contract_name)),                
                message: OwnedParameter::empty(),
            },
        )?;
        let view_state = from_bytes(&view.return_value)?;
        Ok(view_state)
    }

    pub fn reentrace_deposit(
        contract_name: &str,
        account_addr: AccountAddress,
        contract_addr: ContractAddress,
        to_transfer: Amount,
        chain: &mut Chain,
    ) -> Result<ContractInvokeSuccess, ContractInvokeError> {
        chain.contract_update(
            Signer::with_one_key(),
            account_addr,
            Address::from(account_addr),
            Energy::from(42_000),
            UpdateContractPayload {
                amount: to_transfer,
                address: contract_addr,
                receive_name: OwnedReceiveName::new_unchecked(format!("{}.deposit", contract_name)),
                message: OwnedParameter::empty(),
            },
        )
    }

    pub fn reentrance_withdraw_validation(victim: Victim) -> Result<()> {
        // Arrange
        let contract_name = victim.name();
        let (mut chain, contracts) = setup_with_victim(victim)?;
        let reentrance_contract = contracts.reentrance;

        const TO_TRANSFER: Amount = Amount::from_ccd(42);

        let _ = reentrace_deposit(
            contract_name,
            ACC_ADDR_OTHER,
            reentrance_contract.contract_address,
            TO_TRANSFER,
            &mut chain,
        )?;
        let view_before = get_view(victim, &chain, &reentrance_contract)?;

        let reentrance_balance_before = chain
            .contract_balance(reentrance_contract.contract_address)
            .unwrap();

        let params: WithdrawParams = WithdrawParams {
            receiver: Receiver::Account(ACC_ADDR_OTHER),
        };
        let other_balance_before = chain.account_balance(ACC_ADDR_OTHER).unwrap();

        // Act
        let withdraw_update = chain.contract_update(
            Signer::with_one_key(),
            ACC_ADDR_OTHER,
            Address::from(ACC_ADDR_OTHER),
            Energy::from(42_000),
            UpdateContractPayload {
                amount: Amount::zero(),
                address: reentrance_contract.contract_address,
                receive_name: OwnedReceiveName::new_unchecked(format!("{}.withdraw", contract_name)),
                message: OwnedParameter::from_serial(&params)?,
            },
        )?;

        let view_after = get_view(victim, &chain, &reentrance_contract)?;
        let reentrance_balance_after = chain
            .contract_balance(reentrance_contract.contract_address)
            .unwrap();
        let other_balance_after = chain.account_balance(ACC_ADDR_OTHER).unwrap();

        // Assert
        assert_eq!(reentrance_balance_before, TO_TRANSFER);
        assert_eq!(reentrance_balance_after, Amount::zero());
        assert_eq!(
            other_balance_after.available() - other_balance_before.available()
                + withdraw_update.transaction_fee,
            TO_TRANSFER
        );

        assert_eq!(view_before.len(), 1);
        let state = view_before[0];
        assert_eq!(state.1, TO_TRANSFER);
        assert_eq!(state.0, Address::from(ACC_ADDR_OTHER));

        assert_eq!(view_after.len(), 0);

        Ok(())
    }    

    pub fn reentrance_deposit_validation(victim: Victim) -> Result<()> {
        let (mut chain, contracts) = setup_with_victim(victim)?;
        let reentrance_contract = contracts.reentrance;

        const TO_TRANSFER: Amount = Amount::from_ccd(42);

        // Act
        let update = reentrace_deposit(
            REENTRANCE,
            ACC_ADDR_OWNER,
            reentrance_contract.contract_address,
            TO_TRANSFER,
            &mut chain,
        )?;

        let view = get_view(victim, &chain, &reentrance_contract)?;

        // Assert
        let balance_after = chain
            .contract_balance(reentrance_contract.contract_address)
            .unwrap();
        assert_eq!(balance_after, TO_TRANSFER);
        assert_eq!(balance_after, update.new_balance);
        assert_eq!(view.len(), 1);
        let state = view[0];
        assert_eq!(state.1, TO_TRANSFER);
        Ok(())
    }
}

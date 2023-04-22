#![cfg_attr(not(feature = "std"), no_std)]

use concordium_cis2::*;
use concordium_std::*;

const SUPPORTS_STANDARDS: [StandardIdentifier<'static>; 2] =
    [CIS0_STANDARD_IDENTIFIER, CIS2_STANDARD_IDENTIFIER];

type ContractTokenId = TokenIdU32;
type ContractTokenAmount = TokenAmountU64;

#[derive(Debug, Serialize, Clone, SchemaType)]
pub struct TokenMetadata {
    #[concordium(size_length = 2)]
    pub url: String,
    #[concordium(size_length = 2)]
    pub hash: String,
}

impl TokenMetadata {
    fn get_hash_as_bytes(&self) -> Option<[u8; 32]> {
        let mut hash_bytes: [u8; 32] = Default::default();
        let hex_res = hex::decode_to_slice(self.hash.to_owned(), &mut hash_bytes);
        match hex_res {
            Ok(_) => Some(hash_bytes),
            Err(_) => Option::None,
        }
    }

    fn to_metadata_url(&self) -> MetadataUrl {
        MetadataUrl {
            url: self.url.to_string(),
            hash: self.get_hash_as_bytes(),
        }
    }
}

#[derive(Serial, Deserial, SchemaType)]
struct TokenParams {
    amount: TokenAmountU64,
    max_supply: ContractTokenAmount,
}

#[derive(Serial, Deserial, SchemaType)]
struct MintParams {
    tokens: collections::BTreeSet<ContractTokenId>,
    signature: SignatureEd25519,
}

#[derive(Serial, DeserialWithState, Deletable, StateClone)]
#[concordium(state_parameter = "S")]
struct AddressState<S> {
    balances: StateSet<ContractTokenId, S>,
    operators: StateSet<Address, S>,
}

impl<S: HasStateApi> AddressState<S> {
    fn empty(state_builder: &mut StateBuilder<S>) -> Self {
        AddressState {
            balances: state_builder.new_set(),
            operators: state_builder.new_set(),
        }
    }
}

#[derive(Serial, Deserial, SchemaType)]
struct BurnParams {
    token_id: ContractTokenId
}

#[derive(Serial, DeserialWithState, StateClone)]
#[concordium(state_parameter = "S")]
struct State<S> {
    state: StateMap<Address, AddressState<S>, S>,
    tokens: StateMap<ContractTokenId, (TokenMetadata, ContractTokenAmount), S>,
    token_balance: StateMap<ContractTokenId, ContractTokenAmount, S>,
    implementors: StateMap<StandardIdentifierOwned, Vec<ContractAddress>, S>,
    verify_key: PublicKeyEd25519,
}

#[derive(Debug, Serialize, SchemaType)]
struct SetImplementorsParams {
    id: StandardIdentifierOwned,
    implementors: Vec<ContractAddress>,
}

#[derive(Serialize, Debug, PartialEq, Eq, Reject, SchemaType)]
enum CustomContractError {
    #[from(ParseError)]
    ParseParams,
    LogFull,
    LogMalformed,
    InvalidContractName,
    ContractOnly,
    InvokeContractError,
    TokenAlreadyCreated,
    TokenNotCreated,
    AuctionNotInitialized,
    MaxSupplyReached,
    NoBalanceToBurn,
}

type ContractError = Cis2Error<CustomContractError>;

type ContractResult<A> = Result<A, ContractError>;

impl From<LogError> for CustomContractError {
    fn from(le: LogError) -> Self {
        match le {
            LogError::Full => Self::LogFull,
            LogError::Malformed => Self::LogMalformed,
        }
    }
}

impl<T> From<CallContractError<T>> for CustomContractError {
    fn from(_cce: CallContractError<T>) -> Self {
        Self::InvokeContractError
    }
}

impl From<CustomContractError> for ContractError {
    fn from(c: CustomContractError) -> Self {
        Cis2Error::Custom(c)
    }
}

impl From<NewReceiveNameError> for CustomContractError {
    fn from(_: NewReceiveNameError) -> Self {
        Self::InvalidContractName
    }
}

impl From<NewContractNameError> for CustomContractError {
    fn from(_: NewContractNameError) -> Self {
        Self::InvalidContractName
    }
}

impl<S: HasStateApi> State<S> {
    fn empty(state_builder: &mut StateBuilder<S>, verify_key: PublicKeyEd25519) -> Self {
        State {
            state: state_builder.new_map(),
            tokens: state_builder.new_map(),
            token_balance: state_builder.new_map(),
            implementors: state_builder.new_map(),
            verify_key,
        }
    }

    fn mint(
        &mut self,
        token_id: &ContractTokenId,
        owner: &Address,
        state_builder: &mut StateBuilder<S>,
    ) {
        let mut owner_state = self
            .state
            .entry(*owner)
            .or_insert_with(|| AddressState::empty(state_builder));

        owner_state.balances.insert(*token_id);

        let mut circulating = self
            .token_balance
            .entry(*token_id)
            .or_insert_with(|| 0.into());
        *circulating += 1.into();
    }

    fn burn(&mut self, token_id: &ContractTokenId, owner: &Address) -> ContractResult<()> {
        let owner_state_option = self.state.get_mut(owner);

        ensure!(
            owner_state_option.is_some(),
            ContractError::Custom(CustomContractError::NoBalanceToBurn)
        );

        let mut address_state = owner_state_option.unwrap();

        let removed = address_state.balances.remove(&token_id);

        ensure!(removed, ContractError::Custom(CustomContractError::NoBalanceToBurn));

        let mut circulating = self
            .token_balance
            .entry(*token_id)
            .or_insert_with(|| 0.into());
        *circulating -= 1.into();

        Ok(())
    }

    #[inline(always)]
    fn contains_token(&self, token_id: &ContractTokenId) -> bool {
        self.tokens.get(&token_id).is_some()
    }

    fn balance(
        &self,
        token_id: &ContractTokenId,
        address: &Address,
    ) -> ContractResult<ContractTokenAmount> {
        ensure!(self.contains_token(token_id), ContractError::InvalidTokenId);
        let balance = self.state.get(address).map_or(0, |address_state| {
        let contains = address_state.balances.contains(&token_id);

        if contains {
            1
        } else {
            0
        }
        });
        Ok(balance.into())
    }

    fn is_operator(&self, address: &Address, owner: &Address) -> bool {
        self.state
            .get(owner)
            .map(|address_state| address_state.operators.contains(address))
            .unwrap_or(false)
    }

    fn add_operator(
        &mut self,
        owner: &Address,
        operator: &Address,
        state_builder: &mut StateBuilder<S>,
    ) {
        let mut owner_state = self
            .state
            .entry(*owner)
            .or_insert_with(|| AddressState::empty(state_builder));
        owner_state.operators.insert(*operator);
    }

    fn remove_operator(&mut self, owner: &Address, operator: &Address) {
        self.state.entry(*owner).and_modify(|address_state| {
            address_state.operators.remove(operator);
        });
    }

    fn have_implementors(&self, std_id: &StandardIdentifierOwned) -> SupportResult {
        if let Some(addresses) = self.implementors.get(std_id) {
            SupportResult::SupportBy(addresses.to_vec())
        } else {
            SupportResult::NoSupport
        }
    }

    fn set_implementors(
        &mut self,
        std_id: StandardIdentifierOwned,
        implementors: Vec<ContractAddress>,
    ) {
        self.implementors.insert(std_id, implementors);
    }

    fn get_token_supply(&self, token_id: &ContractTokenId) -> ContractResult<ContractTokenAmount> {
        ensure!(self.contains_token(token_id), ContractError::InvalidTokenId);
        let supply = self
            .tokens
            .get(token_id)
            .map(|info| info.1)
            .map_or(0.into(), |v| v);
        Ok(supply)
    }

    fn get_circulating_supply(
        &self,
        token_id: &ContractTokenId,
    ) -> ContractResult<ContractTokenAmount> {
        ensure!(self.contains_token(token_id), ContractError::InvalidTokenId);
        let circulating = self.token_balance.get(token_id).map_or(0.into(), |v| *v);
        Ok(circulating)
    }
}

#[derive(Serial, Deserial, SchemaType)]
struct InitParams {
    verify_key: PublicKeyEd25519,
}

#[init(
    contract = "dino_auction",
    parameter = "InitParams",
    event = "Cis2Event<ContractTokenId, ContractTokenAmount>"
)]
fn contract_init<S: HasStateApi>(
    ctx: &impl HasInitContext,
    state_builder: &mut StateBuilder<S>,
) -> InitResult<State<S>> {
    let params: InitParams = ctx.parameter_cursor().get()?;

    Ok(State::empty(state_builder, params.verify_key))
}

#[derive(Serialize, SchemaType)]
struct ViewAddressState {
    balances: Vec<ContractTokenId>,
    operators: Vec<Address>,
}

#[derive(Serialize, SchemaType)]
struct ViewState {
    state: Vec<(Address, ViewAddressState)>,
    tokens: Vec<ContractTokenId>,
}

#[receive(
    contract = "dino_auction",
    name = "view",
    return_value = "ViewState"
)]
fn contract_view<S: HasStateApi>(
    _ctx: &impl HasReceiveContext,
    host: &impl HasHost<State<S>, StateApiType = S>,
) -> ReceiveResult<ViewState> {
    let state = host.state();

    let mut inner_state = Vec::new();
    for (k, a_state) in state.state.iter() {
        let mut balances = Vec::new();
        let mut operators = Vec::new();
        for token_id in a_state.balances.iter() {
            balances.push(*token_id);
        }
        for o in a_state.operators.iter() {
            operators.push(*o);
        }

        inner_state.push((
            *k,
            ViewAddressState {
                balances,
                operators,
            },
        ));
    }
    let mut tokens = Vec::new();
    for v in state.tokens.iter() {
        tokens.push(*v.0);
    }

    Ok(ViewState {
        state: inner_state,
        tokens,
    })
}

#[receive(
    contract = "dino_auction",
    name = "mint",
    crypto_primitives,
    parameter = "MintParams",
    error = "ContractError",
    enable_logger,
    mutable
)]
fn contract_mint<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State<S>, StateApiType = S>,
    logger: &mut impl HasLogger,
    crypto_primitives: &impl HasCryptoPrimitives,
) -> ContractResult<()> {
    let sender = ctx.sender();

    let sender_account = match sender {
        Address::Account(a) => a,
        Address::Contract(_) => bail!(ContractError::Custom(CustomContractError::ContractOnly)),
    };

    let params: MintParams = ctx.parameter_cursor().get()?;

    let (state, builder) = host.state_and_builder();

    let verify = crypto_primitives.verify_ed25519_signature(
        state.verify_key,
        params.signature,
        &sender_account.0,
    );

    ensure!(verify, ContractError::Unauthorized);

    for token_id in params.tokens {
        let token = state.tokens.get(&token_id);

        ensure!(
            token.is_some(),
            ContractError::Custom(CustomContractError::AuctionNotInitialized)
        );

        let metadata_url = token.unwrap().0.to_metadata_url();

        let max_supply = state.get_token_supply(&token_id)?;
        let circulating_supply = state.get_circulating_supply(&token_id)?;

        ensure!(
            max_supply >= circulating_supply + 1.into(),
            ContractError::Custom(CustomContractError::MaxSupplyReached)
        );

        state.mint(&token_id, &sender, builder);

        logger.log(&Cis2Event::Mint(MintEvent {
            token_id,
            amount: TokenAmountU64::from(1),
            owner: sender,
        }))?;

        logger.log(&Cis2Event::TokenMetadata::<_, ContractTokenAmount>(
            TokenMetadataEvent {
                token_id,
                metadata_url,
            },
        ))?;
    }
    Ok(())
}

#[receive(
    contract = "dino_auction",
    name = "burn",
    parameter = "BurnParams",
    error = "ContractError",
    enable_logger,
    mutable
)]
fn contract_burn<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State<S>, StateApiType = S>,
    logger: &mut impl HasLogger,
) -> ContractResult<()> {
    let sender = ctx.sender();

    let params: BurnParams = ctx.parameter_cursor().get()?;
    let token_id = params.token_id;
    ensure!(
        host.state().contains_token(&token_id),
        ContractError::Custom(CustomContractError::AuctionNotInitialized));

    let state = host.state_mut();

    state.burn(&token_id, &sender)?;

    logger.log(&Cis2Event::Burn(BurnEvent {
        token_id,
        amount: TokenAmountU64::from(1),
        owner: sender,
    }))?;

    Ok(())
}

type TransferParameter = TransferParams<ContractTokenId, ContractTokenAmount>;

#[receive(
    contract = "dino_auction",
    name = "transfer",
    parameter = "TransferParameter",
    error = "ContractError",
    enable_logger,
    mutable
)]
fn contract_transfer<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    _host: &mut impl HasHost<State<S>, StateApiType = S>,
    _logger: &mut impl HasLogger,
) -> ContractResult<()> {
    let _: TransferParameter = ctx.parameter_cursor().get()?;
    bail!(ContractError::Unauthorized);
}

#[receive(
    contract = "dino_auction",
    name = "updateOperator",
    parameter = "UpdateOperatorParams",
    error = "ContractError",
    enable_logger,
    mutable
)]
fn contract_update_operator<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State<S>, StateApiType = S>,
    logger: &mut impl HasLogger,
) -> ContractResult<()> {
    let UpdateOperatorParams(params) = ctx.parameter_cursor().get()?;
    let sender = ctx.sender();
    let (state, builder) = host.state_and_builder();
    for param in params {
        match param.update {
            OperatorUpdate::Add => state.add_operator(&sender, &param.operator, builder),
            OperatorUpdate::Remove => state.remove_operator(&sender, &param.operator),
        }

        logger.log(
            &Cis2Event::<ContractTokenId, ContractTokenAmount>::UpdateOperator(
                UpdateOperatorEvent {
                    owner: sender,
                    operator: param.operator,
                    update: param.update,
                },
            ),
        )?;
    }

    Ok(())
}

#[receive(
    contract = "dino_auction",
    name = "operatorOf",
    parameter = "OperatorOfQueryParams",
    return_value = "OperatorOfQueryResponse",
    error = "ContractError"
)]
fn contract_operator_of<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &impl HasHost<State<S>, StateApiType = S>,
) -> ContractResult<OperatorOfQueryResponse> {
    let params: OperatorOfQueryParams = ctx.parameter_cursor().get()?;
    let mut response = Vec::with_capacity(params.queries.len());
    for query in params.queries {
        let is_operator = host.state().is_operator(&query.address, &query.owner);
        response.push(is_operator);
    }
    let result = OperatorOfQueryResponse::from(response);
    Ok(result)
}

type ContractBalanceOfQueryParams = BalanceOfQueryParams<ContractTokenId>;
type ContractBalanceOfQueryResponse = BalanceOfQueryResponse<ContractTokenAmount>;

#[receive(
    contract = "dino_auction",
    name = "balanceOf",
    parameter = "ContractBalanceOfQueryParams",
    return_value = "ContractBalanceOfQueryResponse",
    error = "ContractError"
)]
fn contract_balance_of<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &impl HasHost<State<S>, StateApiType = S>,
) -> ContractResult<ContractBalanceOfQueryResponse> {
    let params: ContractBalanceOfQueryParams = ctx.parameter_cursor().get()?;
    let mut response = Vec::with_capacity(params.queries.len());
    for query in params.queries {
        let amount = host.state().balance(&query.token_id, &query.address)?;
        response.push(amount);
    }
    let result = ContractBalanceOfQueryResponse::from(response);
    Ok(result)
}

type ContractTokenMetadataQueryParams = TokenMetadataQueryParams<ContractTokenId>;

#[receive(
    contract = "dino_auction",
    name = "tokenMetadata",
    parameter = "ContractTokenMetadataQueryParams",
    return_value = "TokenMetadataQueryResponse",
    error = "ContractError"
)]
fn contract_token_metadata<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &impl HasHost<State<S>, StateApiType = S>,
) -> ContractResult<TokenMetadataQueryResponse> {
    let params: ContractTokenMetadataQueryParams = ctx.parameter_cursor().get()?;
    let mut response = Vec::with_capacity(params.queries.len());
    for token_id in params.queries {
        ensure!(
            host.state().contains_token(&token_id),
            ContractError::InvalidTokenId
        );

        let meta = match host.state().tokens.get(&token_id) {
            Some(url) => url,
            None => return Err(ContractError::InvalidTokenId),
        };

        response.push(meta.0.to_metadata_url());
    }
    let result = TokenMetadataQueryResponse::from(response);
    Ok(result)
}

#[receive(
    contract = "dino_auction",
    name = "supports",
    parameter = "SupportsQueryParams",
    return_value = "SupportsQueryResponse",
    error = "ContractError"
)]
fn contract_supports<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &impl HasHost<State<S>, StateApiType = S>,
) -> ContractResult<SupportsQueryResponse> {
    let params: SupportsQueryParams = ctx.parameter_cursor().get()?;

    let mut response = Vec::with_capacity(params.queries.len());
    for std_id in params.queries {
        if SUPPORTS_STANDARDS.contains(&std_id.as_standard_identifier()) {
            response.push(SupportResult::Support);
        } else {
            response.push(host.state().have_implementors(&std_id));
        }
    }
    let result = SupportsQueryResponse::from(response);
    Ok(result)
}

#[receive(
    contract = "dino_auction",
    name = "setImplementors",
    parameter = "SetImplementorsParams",
    error = "ContractError",
    mutable
)]
fn contract_set_implementor<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State<S>, StateApiType = S>,
) -> ContractResult<()> {
    ensure!(
        ctx.sender().matches_account(&ctx.owner()),
        ContractError::Unauthorized
    );

    let params: SetImplementorsParams = ctx.parameter_cursor().get()?;
    host.state_mut()
        .set_implementors(params.id, params.implementors);
    Ok(())
}

#[derive(Serial, Deserial, SchemaType)]
struct AuctionInitParams {
    tokens: collections::BTreeMap<ContractTokenId, (TokenMetadata, ContractTokenAmount)>,
}

#[receive(
    contract = "dino_auction",
    name = "init_auction",
    parameter = "AuctionInitParams",
    error = "ContractError",
    mutable
)]
fn contract_init_auction<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State<S>, StateApiType = S>,
) -> ContractResult<()> {
    let owner = ctx.owner();
    let sender = ctx.sender();

    ensure!(sender.matches_account(&owner), ContractError::Unauthorized);

    let params: AuctionInitParams = ctx.parameter_cursor().get()?;

    let state = host.state_mut();

    for (token_id, token_info) in params.tokens {
        let contains = state.contains_token(&token_id);

        ensure!(
            !contains,
            ContractError::Custom(CustomContractError::TokenAlreadyCreated)
        );

        state.tokens.insert(token_id, token_info);
    }

    Ok(())
}

#[derive(Serial, Deserial, SchemaType)]
struct ActionBurnParams {
    tokens: collections::BTreeSet<ContractTokenId>,
}

#[receive(
    contract = "dino_auction",
    name = "burn_auction",
    parameter = "ActionBurnParams",
    error = "ContractError",
    mutable
)]
fn contract_burn_auction<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State<S>, StateApiType = S>,
) -> ContractResult<()> {
    let owner = ctx.owner();
    let sender = ctx.sender();

    ensure!(sender.matches_account(&owner), ContractError::Unauthorized);

    let params: ActionBurnParams = ctx.parameter_cursor().get()?;

    let state = host.state_mut();

    for token_id in params.tokens {
        ensure!(
            state.contains_token(&token_id),
            ContractError::Custom(CustomContractError::TokenAlreadyCreated)
        );

        state.tokens.remove(&token_id);
        state.token_balance.remove(&token_id);
        for (_, mut address_state) in state.state.iter_mut() {
            address_state.balances.remove(&token_id);
        }
    }

    Ok(())
}

#[receive(
    contract = "dino_auction",
    name = "get_owner",
    return_value = "AccountAddress"
)]
fn contract_get_owner<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    _host: &impl HasHost<State<S>, StateApiType = S>
) -> ContractResult<AccountAddress> {
    Ok(ctx.owner())
}

// Tests

#[concordium_cfg_test]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use test_infrastructure::*;
    #[cfg(feature = "crypto-primitives")]
    use ed25519_dalek::{ExpandedSecretKey, PublicKey, SecretKey};
    #[cfg(feature = "crypto-primitives")]
    use rand::rngs::OsRng;

    const ACCOUNT_0: AccountAddress = AccountAddress([0u8; 32]);
    const ADDRESS_0: Address = Address::Account(ACCOUNT_0);
    const ACCOUNT_1: AccountAddress = AccountAddress([1u8; 32]);
    const ADDRESS_1: Address = Address::Account(ACCOUNT_1);
    const TOKEN_0: ContractTokenId = TokenIdU32(0);
    const TOKEN_1: ContractTokenId = TokenIdU32(42);

    fn get_token_metadata() -> TokenMetadata {
        let mut hasher = Sha256::new();
        hasher.update(b"hello world");
        let hash = hasher.finalize();
        let hex_hash = hex::encode(hash);

        TokenMetadata {
            url: String::from("hello"),
            hash: hex_hash,
        }
    }

    fn initial_state<S: HasStateApi>(state_builder: &mut StateBuilder<S>) -> State<S> {
        let mut state = State::empty(state_builder, PublicKeyEd25519([0u8; 32]));

        let meta = get_token_metadata();

        state.tokens.insert(TOKEN_0, (meta.clone(), 400.into()));
        state.tokens.insert(TOKEN_1, (meta.clone(), 1.into()));

        state.mint(&TOKEN_0, &ADDRESS_0, state_builder);
        state.mint(&TOKEN_1, &ADDRESS_0, state_builder);
        state
    }

    #[cfg(feature = "crypto-primitives")]
    fn create_crypto_primitives() -> (SignatureEd25519, PublicKeyEd25519) {
        let mut csprng = OsRng {};
        let secret_key: SecretKey = SecretKey::generate(&mut csprng);
        let public_key: PublicKey = (&secret_key).into();
        let expanded: ExpandedSecretKey = ExpandedSecretKey::from(&secret_key);
        let signed = expanded.sign(&ACCOUNT_0.0, &public_key);
        return (
            SignatureEd25519(signed.to_bytes()),
            PublicKeyEd25519(public_key.to_bytes()),
        );
    }

    #[concordium_test]
    fn test_init() {
        // Arrange
        let mut ctx = TestInitContext::empty();
        let mut builder = TestStateBuilder::new();
        let parameter = InitParams {
            verify_key: PublicKeyEd25519([0u8; 32]),
        };
        let parameter_bytes = to_bytes(&parameter);
        ctx.set_parameter(&parameter_bytes);

        // Act
        let result = contract_init(&ctx, &mut builder);

        // Assert
        let state = result.expect_report("Contract initialization failed");
        claim_eq!(
            state.tokens.iter().count(),
            0,
            "Only one token is initialized"
        );
    }

    #[concordium_test]
    fn given_sender_is_owner_when_burn_auction_then_ok() {
        // Arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);
        ctx.set_owner(ACCOUNT_0);

        let mut state_builder = TestStateBuilder::new();
        let state = initial_state(&mut state_builder);
        let mut host = TestHost::new(state, state_builder);

        let mut tokens = collections::BTreeSet::new();
        tokens.insert(TOKEN_0);
        let params = ActionBurnParams { tokens };
        let parameter_bytes = to_bytes(&params);
        ctx.set_parameter(&parameter_bytes);

        // Act
        let result: ContractResult<()> = contract_burn_auction(&ctx, &mut host);

        // Assert
        claim!(result.is_ok());
        claim!(host.state().tokens.get(&TOKEN_0).is_none());

        let address_state = host.state().state.get(&ADDRESS_0).expect("Address missing");
        claim!(!address_state.balances.contains(&TOKEN_0));
        claim!(address_state.balances.contains(&TOKEN_1));

        claim!(host.state().token_balance.get(&TOKEN_0).is_none());
        claim!(host.state().token_balance.get(&TOKEN_1).is_some());
    }

    #[concordium_test]
    fn given_sender_is_not_owner_when_burn_auction_then_error() {
        // Arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);
        ctx.set_owner(ACCOUNT_1);

        let mut state_builder = TestStateBuilder::new();
        let state = initial_state(&mut state_builder);
        let mut host = TestHost::new(state, state_builder);

        let mut tokens = collections::BTreeSet::new();
        tokens.insert(TOKEN_0);
        let params = ActionBurnParams { tokens };
        let parameter_bytes = to_bytes(&params);
        ctx.set_parameter(&parameter_bytes);

        // Act
        let result: ContractResult<()> = contract_burn_auction(&ctx, &mut host);

        // Assert
        claim!(result.is_err());
        claim_eq!(result.unwrap_err(), ContractError::Unauthorized);
    }

    #[concordium_test]
    fn given_sender_is_owner_when_init_auction_then_ok() {
        // Arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);
        ctx.set_owner(ACCOUNT_0);

        let token_info = get_token_metadata();

        let mut tokens = collections::BTreeMap::new();
        tokens.insert(TOKEN_0, (token_info.clone(), 400.into()));
        tokens.insert(TOKEN_1, (token_info.clone(), 1.into()));
        let parameter = AuctionInitParams { tokens };
        let parameter_bytes = to_bytes(&parameter);
        ctx.set_parameter(&parameter_bytes);

        let mut state_builder = TestStateBuilder::new();
        let state = State::empty(&mut state_builder, PublicKeyEd25519([0u8; 32]));
        let mut host = TestHost::new(state, state_builder);

        // Act
        let result: ContractResult<()> = contract_init_auction(&ctx, &mut host);

        // Assert
        claim!(result.is_ok());

        let max_0 = host
            .state()
            .tokens
            .get(&TOKEN_0)
            .expect_report("Token not inserted");

        claim_eq!(max_0.1, 400.into());
    }

    #[concordium_test]
    fn given_sender_is_not_owner_when_init_auction_then_error() {
        // Arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);
        ctx.set_owner(ACCOUNT_1);

        let token_info = get_token_metadata();

        let mut tokens = collections::BTreeMap::new();
        tokens.insert(TOKEN_0, (token_info.clone(), 400.into()));
        let parameter = AuctionInitParams { tokens };
        let parameter_bytes = to_bytes(&parameter);
        ctx.set_parameter(&parameter_bytes);

        let mut state_builder = TestStateBuilder::new();
        let state = State::empty(&mut state_builder, PublicKeyEd25519([0u8; 32]));
        let mut host = TestHost::new(state, state_builder);

        // Act
        let result: ContractResult<()> = contract_init_auction(&ctx, &mut host);

        // Assert
        claim!(result.is_err());
    }

    #[concordium_test]
    #[cfg(not(feature = "crypto-primitives"))]
    fn given_token_not_existing_when_mint_then_return_error() {
        // Arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);

        let mut tokens = collections::BTreeSet::new();
        tokens.insert(TOKEN_0);
        let parameter = MintParams { 
            tokens,
            signature: SignatureEd25519([0u8; 64]),
        };
        let parameter_bytes = to_bytes(&parameter);
        ctx.set_parameter(&parameter_bytes);

        let mut logger = TestLogger::init();
        let mut state_builder = TestStateBuilder::new();
        let state = State::empty(&mut state_builder, PublicKeyEd25519([0u8; 32]));
        let mut host = TestHost::new(state, state_builder);
        let mut crypto = TestCryptoPrimitives::new();
        crypto.setup_verify_ed25519_signature_mock(|_, _, _| true);

        // Act
        let result: ContractResult<()> = contract_mint(&ctx, &mut host, &mut logger, &mut crypto);

        // Assert
        claim!(result.is_err());
        claim_eq!(
            result.unwrap_err(),
            ContractError::Custom(CustomContractError::AuctionNotInitialized)
        );
    }

    #[concordium_test]
    #[cfg(feature = "crypto-primitives")]
    fn given_crypto_primitives_when_signature_not_correct_then_return_error() {
        // Arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);

        let mut tokens = collections::BTreeSet::new();
        tokens.insert(TOKEN_0);

        let (signature, _) = create_crypto_primitives();

        let parameter = MintParams { 
            tokens,
            signature,
        };
        let parameter_bytes = to_bytes(&parameter);
        ctx.set_parameter(&parameter_bytes);

        let mut logger = TestLogger::init();
        let mut state_builder = TestStateBuilder::new();
        let state = State::empty(&mut state_builder, PublicKeyEd25519([0u8; 32]));
        let mut host = TestHost::new(state, state_builder);
        let mut crypto = TestCryptoPrimitives::new();

        // Act
        let result: ContractResult<()> = contract_mint(&ctx, &mut host, &mut logger, &mut crypto);

        // Assert
        claim!(result.is_err());
        claim_eq!(
            result.unwrap_err(),
            ContractError::Unauthorized
        );
    }


    #[concordium_test]
    #[cfg(feature = "crypto-primitives")]
    fn given_crypto_primitives_when_mint_then_add_token() {
        // Arrange
        let token_info = get_token_metadata();

        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);

        let mut tokens = collections::BTreeSet::new();
        tokens.insert(TOKEN_0);

        let (signature, verify_key) = create_crypto_primitives();

        let parameter = MintParams { 
            tokens,
            signature
        };
        let parameter_bytes = to_bytes(&parameter);
        ctx.set_parameter(&parameter_bytes);

        let mut logger = TestLogger::init();
        let mut state_builder = TestStateBuilder::new();
        
        let mut state = State::empty(&mut state_builder, verify_key);
        state.tokens.insert(TOKEN_0, (token_info.clone(), 1.into()));

        let mut host = TestHost::new(state, state_builder);
        let mut crypto = TestCryptoPrimitives::new();

        // Act
        let result: ContractResult<()> = contract_mint(&ctx, &mut host, &mut logger, &mut crypto);

        // Assert
        claim!(result.is_ok());
    }

    #[concordium_test]
    #[cfg(feature = "crypto-primitives")]
    fn given_crypto_primitives_when_mint_with_empty_metadata_then_return_without_error() {
        // Arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);

        let mut tokens = collections::BTreeSet::new();
        tokens.insert(TOKEN_0);

        let (signature, verify_key) = create_crypto_primitives();

        let parameter = MintParams { 
            tokens,
            signature
        };
        let parameter_bytes = to_bytes(&parameter);
        ctx.set_parameter(&parameter_bytes);

        let mut logger = TestLogger::init();
        let mut state_builder = TestStateBuilder::new();
        
        let mut state = State::empty(&mut state_builder, verify_key);
        state.tokens.insert(TOKEN_0, (TokenMetadata{hash:"".to_string(), url: "".to_string()}, 1.into()));

        let mut host = TestHost::new(state, state_builder);
        let mut crypto = TestCryptoPrimitives::new();

        // Act
        let result: ContractResult<()> = contract_mint(&ctx, &mut host, &mut logger, &mut crypto);

        // Assert
        claim!(result.is_ok());
    }

    #[concordium_test]
    fn when_burn_then_ok() {
        // Arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);

        let params = BurnParams{
            token_id: TOKEN_0
        };
        let parameter_bytes = to_bytes(&params);
        ctx.set_parameter(&parameter_bytes);

        let mut logger = TestLogger::init();
        let mut state_builder = TestStateBuilder::new();
        let state = initial_state(&mut state_builder);
        let mut host = TestHost::new(state, state_builder);

        // Act
        let result: ContractResult<()> = contract_burn(&ctx, &mut host, &mut logger);

        // Assert
        claim!(result.is_ok());
        
        claim_eq!(*host.state().token_balance.get(&TOKEN_0).expect_report("Token balance not present"), 0.into());
        claim_eq!(*host.state().token_balance.get(&TOKEN_1).expect_report("Token balance not present"), 1.into());

        let balance0 = host
            .state()
            .balance(&TOKEN_0, &ADDRESS_0)
            .expect_report("Token is expected to exist");
        claim_eq!(
            balance0,
            0.into(),
            "Initial tokens are owned by the contract instantiater"
        );

        let balance1 = host
            .state()
            .balance(&TOKEN_1, &ADDRESS_0)
            .expect_report("Token is expected to exist");
        claim_eq!(
            balance1,
            1.into(),
            "Initial tokens are owned by the contract instantiater"
        );
    }

    #[concordium_test]
    fn given_token_not_exist_when_burn_then_error() {
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);

        let params = BurnParams{
            token_id: TOKEN_0
        };

        let parameter_bytes = to_bytes(&params);
        ctx.set_parameter(&parameter_bytes);

        let mut logger = TestLogger::init();
        let mut state_builder = TestStateBuilder::new();
        let state = State::empty(&mut state_builder, PublicKeyEd25519([0u8; 32]));
        let mut host = TestHost::new(state, state_builder);

        // Act
        let result: ContractResult<()> = contract_burn(&ctx, &mut host, &mut logger);

        // Assert
        claim_eq!(result.expect_err("Should be error"), ContractError::Custom(CustomContractError::AuctionNotInitialized));
    }

    #[concordium_test]
    fn given_sender_not_have_token_when_burn_then_error() {
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);

        let params = BurnParams{
            token_id: TOKEN_0
        };

        let parameter_bytes = to_bytes(&params);
        ctx.set_parameter(&parameter_bytes);

        let mut logger = TestLogger::init();
        let mut state_builder = TestStateBuilder::new();
        let mut state = State::empty(&mut state_builder, PublicKeyEd25519([0u8; 32]));
        state.tokens.insert(TOKEN_0, (get_token_metadata(), 1.into()));
        let mut host = TestHost::new(state, state_builder);

        // Act
        let result: ContractResult<()> = contract_burn(&ctx, &mut host, &mut logger);

        // Assert
        claim_eq!(result.expect_err("Should be error"), ContractError::Custom(CustomContractError::NoBalanceToBurn));
    }

    #[concordium_test]
    #[cfg(not(feature = "crypto-primitives"))]
    fn when_mint_then_add_token() {
        // Arrange
        let token_info = get_token_metadata();

        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);

        let mut tokens = collections::BTreeSet::new();
        tokens.insert(TOKEN_0);
        tokens.insert(TOKEN_1);
        let parameter = MintParams { 
            tokens,
            signature: SignatureEd25519([0u8; 64]),
        };
        let parameter_bytes = to_bytes(&parameter);
        ctx.set_parameter(&parameter_bytes);

        let mut logger = TestLogger::init();
        let mut state_builder = TestStateBuilder::new();
        
        let mut state = State::empty(&mut state_builder, PublicKeyEd25519([0u8; 32]));
        state.tokens.insert(TOKEN_0, (token_info.clone(), 1.into()));
        state.tokens.insert(TOKEN_1, (token_info.clone(), 1.into()));

        let mut host = TestHost::new(state, state_builder);
        let mut crypto = TestCryptoPrimitives::new();
        crypto.setup_verify_ed25519_signature_mock(|_, _, _| true);

        // Act
        let result: ContractResult<()> = contract_mint(&ctx, &mut host, &mut logger, &mut crypto);

        // Assert
        claim!(result.is_ok());
        claim_eq!(*host.state().token_balance.get(&TOKEN_0).expect("token not present in balance"), 1.into());
        let balance0 = host
            .state()
            .balance(&TOKEN_0, &ADDRESS_0)
            .expect_report("Token is expected to exist");
        claim_eq!(
            balance0,
            1.into(),
            "Initial tokens are owned by the contract instantiater"
        );

        let balance1 = host
            .state()
            .balance(&TOKEN_1, &ADDRESS_0)
            .expect_report("Token is expected to exist");
        claim_eq!(
            balance1,
            1.into(),
            "Initial tokens are owned by the contract instantiater"
        );

        claim_eq!(logger.logs.len(), 4, "Exactly four events should be logged");
        claim!(
            logger.logs.contains(&to_bytes(&Cis2Event::Mint(MintEvent {
                owner: ADDRESS_0,
                token_id: TOKEN_0,
                amount: ContractTokenAmount::from(1),
            }))),
            "Expected an event for minting TOKEN_0"
        );
        claim!(
            logger.logs.contains(&to_bytes(&Cis2Event::Mint(MintEvent {
                owner: ADDRESS_0,
                token_id: TOKEN_1,
                amount: ContractTokenAmount::from(1),
            }))),
            "Expected an event for minting TOKEN_1"
        );
        claim!(
            logger.logs.contains(&to_bytes(
                &Cis2Event::TokenMetadata::<_, ContractTokenAmount>(TokenMetadataEvent {
                    token_id: TOKEN_0,
                    metadata_url: token_info.to_metadata_url(),
                })
            )),
            "Expected an event for token metadata for TOKEN_0"
        );
        claim!(
            logger.logs.contains(&to_bytes(
                &Cis2Event::TokenMetadata::<_, ContractTokenAmount>(TokenMetadataEvent {
                    token_id: TOKEN_1,
                    metadata_url: token_info.to_metadata_url(),
                })
            )),
            "Expected an event for token metadata for TOKEN_1"
        );
    }

    #[concordium_test]
    fn test_transfer_account() {
        // Arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);

        let transfer = Transfer {
            token_id: TOKEN_0,
            amount: ContractTokenAmount::from(100),
            from: ADDRESS_0,
            to: Receiver::from_account(ACCOUNT_1),
            data: AdditionalData::empty(),
        };
        let parameter = TransferParams::from(vec![transfer]);
        let parameter_bytes = to_bytes(&parameter);
        ctx.set_parameter(&parameter_bytes);

        let mut logger = TestLogger::init();
        let mut state_builder = TestStateBuilder::new();
        let state = initial_state(&mut state_builder);
        let mut host = TestHost::new(state, state_builder);

        // Act
        let result: ContractResult<()> = contract_transfer(&ctx, &mut host, &mut logger);

        // Assert
        claim!(result.is_err());
        claim_eq!(result.unwrap_err(), ContractError::Unauthorized);
    }

    #[concordium_test]
    fn test_operator_transfer() {
        // Arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_1);

        let transfer = Transfer {
            from: ADDRESS_0,
            to: Receiver::from_account(ACCOUNT_1),
            token_id: TOKEN_0,
            amount: ContractTokenAmount::from(100),
            data: AdditionalData::empty(),
        };
        let parameter = TransferParams::from(vec![transfer]);
        let parameter_bytes = to_bytes(&parameter);
        ctx.set_parameter(&parameter_bytes);

        let mut logger = TestLogger::init();
        let mut state_builder = TestStateBuilder::new();
        let mut state = initial_state(&mut state_builder);
        state.add_operator(&ADDRESS_0, &ADDRESS_1, &mut state_builder);
        let mut host = TestHost::new(state, state_builder);

        // Act
        let result: ContractResult<()> = contract_transfer(&ctx, &mut host, &mut logger);

        // Assert
        claim!(result.is_err());
        claim_eq!(result.unwrap_err(), ContractError::Unauthorized);
    }

    #[concordium_test]
    fn test_add_operator() {
        // Arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(ADDRESS_0);

        let update = UpdateOperator {
            operator: ADDRESS_1,
            update: OperatorUpdate::Add,
        };
        let parameter = UpdateOperatorParams(vec![update]);
        let parameter_bytes = to_bytes(&parameter);
        ctx.set_parameter(&parameter_bytes);

        let mut logger = TestLogger::init();
        let mut state_builder = TestStateBuilder::new();
        let state = initial_state(&mut state_builder);
        let mut host = TestHost::new(state, state_builder);

        // Act
        let result: ContractResult<()> = contract_update_operator(&ctx, &mut host, &mut logger);

        // Assert
        claim!(result.is_ok(), "Results in rejection");

        let is_operator = host.state().is_operator(&ADDRESS_1, &ADDRESS_0);
        claim!(is_operator, "Account should be an operator");

        let operator_of_query = OperatorOfQuery {
            address: ADDRESS_1,
            owner: ADDRESS_0,
        };

        let operator_of_query_vector = OperatorOfQueryParams {
            queries: vec![operator_of_query],
        };
        let parameter_bytes = to_bytes(&operator_of_query_vector);

        ctx.set_parameter(&parameter_bytes);

        let result: ContractResult<OperatorOfQueryResponse> = contract_operator_of(&ctx, &host);

        claim_eq!(
            result.expect_report("Failed getting result value").0,
            [true],
            "Account should be an operator in the query response"
        );

        claim_eq!(logger.logs.len(), 1, "One event should be logged");
        claim_eq!(
            logger.logs[0],
            to_bytes(
                &Cis2Event::<ContractTokenId, ContractTokenAmount>::UpdateOperator(
                    UpdateOperatorEvent {
                        owner: ADDRESS_0,
                        operator: ADDRESS_1,
                        update: OperatorUpdate::Add,
                    }
                )
            ),
            "Incorrect event emitted"
        )
    }
}

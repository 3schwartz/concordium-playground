use concordium_rust_sdk::{
    common::Versioned,
    endpoints::QueryError,
    id::{
        constants::{ArCurve, AttributeKind},
        id_proof_types::Proof,
        types::{AccountAddress, GlobalContext},
    }, types::CredentialRegistrationID,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::SystemTime,
};

#[derive(Debug, thiserror::Error)]
pub enum InjectStatementError {
    #[error("Error when acquiring internal lock")]
    LockingError,
    #[error("Unknown session")]
    UnknownSession,
    #[error("Issues with credentials")]
    Credential,
    #[error("Not allowed")]
    NotAllowed,
    #[error("Invalid proof")]
    InvalidProofs,
    #[error("Node access error: {0}")]
    NodeAccess(#[from] QueryError),
    #[error("Error parsing challenge")]
    ChallengeParse,
}

impl warp::reject::Reject for InjectStatementError {}

#[derive(Clone)]
pub struct Server {
    pub challenges: Arc<Mutex<HashMap<String, ChallengeStatus>>>,
    pub global_context: Arc<GlobalContext<ArCurve>>,
}

#[derive(Deserialize, Clone)]
pub struct WithAccountAddress {
    pub address: AccountAddress,
}

#[derive(Clone)]
pub struct ChallengeStatus {
    pub address: AccountAddress,
    pub created_at: SystemTime,
}

#[derive(Serialize)]
pub struct ChallengeResponse {
    pub challenge: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ChallengedProof {
    pub challenge: String,
    pub proof: ProofWithContext
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ProofWithContext {
    pub credential: CredentialRegistrationID,
    pub proof: Versioned<Proof<ArCurve, AttributeKind>>
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub code: u16,
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub struct Keys {
    pub sign_key: String,
    pub verify_key: String
}

impl Keys {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let keys: Keys = serde_json::from_reader(reader)?;
        Ok(keys)
    }
}
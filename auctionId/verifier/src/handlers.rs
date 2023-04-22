use crate::types::*;
use concordium_rust_sdk::{
    common::{base16_encode_string, types::KeyPair, base16_decode_string},
    id::{
        constants::{ArCurve, AttributeKind},
        id_proof_types::Statement,
        types::{AccountAddress, AccountCredentialWithoutProofs},
    },
    v2::BlockIdentifier,
};
use log::warn;
use rand::Rng;
use std::convert::Infallible;
use std::time::SystemTime;
use warp::{http::StatusCode, Rejection};

static CHALLENGE_EXPIRY_SECONDS: u64 = 600;
static CLEAN_INTERVAL_SECONDS: u64 = 600;

pub async fn handle_get_challenge(
    state: Server,
    address: AccountAddress,
) -> Result<impl warp::Reply, Rejection> {
    let state = state.clone();
    log::debug!("Parsed statement. Generating challenge");
    match get_challenge_worker(state, address).await {
        Ok(r) => Ok(warp::reply::json(&r)),
        Err(e) => {
            warn!("Request is invalid {:#?}.", e);
            Err(warp::reject::custom(e))
        }
    }
}

async fn get_challenge_worker(
    state: Server,
    address: AccountAddress,
) -> Result<ChallengeResponse, InjectStatementError> {
    let mut challenge_bytes = [0u8; 32];
    rand::thread_rng().fill(&mut challenge_bytes[..]);

    let mut sm = state
        .challenges
        .lock()
        .map_err(|_| InjectStatementError::LockingError)?;

    log::debug!("Generated challenge: {:?}", challenge_bytes);

    let challenge = base16_encode_string(&challenge_bytes);

    log::debug!("Challenge encoded: {:?}", challenge);

    sm.insert(
        challenge.clone(),
        ChallengeStatus {
            address,
            created_at: SystemTime::now(),
        },
    );

    Ok(ChallengeResponse { challenge })
}

pub async fn handle_provide_proof(
    client: concordium_rust_sdk::v2::Client,
    state: Server,
    statement: Statement<ArCurve, AttributeKind>,
    request: ChallengedProof,
    key_pair: KeyPair,
) -> Result<impl warp::Reply, Rejection> {
    match check_proof_worker(client, state, request, statement, key_pair).await {
        Ok(r) => Ok(warp::reply::json(&r)),
        Err(e) => {
            warn!("Request is invalid {:#?}.", e);
            Err(warp::reject::custom(e))
        }
    }
}

async fn check_proof_worker(
    mut client: concordium_rust_sdk::v2::Client,
    state: Server,
    request: ChallengedProof,
    statement: Statement<ArCurve, AttributeKind>,
    key_pair: KeyPair,
) -> Result<String, InjectStatementError> {
    let status = {
        let challenges = state
            .challenges
            .lock()
            .map_err(|_| InjectStatementError::LockingError)?;
        
        challenges
            .get(&request.challenge)
            .ok_or(InjectStatementError::UnknownSession)?
            .clone()
    };

    let cred_id = request.proof.credential;
    let acc_info = client
        .get_account_info(&status.address.into(), BlockIdentifier::LastFinal)
        .await?;

    let credentials = acc_info
        .response
        .account_credentials
        .get(&0.into())
        .ok_or(InjectStatementError::Credential)?;

    if concordium_rust_sdk::common::to_bytes(credentials.value.cred_id())
        != concordium_rust_sdk::common::to_bytes(&cred_id)
    {
        return Err(InjectStatementError::Credential);
    }

    let commitments = match &credentials.value {
        AccountCredentialWithoutProofs::Initial { icdv: _ } => {
            return Err(InjectStatementError::NotAllowed);
        }
        AccountCredentialWithoutProofs::Normal {
            cdv: _,
            commitments,
        } => commitments,
    };

    let challenge: [u8; 32] = base16_decode_string(&request.challenge)
        .map_err(|_| InjectStatementError::ChallengeParse)?;

    let valid = statement.verify(
        &challenge,
        &state.global_context,
        cred_id.as_ref(),
        commitments,
        &request.proof.proof.value,
    );

    if !valid {
        return Err(InjectStatementError::InvalidProofs);
    }

    let mut challenges = state
        .challenges
        .lock()
        .map_err(|_| InjectStatementError::LockingError)?;

    challenges.remove(&request.challenge);
    
    let sig = key_pair.sign(&acc_info.response.account_address.0);

    Ok(hex::encode_upper(sig.sig))
}

pub async fn handle_clean_state(state: Server) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(
        tokio::time::Duration::from_secs(CLEAN_INTERVAL_SECONDS)
    );

    loop {
        interval.tick().await;
        {
            let mut challenge = state.challenges.lock().unwrap();
            challenge.retain(|_, c| {
                c.created_at
                .elapsed()
                .map(|e| e.as_secs() < CHALLENGE_EXPIRY_SECONDS)
                .unwrap_or(false)
            });
        }
    }
}

pub async fn handle_rejection(err: Rejection) -> Result<impl warp::Reply, Infallible> {
    if err.is_not_found() {
        let code = StatusCode::NOT_FOUND;
        let message = "Not Found";
        Ok(make_reply(message.into(), code))
    } else if let Some(InjectStatementError::NotAllowed) = err.find() {
        let code = StatusCode::BAD_REQUEST;
        let message = "Needs proof.";
        Ok(make_reply(message.into(), code))
    } else if let Some(InjectStatementError::InvalidProofs) = err.find() {
        let code = StatusCode::BAD_REQUEST;
        let message = "Invalid proofs.";
        Ok(make_reply(message.into(), code))
    } else if let Some(InjectStatementError::NodeAccess(e)) = err.find() {
        let code = StatusCode::INTERNAL_SERVER_ERROR;
        let message = format!("Cannot access the node: {}", e);
        Ok(make_reply(message, code))
    } else if let Some(InjectStatementError::LockingError) = err.find() {
        let code = StatusCode::INTERNAL_SERVER_ERROR;
        let message = "Could not acquire lock.";
        Ok(make_reply(message.into(), code))
    } else if let Some(InjectStatementError::UnknownSession) = err.find() {
        let code = StatusCode::NOT_FOUND;
        let message = "Session not found.";
        Ok(make_reply(message.into(), code))
    } else if err
        .find::<warp::filters::body::BodyDeserializeError>()
        .is_some()
    {
        let code = StatusCode::BAD_REQUEST;
        let message = "Malformed body.";
        Ok(make_reply(message.into(), code))
    } else {
        let code = StatusCode::INTERNAL_SERVER_ERROR;
        let message = "Internal error.";
        Ok(make_reply(message.into(), code))
    }
}

fn make_reply(message: String, code: StatusCode) -> impl warp::Reply {
    let msg = ErrorResponse {
        message,
        code: code.as_u16()
    };
    warp::reply::with_status(warp::reply::json(&msg), code)
}
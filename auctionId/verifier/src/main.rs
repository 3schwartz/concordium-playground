mod handlers;
mod types;
use crate::handlers::*;
use crate::types::*;

use clap::Parser;
use concordium_rust_sdk::common::types::KeyPair;
use concordium_rust_sdk::{
    id::{
        constants::{ArCurve, AttributeKind},
        id_proof_types::Statement,
    },
    v2::BlockIdentifier,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use warp::Filter;

#[derive(clap::Parser, Debug)]
#[clap(version, author)]
struct IdVerifierConfig {
    #[clap(
        long,
        short,
        help = "GRPC V2 interface of the node",
        default_value = "https://node.testnet.concordium.com:20000"
    )]
    endpoint: concordium_rust_sdk::v2::Endpoint,

    #[structopt(
        long = "log-level",
        default_value = "debug",
        help = "Maximum log level"
    )]
    log_level: log::LevelFilter,

    #[clap(
        long = "statement",
        help = "The statement that the server accepts proofs for.",
        default_value = r#"[{"type":"AttributeInSet","attributeTag":"nationality","set":["AT","BE","BG","CY","CZ","DK","EE","FI","FR","DE","GR","HU","IE","IT","LV","LT","LU","MT","NL","PL","PT","RO","SK","SI","ES","SE","HR"]}]"#,
    )]
    statement: String,

    #[structopt(
        help = "path to keys",
        default_value = "./verifier/keys.json"
    )]
    keys_path: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Current dir: {:?}", std::env::current_dir()?);

    let app = IdVerifierConfig::parse();
    let mut log_builder = env_logger::Builder::new();
    
    log_builder.filter_level(app.log_level);
    log_builder.init();

    let keys = Keys::from_file(&app.keys_path)?;

    let mut client = concordium_rust_sdk::v2::Client::new(app.endpoint).await?;
    let global_context = client
        .get_cryptographic_parameters(BlockIdentifier::LastFinal)
        .await?
        .response;

    let state = Server {
        challenges: Arc::new(Mutex::new(HashMap::new())),
        global_context: Arc::new(global_context),
    };
    let prove_state = state.clone();
    let challenge_state = state.clone();

    let statement: Statement<ArCurve, AttributeKind> = serde_json::from_str(&app.statement)?;

    let cors = warp::cors()
        .allow_any_origin()
        .allow_header("Content-Type")
        .allow_method("POST");

    let get_challenge = warp::get()
        .and(warp::path!("api" / "challenge"))
        .and(warp::query::<WithAccountAddress>())
        .and_then(move |query: WithAccountAddress| {
            handle_get_challenge(challenge_state.clone(), query.address)
        });

    let get_statement = warp::get()
        .and(warp::path!("api" / "statement"))
        .map(move || warp::reply::json(&app.statement));        

    let provide_proof = warp::post()
        .and(warp::filters::body::content_length_limit(50 * 1024))
        .and(warp::path!("api" / "prove"))
        .and(warp::body::json::<ChallengedProof>())
        .and_then(move |request: ChallengedProof| {
            let kp = KeyPair::from(ed25519_dalek::Keypair {
                public: ed25519_dalek::PublicKey::from_bytes(
                    hex::decode(&keys.verify_key).unwrap().as_slice(),
                )
                .unwrap(),
                secret: ed25519_dalek::SecretKey::from_bytes(
                    hex::decode(&keys.sign_key).unwrap().as_slice(),
                )
                .unwrap(),
            });
            handle_provide_proof(
                client.clone(),
                prove_state.clone(),
                statement.clone(),
                request,
                kp,
            )
        });

    tokio::spawn(handle_clean_state(state.clone()));

    let server = get_challenge
        .or(get_statement)
        .or(provide_proof)
        .recover(handle_rejection)
        .with(cors)
        .with(warp::trace::request());

    warp::serve(server).run(([0, 0, 0, 0], 8020)).await;        

    Ok(())
}
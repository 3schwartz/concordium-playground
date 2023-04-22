use ed25519_dalek::{SecretKey, PublicKey};
use rand::rngs::OsRng;
use serde::Serialize;
use std::{env, fs::File, io::Write};

const DEFAULT_PATH_CONTRACT: &str = "./contract/input/init.json";
const DEFAULT_PATH_FRONTEND: &str = "./frontend/src/keys.json";
const DEFAULT_PATH_VERIFIER: &str = "./verifier/keys.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut csprng = OsRng {};
    let secret_key: SecretKey = SecretKey::generate(&mut csprng);
    let public_key: PublicKey = (&secret_key).into();

    println!("{secret_key:?}");
    println!("{public_key:?}");

    let hex_secret = hex::encode(secret_key.as_bytes());
    let hex_public = hex::encode(public_key.as_bytes());

    println!("Secret key: {hex_secret}");
    println!("Public key: {hex_public}");

    let (contract_path, frontend_path, verifier_path) = get_output_path();

    let contract_input = ContractInitInput::new(hex_public.clone());
    let frontend_input= Frontend::new(hex_public.clone());
    let verifier_input = Verifier::new(hex_secret, hex_public);

    store_to_file(contract_input, contract_path)?;
    store_to_file(frontend_input, frontend_path)?;
    store_to_file(verifier_input, verifier_path)?;

    println!("Successfully created files");

    Ok(())
}

fn store_to_file<T: Serialize>(object: T, path: String) -> Result<(), Box<dyn std::error::Error>> {
    let serialized_input = serde_json::to_string(&object)?;

    let mut file = File::create(path)?;
    file.write_all(serialized_input.as_bytes())?;

    Ok(())
}

fn get_output_path() -> (String, String, String) {
    let args: Vec<String> = env::args()
        .skip(1)
        .collect();

    match args.len() != 3 {
        true => {
            println!("Using default paths");
            (DEFAULT_PATH_CONTRACT.to_string(), 
            DEFAULT_PATH_FRONTEND.to_string(),
            DEFAULT_PATH_VERIFIER.to_string())
        },
        false => {
            (args[0].to_owned(), args[1].to_owned(), args[2].to_owned())
        },
    }

}

#[derive(Serialize)]
struct Frontend {
    verify_key: String,
}

impl Frontend {
    fn new(verify_key: String) -> Self {
        Self {verify_key}
    }
}

#[derive(Serialize)]
struct Verifier {
    sign_key: String,
    verify_key: String
}

impl Verifier {
    fn new(sign_key: String, verify_key: String) -> Self {
        Self {sign_key, verify_key}
    }
}

#[derive(Serialize)]
struct ContractInitInput {
    verify_key: String
}

impl ContractInitInput {
    fn new(verify_key: String) -> Self {
        Self {verify_key}
    }
}

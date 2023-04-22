use core::fmt;
use std::{env, fs::{self, File}, io::Write};
use serde::Serialize;

#[derive(Serialize)]
struct Schema {
    schema: String
}

impl Schema {
    fn new(schema: String) -> Self {
        Self { schema }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Current dir: {:?}", std::env::current_dir()?);
    
    let (input_path, output_path) = get_args()?;

    let file_contents = fs::read_to_string(input_path)?;

    let schema = Schema::new(file_contents);

    let serialized = serde_json::to_string(&schema)?;
    let mut file = File::create(output_path)?;
    file.write_all(serialized.as_bytes())?;

    println!("Successfully created file");

    Ok(())
}

fn get_args() -> Result<(String, String), CustomError> {
    let args: Vec<String> = env::args()
    .skip(1)
    .collect();

    if args.len() != 2 {
        println!("Input and output path should be given: {:?}", args);
        return Err(CustomError::MissingArgs);
    };

    return Ok((args[0].to_owned(), args[1].to_owned()));
}

#[derive(Debug)]
enum CustomError {
    MissingArgs
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CustomError::MissingArgs => write!(f, "MissingArgs"),
        }
    }
}

impl std::error::Error for CustomError {
}

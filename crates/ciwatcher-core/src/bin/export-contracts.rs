use ciwatcher_core::contracts::render_typescript_contract;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::PathBuf;

fn contract_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/src/generated/contracts.ts")
}

fn main() -> Result<(), Box<dyn Error>> {
    let mode = env::args().nth(1).unwrap_or_else(|| "generate".to_owned());
    let expected = render_typescript_contract();
    let path = contract_path();

    match mode.as_str() {
        "generate" => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, expected)?;
            println!("generated {}", path.display());
            Ok(())
        }
        "check" => {
            let actual = fs::read_to_string(&path).map_err(|error| {
                IoError::new(
                    error.kind(),
                    format!("cannot read generated contract {}: {error}", path.display()),
                )
            })?;
            if actual == expected {
                println!("generated contract is current");
                Ok(())
            } else {
                Err(IoError::new(
                    ErrorKind::InvalidData,
                    "generated TypeScript contract is stale; run `npm run contracts:generate`",
                )
                .into())
            }
        }
        _ => Err(IoError::new(
            ErrorKind::InvalidInput,
            "usage: export-contracts [generate|check]",
        )
        .into()),
    }
}

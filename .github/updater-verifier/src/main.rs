use minisign_verify::{PublicKey, Signature};
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

fn verify(
    public_key_path: &Path,
    artifact_path: &Path,
    signature_path: &Path,
) -> Result<(), String> {
    let public_key = PublicKey::from_file(public_key_path)
        .map_err(|error| format!("failed to load updater public key: {error}"))?;
    let signature = Signature::from_file(signature_path)
        .map_err(|error| format!("failed to load updater signature: {error}"))?;
    let mut verifier = public_key
        .verify_stream(&signature)
        .map_err(|error| format!("failed to initialize signature verification: {error}"))?;
    let mut artifact = File::open(artifact_path)
        .map_err(|error| format!("failed to open updater artifact: {error}"))?;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes_read = artifact
            .read(&mut buffer)
            .map_err(|error| format!("failed to read updater artifact: {error}"))?;
        if bytes_read == 0 {
            break;
        }
        verifier.update(&buffer[..bytes_read]);
    }

    verifier
        .finalize()
        .map_err(|error| format!("updater signature verification failed: {error}"))
}

fn run() -> Result<(), String> {
    let mut args = env::args_os();
    let program = args
        .next()
        .and_then(|value| value.into_string().ok())
        .unwrap_or_else(|| "sitecmd-updater-verifier".to_string());
    let public_key = args
        .next()
        .ok_or_else(|| format!("usage: {program} <public-key-file> <artifact> <signature>"))?;
    let artifact = args
        .next()
        .ok_or_else(|| format!("usage: {program} <public-key-file> <artifact> <signature>"))?;
    let signature = args
        .next()
        .ok_or_else(|| format!("usage: {program} <public-key-file> <artifact> <signature>"))?;
    if args.next().is_some() {
        return Err(format!(
            "usage: {program} <public-key-file> <artifact> <signature>"
        ));
    }

    verify(
        Path::new(&public_key),
        Path::new(&artifact),
        Path::new(&signature),
    )
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

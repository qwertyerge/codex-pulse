use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};
use serde_json::Value;
use std::{env, fs, path::Path, process::ExitCode};

#[derive(Debug, PartialEq, Eq)]
enum VerificationError {
    Input,
    Rejected,
}

#[cfg(test)]
const TEST_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXkgRTc2MjBGMTg0MkI0RTgxRgpSV1FmNkxSQ0dBOWk1M21sWWVjTzRJelQ1MVRHUHB2V3VjTlNDaDFDQk0wUVRhTG43M1k3R0ZPMwo=";
#[cfg(test)]
const TEST_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIG1pbmlzaWduIHNlY3JldCBrZXkKUldRZjZMUkNHQTlpNTlTTE9GeHo2Tnh2QVNYREplUnR1Wnlrd1FlcGJERUd0ODdpZzFCTnBXYVZXdU5ybTczWWlJaUpicTcxV2krZFA5ZUtMOE9DMzUxdndJYXNTU2JYeHdBPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNTU1Nzc5OTY2CWZpbGU6dGVzdApRdEtNWFd5WWN3ZHBaQWxQRjd0RTJFTkprUmQxdWp2S2psajFtOVJ0SFRCblpQYTVXS1U1dVdSczVHb1A1TS9WcUU4MVFGdU1LSTVrL1NmTlFVYU9BQT09Cg==";

fn decode_document(encoded: &str) -> Result<String, VerificationError> {
    let bytes = STANDARD
        .decode(encoded.trim())
        .map_err(|_| VerificationError::Input)?;
    String::from_utf8(bytes).map_err(|_| VerificationError::Input)
}

fn verify_encoded_signature(
    encoded_public_key: &str,
    encoded_signature: &str,
    fixture: &[u8],
) -> Result<(), VerificationError> {
    let public_key_document = decode_document(encoded_public_key)?;
    let signature_document = decode_document(encoded_signature)?;
    let public_key =
        PublicKey::decode(&public_key_document).map_err(|_| VerificationError::Input)?;
    let signature = Signature::decode(&signature_document).map_err(|_| VerificationError::Input)?;

    public_key
        .verify(fixture, &signature, true)
        .map_err(|_| VerificationError::Rejected)
}

fn verify_files(
    config_path: &Path,
    fixture_path: &Path,
    signature_path: &Path,
) -> Result<(), VerificationError> {
    let config_bytes = fs::read(config_path).map_err(|_| VerificationError::Input)?;
    let config: Value =
        serde_json::from_slice(&config_bytes).map_err(|_| VerificationError::Input)?;
    let encoded_public_key = config
        .pointer("/plugins/updater/pubkey")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(VerificationError::Input)?;
    let fixture = fs::read(fixture_path).map_err(|_| VerificationError::Input)?;
    let encoded_signature =
        fs::read_to_string(signature_path).map_err(|_| VerificationError::Input)?;

    verify_encoded_signature(encoded_public_key, &encoded_signature, &fixture)
}

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 3 {
        eprintln!("verification_status=usage_error");
        return ExitCode::from(2);
    }

    match verify_files(
        Path::new(&arguments[0]),
        Path::new(&arguments[1]),
        Path::new(&arguments[2]),
    ) {
        Ok(()) => {
            println!("signature_verified=true");
            ExitCode::SUCCESS
        }
        Err(VerificationError::Input) => {
            eprintln!("verification_status=input_error");
            ExitCode::from(3)
        }
        Err(VerificationError::Rejected) => {
            eprintln!("verification_status=rejected");
            ExitCode::from(4)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn outer_encode(document: &str) -> String {
        STANDARD.encode(document.as_bytes())
    }

    #[test]
    fn accepts_a_valid_tauri_encoded_signature() {
        assert_eq!(
            verify_encoded_signature(TEST_PUBLIC_KEY, TEST_SIGNATURE, b"test"),
            Ok(())
        );
    }

    #[test]
    fn rejects_a_modified_fixture() {
        assert_eq!(
            verify_encoded_signature(TEST_PUBLIC_KEY, TEST_SIGNATURE, b"Test"),
            Err(VerificationError::Rejected)
        );
    }

    #[test]
    fn rejects_a_modified_public_key() {
        let public_document = decode_document(TEST_PUBLIC_KEY)
            .expect("public test vector should decode")
            .replace("GFO3", "GFO2");

        assert_eq!(
            verify_encoded_signature(&outer_encode(&public_document), TEST_SIGNATURE, b"test"),
            Err(VerificationError::Rejected)
        );
    }

    #[test]
    fn rejects_a_modified_signature() {
        let signature_document = decode_document(TEST_SIGNATURE)
            .expect("signature test vector should decode")
            .replace("RWQf6LRCGA9i59S", "RWQf6LRCGA9i58S");

        assert_eq!(
            verify_encoded_signature(TEST_PUBLIC_KEY, &outer_encode(&signature_document), b"test"),
            Err(VerificationError::Rejected)
        );
    }

    #[test]
    fn rejects_malformed_outer_base64() {
        assert_eq!(
            verify_encoded_signature("not-base64", TEST_SIGNATURE, b"test"),
            Err(VerificationError::Input)
        );
        assert_eq!(
            verify_encoded_signature(TEST_PUBLIC_KEY, "not-base64", b"test"),
            Err(VerificationError::Input)
        );
    }

    #[test]
    fn rejects_a_config_without_an_updater_public_key() {
        let temporary = tempdir().expect("temporary directory should exist");
        let config = temporary.path().join("tauri.conf.json");
        let fixture = temporary.path().join("fixture.txt");
        let signature = temporary.path().join("fixture.txt.sig");

        fs::write(&config, r#"{"plugins":{"updater":{}}}"#).expect("config should be written");
        fs::write(&fixture, b"test").expect("fixture should be written");
        fs::write(&signature, TEST_SIGNATURE).expect("signature should be written");

        assert_eq!(
            verify_files(&config, &fixture, &signature),
            Err(VerificationError::Input)
        );
    }
}

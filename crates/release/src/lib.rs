//! What a release needs besides the installer itself: its signature and
//! `latest.json`, in the format Tauri's updater reads (so Tenths 0.4.x, which
//! is a Tauri app, can update straight into this one).
//!
//! * The signature is a minisign signature **file**, base64-encoded as a whole
//!   (that is what goes in `latest.json` and into `<installer>.sig`). Its
//!   trusted comment is tab-separated `key:value` pairs, among them
//!   `version:<x>`, which the updater checks against the announced version.
//! * The secret key is `TAURI_SIGNING_PRIVATE_KEY`: the base64 of a minisign
//!   *encrypted* secret key file, with its password in
//!   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
//!
//! The binary (`tenths-release`) is a thin command line over these functions.

use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Map, Value};

/// The platform keys Tauri's updater looks for on 64-bit Windows with an NSIS
/// installer (`windows-x86_64-nsis` first, then `windows-x86_64`).
pub const PLATFORMS: [&str; 2] = ["windows-x86_64", "windows-x86_64-nsis"];

/// `timestamp:<secs>\tfile:<name>\tversion:<x>`: what the signature vouches for besides the bytes.
pub fn trusted_comment(file_name: &str, version: &str, timestamp: u64) -> String {
    format!("timestamp:{timestamp}\tfile:{file_name}\tversion:{version}")
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The secret key as the CI secret holds it (base64 of the key file), or the key file's text itself.
fn key_text(secret: &str) -> Result<String, String> {
    let secret = secret.trim();
    if secret.starts_with("untrusted comment:") {
        return Ok(secret.to_string());
    }
    let bytes = B64.decode(secret.as_bytes()).map_err(|_| "The signing key isn't base64 (it should be the base64 of a minisign secret key file).".to_string())?;
    String::from_utf8(bytes).map_err(|_| "The signing key isn't a minisign key file.".to_string())
}

/// Signs `data` and returns the signature file's text (not yet base64).
pub fn sign_to_text(secret_key: &str, password: &str, data: &[u8], file_name: &str, version: &str, timestamp: u64) -> Result<String, String> {
    let text = key_text(secret_key)?;
    let boxed = minisign::SecretKeyBox::from_string(&text).map_err(|e| format!("The signing key can't be read: {e}"))?;
    let password = (!password.is_empty()).then(|| password.to_string());
    let key = boxed.into_secret_key(password).map_err(|e| format!("The signing key can't be unlocked (wrong password?): {e}"))?;
    let comment = trusted_comment(file_name, version, timestamp);
    let sig = minisign::sign(None, &key, Cursor::new(data), Some(&comment), Some("signature from tenths-release"))
        .map_err(|e| format!("Signing failed: {e}"))?;
    Ok(sig.into_string())
}

/// What `latest.json` and `<installer>.sig` carry: the base64 of the signature file.
pub fn sign(secret_key: &str, password: &str, data: &[u8], file_name: &str, version: &str, timestamp: u64) -> Result<String, String> {
    Ok(B64.encode(sign_to_text(secret_key, password, data, file_name, version, timestamp)?))
}

/// Checks a base64 signature against a base64 public key (both as the updater holds them), for the
/// bytes and the version. The release workflow runs this on what it just signed, so a wrong secret
/// fails the release rather than shipping updates nobody's app will accept.
pub fn verify(public_key_b64: &str, signature_b64: &str, data: &[u8], version: &str) -> Result<(), String> {
    let pk_text = String::from_utf8(B64.decode(public_key_b64.trim()).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let sig_text = String::from_utf8(B64.decode(signature_b64.trim()).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let pk = minisign::PublicKeyBox::from_string(&pk_text).and_then(|b| b.into_public_key()).map_err(|e| e.to_string())?;
    let sig = minisign::SignatureBox::from_string(&sig_text).map_err(|e| e.to_string())?;
    minisign::verify(&pk, &sig, Cursor::new(data), true, false, false).map_err(|e| format!("The signature doesn't match the key and file: {e}"))?;
    let comment = sig.trusted_comment().map_err(|e| e.to_string())?;
    match comment.split('\t').find_map(|p| p.strip_prefix("version:")) {
        Some(v) if v == version => Ok(()),
        Some(v) => Err(format!("The signature is for version {v}, not {version}.")),
        None => Err("The signature has no version in its trusted comment.".into()),
    }
}

/// `latest.json`: `platforms` is `(key, signature, url)`.
pub fn manifest(version: &str, notes: &str, pub_date: &str, platforms: &[(&str, &str, &str)]) -> String {
    let mut map = Map::new();
    for (key, signature, url) in platforms {
        map.insert((*key).to_string(), json!({ "signature": signature, "url": url }));
    }
    let doc = json!({ "version": version, "notes": notes, "pub_date": pub_date, "platforms": Value::Object(map) });
    serde_json::to_string_pretty(&doc).expect("a JSON value serialises") + "\n"
}

/// `2026-11-02T12:00:00Z` from seconds since 1970 (UTC).
pub fn rfc3339(secs: u64) -> String {
    let (days, rest) = ((secs / 86_400) as i64, secs % 86_400);
    // Days to a calendar date (Howard Hinnant's civil_from_days).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest / 60 % 60, rest % 60)
}

/// The body of the `## <version>` section of CHANGELOG.md (up to the next `## `), trimmed.
/// The heading may carry more after the version (`## 0.5.0 (in development)`).
pub fn changelog_section(changelog: &str, version: &str) -> Option<String> {
    let mut lines = changelog.lines();
    lines.find(|l| l.strip_prefix("## ").and_then(|h| h.split_whitespace().next()) == Some(version))?;
    let body: Vec<&str> = lines.take_while(|l| !l.starts_with("## ")).collect();
    let text = body.join("\n").trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh encrypted key pair, as `tauri signer generate` makes: `(secret as CI holds it, password, public key base64)`.
    pub(crate) fn key_pair(password: &str) -> (String, String) {
        let pair = minisign::KeyPair::generate_encrypted_keypair(Some(password.to_string())).unwrap();
        let sk = pair.sk.to_box(Some("test key")).unwrap().into_string();
        let pk = pair.pk.to_box().unwrap().into_string();
        (B64.encode(sk), B64.encode(pk))
    }

    #[test]
    fn signs_and_verifies_with_the_version_in_the_trusted_comment() {
        let (sk, pk) = key_pair("hunter2");
        let data = b"installer bytes";
        let sig = sign(&sk, "hunter2", data, "Tenths_0.5.0_x64-setup.exe", "0.5.0", 1_700_000_000).unwrap();
        // The signature is base64 of a signature file whose trusted comment names the version.
        let text = String::from_utf8(B64.decode(&sig).unwrap()).unwrap();
        assert!(text.contains("trusted comment: timestamp:1700000000\tfile:Tenths_0.5.0_x64-setup.exe\tversion:0.5.0"), "{text}");
        verify(&pk, &sig, data, "0.5.0").unwrap();
        // The independent verifier the app uses agrees.
        let key = minisign_verify::PublicKey::decode(&String::from_utf8(B64.decode(&pk).unwrap()).unwrap()).unwrap();
        let parsed = minisign_verify::Signature::decode(&text).unwrap();
        key.verify(data, &parsed, false).unwrap();
        assert!(parsed.trusted_comment().contains("version:0.5.0"));
    }

    #[test]
    fn tampering_a_wrong_key_and_a_wrong_version_fail() {
        let (sk, pk) = key_pair("pw");
        let (_, other_pk) = key_pair("pw");
        let sig = sign(&sk, "pw", b"bytes", "x.exe", "0.5.0", 1).unwrap();
        assert!(verify(&pk, &sig, b"bytes!", "0.5.0").is_err());
        assert!(verify(&other_pk, &sig, b"bytes", "0.5.0").is_err());
        assert!(verify(&pk, &sig, b"bytes", "0.5.1").unwrap_err().contains("0.5.0"));
        // A wrong password doesn't sign.
        assert!(sign(&sk, "nope", b"bytes", "x.exe", "0.5.0", 1).is_err());
        // Neither does something that isn't a key.
        assert!(sign("not a key", "pw", b"bytes", "x.exe", "0.5.0", 1).is_err());
    }

    #[test]
    fn the_manifest_has_tauris_shape() {
        let text = manifest("0.5.0", "- notes", "2026-11-02T12:00:00Z", &[("windows-x86_64", "SIG", "https://x/a.exe"), ("windows-x86_64-nsis", "SIG", "https://x/a.exe")]);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["version"], "0.5.0");
        assert_eq!(v["notes"], "- notes");
        assert_eq!(v["pub_date"], "2026-11-02T12:00:00Z");
        assert_eq!(v["platforms"]["windows-x86_64"]["signature"], "SIG");
        assert_eq!(v["platforms"]["windows-x86_64-nsis"]["url"], "https://x/a.exe");
    }

    #[test]
    fn dates_and_changelog_sections() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_709_251_199), "2024-02-29T23:59:59Z");
        let log = "# Changelog\n\nintro\n\n## 0.5.0 (in development)\n\n- one\n- two\n\n## 0.4.13\n\n- old\n";
        assert_eq!(changelog_section(log, "0.5.0").as_deref(), Some("- one\n- two"));
        assert_eq!(changelog_section(log, "0.4.13").as_deref(), Some("- old"));
        assert_eq!(changelog_section(log, "0.4.1"), None);
        assert_eq!(changelog_section(log, "0.5"), None);
    }
}

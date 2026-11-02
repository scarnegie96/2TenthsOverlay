//! The command line the release workflow runs, end to end: sign an installer,
//! check it against the public key, write latest.json, pull the notes out of the changelog.

use std::path::PathBuf;
use std::process::Command;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

fn tool() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tenths-release"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tenths-release-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn sign_verify_manifest_and_notes() {
    let dir = scratch("flow");
    let pair = minisign::KeyPair::generate_encrypted_keypair(Some("pw".into())).unwrap();
    let secret = B64.encode(pair.sk.to_box(Some("test")).unwrap().into_string());
    let public = B64.encode(pair.pk.to_box().unwrap().into_string());
    let other = minisign::KeyPair::generate_encrypted_keypair(Some("pw".into())).unwrap();
    let other_public = B64.encode(other.pk.to_box().unwrap().into_string());

    let installer = dir.join("Tenths_0.5.0_x64-setup.exe");
    std::fs::write(&installer, b"pretend installer").unwrap();
    let installer_s = installer.to_str().unwrap();

    // No secret: a plain failure, nothing written.
    let out = tool().args(["sign", "--file", installer_s, "--version", "0.5.0"]).env_remove("TAURI_SIGNING_PRIVATE_KEY").output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("TAURI_SIGNING_PRIVATE_KEY secret isn't set"));
    // Wrong password.
    let out = tool().args(["sign", "--file", installer_s, "--version", "0.5.0"]).env("TAURI_SIGNING_PRIVATE_KEY", &secret).env("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", "nope").output().unwrap();
    assert!(!out.status.success());
    assert!(!dir.join("Tenths_0.5.0_x64-setup.exe.sig").exists());

    // Signs, and the signature checks out for the right key and version only.
    let out = tool().args(["sign", "--file", installer_s, "--version", "0.5.0"]).env("TAURI_SIGNING_PRIVATE_KEY", &secret).env("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", "pw").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let verify = |key: &str, version: &str| tool().args(["verify", "--file", installer_s, "--pubkey", key, "--version", version]).output().unwrap().status.success();
    assert!(verify(&public, "0.5.0"));
    assert!(!verify(&other_public, "0.5.0"));
    assert!(!verify(&public, "0.5.1"));

    // latest.json from it, with the changelog's notes.
    let changelog = dir.join("CHANGELOG.md");
    std::fs::write(&changelog, "# Changelog\n\n## 0.5.0 (in development)\n\n- one\n- two\n\n## 0.4.13\n\n- old\n").unwrap();
    let out = tool().args(["notes", "--version", "0.5.0", "--changelog", changelog.to_str().unwrap()]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "- one\n- two");
    let notes = dir.join("notes.md");
    std::fs::write(&notes, &out.stdout).unwrap();
    let latest = dir.join("latest.json");
    let out = tool()
        .args(["manifest", "--version", "0.5.0", "--url", "https://example.com/Tenths_0.5.0_x64-setup.exe"])
        .args(["--sig-file", dir.join("Tenths_0.5.0_x64-setup.exe.sig").to_str().unwrap(), "--notes-file", notes.to_str().unwrap(), "--out", latest.to_str().unwrap(), "--pub-date", "2026-11-02T12:00:00Z"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&latest).unwrap()).unwrap();
    assert_eq!(json["version"], "0.5.0");
    assert_eq!(json["notes"], "- one\n- two");
    for key in ["windows-x86_64", "windows-x86_64-nsis"] {
        assert_eq!(json["platforms"][key]["url"], "https://example.com/Tenths_0.5.0_x64-setup.exe");
        // The signature in the manifest is the one the key vouches for.
        let sig = json["platforms"][key]["signature"].as_str().unwrap();
        tenths_release::verify(&public, sig, b"pretend installer", "0.5.0").unwrap();
    }
    std::fs::remove_dir_all(&dir).ok();
}

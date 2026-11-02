//! `tenths-release`: the small tool the release workflow runs.
//!
//!   tenths-release sign     --file <installer> --version <x.y.z>          writes <installer>.sig
//!   tenths-release verify   --file <installer> --version <x.y.z> --pubkey <base64>
//!   tenths-release manifest --version <x.y.z> --url <installer url> --sig-file <.sig> --notes-file <file> --out latest.json [--pub-date <rfc3339>]
//!   tenths-release notes    --version <x.y.z> [--changelog CHANGELOG.md]  prints that version's section
//!
//! `sign` reads the key from `TAURI_SIGNING_PRIVATE_KEY` and its password from
//! `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (names can be changed with `--key-env`
//! and `--password-env`), the same secrets the Tauri releases used.

use std::collections::HashMap;
use std::process::ExitCode;

use tenths_release as release;

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tenths-release: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `--name value` pairs after the command.
fn options(args: &[String]) -> Result<HashMap<&str, &str>, String> {
    let mut map = HashMap::new();
    let mut it = args.iter();
    while let Some(name) = it.next() {
        let name = name.strip_prefix("--").ok_or_else(|| format!("unexpected argument {name}"))?;
        let value = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
        map.insert(name, value.as_str());
    }
    Ok(map)
}

fn need<'a>(o: &HashMap<&str, &'a str>, name: &str) -> Result<&'a str, String> {
    o.get(name).copied().ok_or_else(|| format!("--{name} is required"))
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("can't read {path}: {e}"))
}

fn file_name(path: &str) -> &str {
    std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path)
}

fn run(args: Vec<String>) -> Result<(), String> {
    let (command, rest) = args.split_first().ok_or("usage: tenths-release sign|verify|manifest|notes ...")?;
    let o = options(rest)?;
    match command.as_str() {
        "sign" => {
            let file = need(&o, "file")?;
            let version = need(&o, "version")?;
            let key_env = o.get("key-env").copied().unwrap_or("TAURI_SIGNING_PRIVATE_KEY");
            let pass_env = o.get("password-env").copied().unwrap_or("TAURI_SIGNING_PRIVATE_KEY_PASSWORD");
            let key = std::env::var(key_env).ok().filter(|k| !k.trim().is_empty()).ok_or_else(|| format!("the {key_env} secret isn't set"))?;
            let password = std::env::var(pass_env).unwrap_or_default();
            let sig = release::sign(&key, &password, &read(file)?, file_name(file), version, release::now())?;
            let out = format!("{file}.sig");
            std::fs::write(&out, &sig).map_err(|e| format!("can't write {out}: {e}"))?;
            println!("signed {file} as version {version}: {out}");
            Ok(())
        }
        "verify" => {
            let file = need(&o, "file")?;
            let sig = std::fs::read_to_string(o.get("sig-file").map_or_else(|| format!("{file}.sig"), |s| (*s).to_string())).map_err(|e| e.to_string())?;
            release::verify(need(&o, "pubkey")?, &sig, &read(file)?, need(&o, "version")?)?;
            println!("{file}: signature is good");
            Ok(())
        }
        "manifest" => {
            let version = need(&o, "version")?;
            let url = need(&o, "url")?;
            let sig = std::fs::read_to_string(need(&o, "sig-file")?).map_err(|e| e.to_string())?;
            let notes = String::from_utf8_lossy(&read(need(&o, "notes-file")?)?).trim().to_string();
            let date = o.get("pub-date").map_or_else(|| release::rfc3339(release::now()), |d| (*d).to_string());
            let sig = sig.trim();
            let platforms: Vec<(&str, &str, &str)> = release::PLATFORMS.iter().map(|k| (*k, sig, url)).collect();
            let out = need(&o, "out")?;
            std::fs::write(out, release::manifest(version, &notes, &date, &platforms)).map_err(|e| format!("can't write {out}: {e}"))?;
            println!("wrote {out} for {version}");
            Ok(())
        }
        "notes" => {
            let version = need(&o, "version")?;
            let path = o.get("changelog").copied().unwrap_or("CHANGELOG.md");
            let text = String::from_utf8_lossy(&read(path)?).into_owned();
            // No section: a plain line, so the release still has notes.
            println!("{}", release::changelog_section(&text, version).unwrap_or_else(|| format!("Tenths {version}")));
            Ok(())
        }
        other => Err(format!("unknown command {other}")),
    }
}

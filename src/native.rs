//! Chrome native messaging transfers pairing settings through a private stdio pipe.
//! No HTTP endpoint, URL, clipboard, or log carries the secret.
use crate::{config, model::new_id};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const HOST: &str = "com.x_bot_follower_remover.pairing";

/// Chromium's GenerateIdForPath: first 128 bits of SHA-256, encoded as a..p.
/// The packaged extension has no manifest key; Chrome uses its absolute path.
pub fn extension_id(path: &Path) -> Result<String> {
    let absolute = path.canonicalize()?;
    path_id(&absolute)
}
fn path_id(path: &Path) -> Result<String> {
    let path = path.to_str().context("Extension path must be UTF-8")?;
    Ok(Sha256::digest(path.as_bytes())[..16]
        .iter()
        .flat_map(|b| [char::from(b'a' + (b >> 4)), char::from(b'a' + (b & 15))])
        .collect())
}
/// Repair only the known former bundled identity, not arbitrary manual pairings.
/// The caller must hold the data-directory lock before changing credentials.
pub fn migrate_legacy_pairing(
    data: &Path,
    settings: &mut config::Config,
    legacy: &Path,
    current: &Path,
) -> Result<bool> {
    if settings.extension_id.as_deref() != Some(path_id(legacy)?.as_str()) {
        return Ok(false);
    }
    let expected = extension_id(current)?;
    if settings.extension_id.as_deref() == Some(expected.as_str()) {
        return Ok(false);
    }
    settings.token = new_id();
    settings.extension_id = Some(expected);
    config::save(data, settings)?;
    Ok(true)
}

fn host_dir() -> Result<PathBuf> {
    ensure!(
        cfg!(target_os = "macos"),
        "Automatic pairing currently requires macOS"
    );
    Ok(dirs::home_dir()
        .context("Home directory unavailable")?
        .join("Library/Application Support/Google/Chrome/NativeMessagingHosts"))
}
fn quote(path: &Path) -> Result<String> {
    Ok(format!(
        "'{}'",
        path.to_str()
            .context("Path must be UTF-8")?
            .replace('\'', "'\\''")
    ))
}
fn write_private(path: &Path, bytes: &[u8], executable: bool) -> Result<()> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let tmp = path.with_extension(format!("{}.tmp", new_id()));
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    options.mode(if executable { 0o700 } else { 0o600 });
    #[cfg(not(unix))]
    let _ = executable;
    let mut file = options.open(&tmp)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result.map_err(Into::into)
}

pub fn register(data: &Path, extension: &Path) -> Result<()> {
    let executable = std::env::current_exe()?.canonicalize()?;
    register_at(&host_dir()?, &executable, data, extension)
}
pub fn register_at(
    host_dir: &Path,
    executable: &Path,
    data: &Path,
    extension: &Path,
) -> Result<()> {
    let id = extension_id(extension)?;
    let wrapper = executable
        .parent()
        .context("Executable folder unavailable")?
        .join("remover-native-host.sh");
    let script = format!(
        "#!/bin/sh\nexec {} --data-dir {} native-host \"$@\"\n",
        quote(executable)?,
        quote(&data.canonicalize()?)?
    );
    write_private(&wrapper, script.as_bytes(), true)?;
    fs::create_dir_all(host_dir)?;
    write_private(
        &host_dir.join(format!("{HOST}.json")),
        &serde_json::to_vec_pretty(&json!({
            "name": HOST, "description": "Connect remover to its Chrome extension",
            "path": wrapper, "type": "stdio", "allowed_origins": [format!("chrome-extension://{id}/")],
        }))?,
        false,
    )
}

pub fn unregister() -> Result<()> {
    let executable = std::env::current_exe()?.canonicalize()?;
    let wrapper = executable
        .parent()
        .context("Executable folder unavailable")?
        .join("remover-native-host.sh");
    let path = host_dir()?.join(format!("{HOST}.json"));
    if let Ok(bytes) = fs::read(&path) {
        let manifest: Value = serde_json::from_slice(&bytes)?;
        // An old or custom installation must not remove another installation's host.
        if manifest["path"].as_str() == wrapper.to_str() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

pub fn reply(data: &Path, extension: &Path, origin: &str, request: Value) -> Result<Value> {
    ensure!(
        request == json!({"type":"pair", "v":1}),
        "Unsupported pairing request"
    );
    let id = extension_id(extension)?;
    ensure!(
        origin == format!("chrome-extension://{id}/"),
        "Extension identity rejected"
    );
    // Read settings only while the corresponding TUI holds its process lock.
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(data.join("app.lock"))
        .context("Start remover in your terminal first")?;
    match lock.try_lock_exclusive() {
        Ok(()) => anyhow::bail!("Start remover in your terminal first"),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
        Err(e) => return Err(e.into()),
    }
    let settings: config::Config = serde_json::from_slice(&fs::read(data.join("config.json"))?)?;
    ensure!(
        settings.token.len() == 64
            && settings.token.bytes().all(|b| b.is_ascii_hexdigit())
            && settings.port >= 1024,
        "Invalid local pairing settings"
    );
    ensure!(
        settings
            .extension_id
            .as_ref()
            .is_none_or(|paired| paired == &id),
        "A different extension is paired. Run remover pair --reset with the TUI closed"
    );
    Ok(json!({"ok":true, "v":1, "port":settings.port, "token":settings.token, "extension_id":id}))
}
pub fn serve(
    data: &Path,
    extension: &Path,
    origin: &str,
    mut input: impl Read,
    mut output: impl Write,
) -> Result<()> {
    let result = (|| -> Result<Value> {
        let mut length = [0; 4];
        input.read_exact(&mut length)?;
        let length = u32::from_ne_bytes(length) as usize;
        ensure!(
            (1..=4096).contains(&length),
            "Invalid native message length"
        );
        let mut bytes = vec![0; length];
        input.read_exact(&mut bytes)?;
        reply(data, extension, origin, serde_json::from_slice(&bytes)?)
    })();
    let response = result.unwrap_or_else(|e| json!({"ok":false, "error":format!("{e:#}")}));
    let bytes = serde_json::to_vec(&response)?;
    output.write_all(&(bytes.len() as u32).to_ne_bytes())?;
    output.write_all(&bytes)?;
    output.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_pipe_requires_exact_origin_and_a_running_tui() {
        let temp = tempfile::tempdir().unwrap();
        let extension = temp.path().join("extension");
        fs::create_dir(&extension).unwrap();
        let data = temp.path().join("data");
        fs::create_dir(&data).unwrap();
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(data.join("app.lock"))
            .unwrap();
        let settings = config::Config::default();
        config::save(&data, &settings).unwrap();
        let origin = format!("chrome-extension://{}/", extension_id(&extension).unwrap());
        assert!(reply(&data, &extension, &origin, json!({"type":"pair","v":1})).is_err());
        lock.lock_exclusive().unwrap();
        let expected = reply(&data, &extension, &origin, json!({"type":"pair","v":1})).unwrap();
        assert_eq!(expected["token"], settings.token);
        for caller in [
            "https://x.com",
            "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/",
            &format!("{origin}extra"),
        ] {
            assert!(reply(&data, &extension, caller, json!({"type":"pair","v":1})).is_err());
        }
        assert!(reply(&data, &extension, &origin, json!({"type":"remove","v":1})).is_err());
        let mut settings = settings;
        settings.extension_id = Some("other".into());
        config::save(&data, &settings).unwrap();
        assert!(reply(&data, &extension, &origin, json!({"type":"pair","v":1})).is_err());
    }
    #[test]
    fn framing_rejects_oversize_input_without_reading_body() {
        let mut output = Vec::new();
        serve(
            Path::new("/unused"),
            Path::new("/unused"),
            "",
            &u32::MAX.to_ne_bytes()[..],
            &mut output,
        )
        .unwrap();
        let length = u32::from_ne_bytes(output[..4].try_into().unwrap()) as usize;
        assert_eq!(length, output.len() - 4);
        let value: Value = serde_json::from_slice(&output[4..]).unwrap();
        assert_eq!(value["ok"], false);
        assert!(value.get("token").is_none());
    }
    #[test]
    fn registration_quotes_paths_and_limits_the_chrome_origin() {
        let temp = tempfile::tempdir().unwrap();
        let extension = temp.path().join("extension");
        fs::create_dir(&extension).unwrap();
        let binary = temp.path().join("bin's app");
        let data = temp.path().join("data $(literal)");
        fs::create_dir(&data).unwrap();
        let registry = temp.path().join("hosts");
        register_at(&registry, &binary, &data, &extension).unwrap();
        let manifest: Value =
            serde_json::from_slice(&fs::read(registry.join(format!("{HOST}.json"))).unwrap())
                .unwrap();
        assert_eq!(
            manifest["allowed_origins"],
            json!([format!(
                "chrome-extension://{}/",
                extension_id(&extension).unwrap()
            )])
        );
        let script = fs::read_to_string(temp.path().join("remover-native-host.sh")).unwrap();
        assert!(script.contains("bin'\\''s app'"));
        assert!(script.contains("data $(literal)'"));
        assert!(!script.contains("token"));
        // Chrome starts the helper from the executable folder, not the TUI's cwd.
        register_at(&registry, &binary, Path::new("."), &extension).unwrap();
        let script = fs::read_to_string(temp.path().join("remover-native-host.sh")).unwrap();
        assert!(script.contains(&format!(
            "--data-dir {} native-host",
            quote(&Path::new(".").canonicalize().unwrap()).unwrap()
        )));
    }
}

use crate::model::new_id;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub token: String,
    pub port: u16,
    pub extension_id: Option<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            token: new_id(),
            port: 47831,
            extension_id: None,
        }
    }
}
pub fn data_dir(custom: Option<PathBuf>) -> Result<PathBuf> {
    let p = custom.unwrap_or(
        dirs::data_local_dir()
            .context("No local data directory")?
            .join("forgive-me"),
    );
    fs::create_dir_all(&p)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700))?;
    }
    Ok(p)
}
pub fn load(dir: &Path) -> Result<Config> {
    let p = dir.join("config.json");
    if p.exists() {
        let c: Config = serde_json::from_slice(&fs::read(p)?)?;
        if c.token.len() != 64 {
            bail!("Invalid pairing configuration");
        }
        Ok(c)
    } else {
        let c = Config::default();
        save(dir, &c)?;
        Ok(c)
    }
}
pub fn save(dir: &Path, c: &Config) -> Result<()> {
    let path = dir.join("config.tmp");
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    use std::io::Write;
    let mut f = opts.open(&path)?;
    f.write_all(&serde_json::to_vec_pretty(c)?)?;
    f.sync_all()?;
    fs::rename(path, dir.join("config.json"))?;
    Ok(())
}

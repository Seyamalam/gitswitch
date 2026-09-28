use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub alias: String,
    pub username: String,
    pub name: String,
    pub email: String,
    /// "gh" (main, HTTPS) or "ssh"
    pub auth: String,
    pub ssh_key: Option<String>,
    pub ssh_alias: Option<String>,
    /// Optional per-account token (classic PAT with repo+workflow) so
    /// `gitswitch gh <alias> -- …` can act as this account (PRs, issues…).
    /// Never printed by list/status. File is stored with 0600 on unix.
    #[serde(default)]
    pub token: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub accounts: Vec<Account>,
}

pub fn config_dir() -> Result<PathBuf> {
    let d = dirs::config_dir().context("no config dir")?.join("gitswitch");
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

pub fn store_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("accounts.toml"))
}

pub fn load() -> Result<Store> {
    let p = store_path()?;
    if !p.exists() {
        return Ok(Store::default());
    }
    let s = std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
    if s.trim().is_empty() {
        return Ok(Store::default());
    }
    // Support both `{ accounts = [...] }` (our format) and bare `[[accounts]]` files.
    if let Ok(st) = toml::from_str::<Store>(&s) {
        return Ok(st);
    }
    #[derive(Deserialize)]
    struct Bare {
        #[serde(default)]
        accounts: Vec<Account>,
    }
    let b: Bare = toml::from_str(&s)?;
    Ok(Store { accounts: b.accounts })
}

impl Store {
    pub fn save(&self) -> Result<()> {
        let p = store_path()?;
        let s = toml::to_string_pretty(self)?;
        std::fs::write(&p, s)?;
        // The store may hold per-account tokens — keep it private.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&p)?.permissions();
            perm.set_mode(0o600);
            std::fs::set_permissions(&p, perm)?;
        }
        Ok(())
    }
}

pub fn norm_alias(a: &str) -> Result<String> {
    let a = a.trim().to_lowercase();
    if a.is_empty() {
        bail!("alias must not be empty");
    }
    if !a.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("alias '{a}' must be [a-z0-9-_]");
    }
    Ok(a)
}

pub fn expand_tilde(p: &std::path::Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(h) = dirs::home_dir() {
            return h.join(rest);
        }
    }
    p.to_path_buf()
}

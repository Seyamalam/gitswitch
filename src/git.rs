use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

fn git(args: &[&str]) -> Result<String> {
    let out = Command::new("git").args(args).output().context("run git")?;
    if !out.status.success() {
        anyhow::bail!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn assert_in_repo() -> Result<()> {
    git(&["rev-parse", "--git-dir"])?;
    Ok(())
}

pub fn repo_root() -> Result<PathBuf> {
    let s = git(&["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(s))
}

pub fn local_config(key: &str) -> Result<String> {
    git(&["config", "--local", key])
}

pub fn global_config(key: &str) -> Result<String> {
    git(&["config", "--global", key])
}

pub fn set_local_config(key: &str, val: &str) -> Result<()> {
    git(&["config", "--local", key, val])?;
    Ok(())
}

pub fn unset_local_config(key: &str) -> Result<()> {
    let out = Command::new("git").args(["config", "--local", "--unset", key]).output()?;
    // ignore failure (key may not exist)
    let _ = out;
    Ok(())
}

pub fn set_global_config(key: &str, val: &str) -> Result<()> {
    git(&["config", "--global", key, val])?;
    Ok(())
}

pub fn remote_url(remote: &str) -> Result<String> {
    git(&["remote", "get-url", remote])
}

pub fn set_remote_url(remote: &str, url: &str) -> Result<()> {
    git(&["remote", "set-url", remote, url])?;
    Ok(())
}

/// Parse owner/repo out of HTTPS, SSH, or bare owner/repo strings.
pub fn parse_github_repo(s: &str) -> Option<(String, String)> {
    let s = s.trim();
    // bare owner/repo
    if !s.contains("://") && !s.contains('@') && s.matches('/').count() == 1 && !s.contains(' ') {
        let mut it = s.split('/');
        return Some((it.next()?.into(), it.next()?.trim_end_matches(".git").into()));
    }
    // https://github.com/owner/repo(.git)
    if let Some(rest) = s.strip_prefix("https://github.com/").or_else(|| s.strip_prefix("http://github.com/")) {
        let rest = rest.trim_end_matches(".git").trim_matches('/');
        let mut it = rest.split('/');
        let o = it.next()?.to_string();
        let r = it.next()?.to_string();
        if !o.is_empty() && !r.is_empty() {
            return Some((o, r));
        }
        return None;
    }
    // git@github.com:owner/repo(.git)  or  git@<alias>:owner/repo(.git)  or ssh://git@host/owner/repo
    if let Some(colon) = s.find(':') {
        let after = &s[colon + 1..];
        if after.contains('/') && (s.starts_with("git@") || s.starts_with("ssh://")) {
            let repo = after.trim_start_matches('/').trim_end_matches(".git");
            let mut it = repo.split('/');
            let o = it.next()?.to_string();
            let r = it.next()?.to_string();
            if !o.is_empty() && !r.is_empty() {
                return Some((o, r));
            }
        }
    }
    None
}

pub fn https_url(owner: &str, repo: &str) -> String {
    format!("https://github.com/{}/{}.git", owner, repo.trim_end_matches(".git"))
}

pub fn ssh_url(host: &str, owner: &str, repo: &str) -> String {
    format!("git@{host}:{}/{}.git", owner, repo.trim_end_matches(".git"))
}

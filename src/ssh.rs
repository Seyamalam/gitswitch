use anyhow::Result;

fn ssh_config_path() -> Result<std::path::PathBuf> {
    let h = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home dir"))?;
    Ok(h.join(".ssh").join("config"))
}

fn block_for(host: &str, key: &str) -> String {
    // Quote the key path: Windows profiles often live under `C:\Users\First Last\...`
    // and OpenSSH accepts quoted IdentityFile values on all platforms.
    format!(
        "# === gitswitch {host} START ===\nHost {host}\n    HostName github.com\n    User git\n    IdentityFile \"{key}\"\n    IdentitiesOnly yes\n# === gitswitch {host} END ===\n"
    )
}

/// Ensure (create or update) the managed Host block for `host`.
pub fn ensure_host_entry(host: &str, key: &str) -> Result<()> {
    let path = ssh_config_path()?;
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let block = block_for(host, key);
    let cur = std::fs::read_to_string(&path).unwrap_or_default();
    let start = format!("# === gitswitch {host} START ===");
    let end = format!("# === gitswitch {host} END ===");
    let new_content = if cur.contains(&start) && cur.contains(&end) {
        // replace existing block
        let s = cur.find(&start).unwrap();
        let e = cur.find(&end).unwrap() + end.len();
        let mut out = cur[..s].to_string();
        out.push_str(&block);
        out.push_str(cur[e..].trim_start_matches('\n'));
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    } else {
        let mut out = cur;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&block);
        out
    };
    std::fs::write(&path, new_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = std::fs::metadata(&path)?.permissions();
        perm.set_mode(0o600);
        std::fs::set_permissions(&path, perm)?;
    }
    Ok(())
}

pub fn remove_host_entry(host: &str) -> Result<()> {
    let path = ssh_config_path()?;
    let cur = std::fs::read_to_string(&path).unwrap_or_default();
    let start = format!("# === gitswitch {host} START ===");
    let end = format!("# === gitswitch {host} END ===");
    if !cur.contains(&start) {
        return Ok(());
    }
    let s = cur.find(&start).unwrap();
    let e = cur.find(&end).map(|i| i + end.len()).unwrap_or(cur.len());
    let mut out = cur[..s].to_string();
    out.push_str(cur[e..].trim_start_matches('\n'));
    std::fs::write(&path, out)?;
    Ok(())
}

pub fn host_exists(host: &str) -> Result<bool> {
    let path = ssh_config_path()?;
    let cur = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(cur.contains(&format!("Host {host}")))
}

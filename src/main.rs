pub mod git;
pub mod ssh;
pub mod store;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "gitswitch", version, about = "Use multiple GitHub accounts at once: main via gh/HTTPS, extras via SSH")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List all configured accounts (marks the one used by the current repo)
    List {
        /// Machine-readable output (for scripts and AI agents)
        #[arg(long)]
        json: bool,
    },
    /// Show which account the current repo is using
    Status {
        /// Remote to inspect (default: origin)
        #[arg(long, default_value = "origin")]
        remote: String,
        /// Machine-readable output (for scripts and AI agents)
        #[arg(long)]
        json: bool,
    },
    /// Add an account. Main account uses gh/HTTPS, extras use SSH.
    Add {
        /// Short alias, e.g. main, work, personal
        #[arg(long)]
        alias: String,
        /// GitHub username
        #[arg(long)]
        username: Option<String>,
        /// Commit email for this account
        #[arg(long)]
        email: Option<String>,
        /// Display name for commits (defaults to username or alias)
        #[arg(long)]
        name: Option<String>,
        /// Mark this as the main account (uses `gh` + HTTPS, no SSH key)
        #[arg(long)]
        main: bool,
        /// Path to SSH private key (defaults to ~/.ssh/id_<alias>). Implies SSH account.
        #[arg(long)]
        ssh_key: Option<PathBuf>,
        /// SSH Host alias written to ~/.ssh/config (defaults to github-<alias>)
        #[arg(long)]
        ssh_alias: Option<String>,
        /// Don't touch ~/.ssh/config yet
        #[arg(long)]
        no_ssh_setup: bool,
    },
    /// Remove an account from gitswitch (optionally removes its SSH block)
    Remove {
        alias: String,
        #[arg(long)]
        yes: bool,
        /// Also remove the SSH Host block from ~/.ssh/config
        #[arg(long)]
        drop_ssh: bool,
    },
    /// Switch the current repo to an account (sets local user.name/email + remote URL)
    Use {
        alias: String,
        /// Remote to rewrite (default: origin)
        #[arg(long, default_value = "origin")]
        remote: String,
        /// Apply to --global gitconfig instead of the current repo
        #[arg(long)]
        global: bool,
    },
    /// Run a git command as an account without switching repo config.
    /// Example: gitswitch exec work -- commit -m "fix"   or   gitswitch exec work -- push
    Exec {
        alias: String,
        /// Args passed to git (everything after --)
        #[arg(last = true, required = true)]
        git_args: Vec<String>,
    },
    /// Clone a repo as an account (picks HTTPS vs SSH URL automatically)
    Clone {
        alias: String,
        /// owner/repo, HTTPS URL, or SSH URL
        repo: String,
        /// Destination directory (optional)
        dest: Option<PathBuf>,
    },
    /// Generate an ed25519 SSH key for an SSH account (or a bare alias)
    GenKey {
        alias: String,
        /// Overwrite if key already exists
        #[arg(long)]
        force: bool,
    },
    /// Check gh auth, keys, ssh config and current repo
    Doctor,
    /// Print shell completions (bash, zsh, fish, powershell, elvish).
    /// Example: gitswitch completions bash >> ~/.bash_completion
    Completions {
        shell: Shell,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::List { json } => cmd_list(json),
        Cmd::Status { remote, json } => cmd_status(&remote, json),
        Cmd::Add { alias, username, email, name, main, ssh_key, ssh_alias, no_ssh_setup } =>
            cmd_add(&alias, username.as_deref(), email.as_deref(), name.as_deref(), main, ssh_key, ssh_alias.as_deref(), no_ssh_setup),
        Cmd::Remove { alias, yes, drop_ssh } => cmd_remove(&alias, yes, drop_ssh),
        Cmd::Use { alias, remote, global } => cmd_use(&alias, &remote, global),
        Cmd::Exec { alias, git_args } => cmd_exec(&alias, &git_args),
        Cmd::Clone { alias, repo, dest } => cmd_clone(&alias, &repo, dest),
        Cmd::GenKey { alias, force } => cmd_gen_key(&alias, force),
        Cmd::Doctor => cmd_doctor(),
        Cmd::Completions { shell } => {
            generate(shell, &mut Cli::command(), "gitswitch", &mut std::io::stdout());
            Ok(())
        }
    }
}

fn cmd_list(json: bool) -> Result<()> {
    let st = store::load()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&st.accounts)?);
        return Ok(());
    }
    if st.accounts.is_empty() {
        println!("No accounts yet.");
        println!("Add your main account:  gitswitch add --alias main --main --email you@mail.com --username You");
        println!("Add an SSH account:     gitswitch add --alias work --username workuser --email work@co.com");
        return Ok(());
    }
    let current_email = git::local_config("user.email").ok().filter(|s| !s.is_empty());
    println!("{:<10} {:<18} {:<30} {:<6} {}", "ALIAS", "USERNAME", "EMAIL", "AUTH", "CURRENT");
    for a in &st.accounts {
        let cur = match &current_email {
            Some(e) if e == &a.email => "*",
            _ => "",
        };
        println!("{:<10} {:<18} {:<30} {:<6} {}", a.alias, a.username, a.email, a.auth, cur);
    }
    Ok(())
}

fn cmd_status(remote: &str, json: bool) -> Result<()> {
    let root = git::repo_root().unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
    let lname = git::local_config("user.name").unwrap_or_default();
    let lemail = git::local_config("user.email").unwrap_or_default();
    let gname = git::global_config("user.name").unwrap_or_default();
    let gemail = git::global_config("user.email").unwrap_or_default();
    let remote_url = git::remote_url(remote).ok();
    let st = store::load()?;
    let matched = if !lemail.is_empty() {
        st.accounts.iter().find(|a| a.email == lemail).map(|a| a.alias.clone())
    } else {
        None
    };
    if json {
        let v = serde_json::json!({
            "repo": root.to_string_lossy(),
            "local": { "name": lname, "email": lemail },
            "global": { "name": gname, "email": gemail },
            "remote": remote_url,
            "account": matched,
        });
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    println!("repo: {}", root.display());
    println!("local user:  {} <{}>", lname, lemail);
    println!("global user: {} <{}>", gname, gemail);
    match remote_url {
        Some(u) => println!("remote {remote}: {u}"),
        None => println!("remote {remote}: (not set)"),
    }
    if !lemail.is_empty() {
        if let Some(a) = st.accounts.iter().find(|a| a.email == lemail) {
            println!("account: {} (auth={}, ssh_alias={})", a.alias, a.auth, a.ssh_alias.as_deref().unwrap_or("-"));
        } else {
            println!("account: (email not in gitswitch store)");
        }
    } else {
        println!("account: (no local user.email — run `gitswitch use <alias>`)");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_add(alias: &str, username: Option<&str>, email: Option<&str>, name: Option<&str>,
           main: bool, ssh_key: Option<PathBuf>, ssh_alias: Option<&str>, no_ssh_setup: bool) -> Result<()> {
    let alias = store::norm_alias(alias)?;
    let mut st = store::load()?;
    if st.accounts.iter().any(|a| a.alias == alias) {
        bail!("account '{alias}' already exists (use `gitswitch remove {alias}` first)");
    }
    let is_main = main || ssh_key.is_none() && {
        // default: if user explicitly passed --main OR no ssh intent? We treat missing key as main
        // only when --main given; otherwise we create an SSH account with default key path.
        false
    };
    if is_main {
        let username = username.map(str::to_string)
            .or_else(|| git::global_config("user.name").ok())
            .unwrap_or_else(|| alias.clone());
        let display = name.map(str::to_string).unwrap_or_else(|| username.clone());
        let email = email.map(str::to_string)
            .or_else(|| git::global_config("user.email").ok())
            .context("pass --email (could not read global git user.email)")?;
        st.accounts.push(store::Account {
            alias: alias.clone(), username, name: display, email,
            auth: "gh".into(), ssh_key: None, ssh_alias: None,
        });
        st.save()?;
        println!("Added main account '{alias}' (gh/HTTPS).");
        println!("Activate in a repo with: gitswitch use {alias}");
        return Ok(());
    }
    // SSH account
    let ssh_alias = ssh_alias.map(str::to_string).unwrap_or_else(|| format!("github-{alias}"));
    let key_path = match ssh_key {
        Some(p) => store::expand_tilde(&p),
        None => dirs::home_dir().context("no home dir")?.join(".ssh").join(format!("id_{alias}")),
    };
    let username = username.unwrap_or(&alias).to_string();
    let display = name.unwrap_or(&username).to_string();
    let email = email.context("pass --email for the SSH account (e.g. --email work@company.com)")?.to_string();
    let acct = store::Account {
        alias: alias.clone(), username, name: display, email,
        auth: "ssh".into(), ssh_key: Some(key_path.to_string_lossy().into_owned()),
        ssh_alias: Some(ssh_alias.clone()),
    };
    if !no_ssh_setup {
        let key_str = key_path.to_string_lossy().into_owned();
        ssh::ensure_host_entry(&ssh_alias, &key_str)
            .context("failed writing ~/.ssh/config")?;
    }
    st.accounts.push(acct);
    st.save()?;
    println!("Added SSH account '{alias}' (Host {ssh_alias}).");
    if !key_path.exists() {
        println!("No key at {} yet — run: gitswitch gen-key {alias}", key_path.display());
        println!("Then add the .pub to GitHub: gh ssh-key add {} --title gitswitch-{alias}", key_path.with_extension("pub").display());
    } else if !no_ssh_setup {
        println!("SSH config Host {ssh_alias} ready. Activate with: gitswitch use {alias}");
    }
    Ok(())
}

fn cmd_remove(alias: &str, yes: bool, drop_ssh: bool) -> Result<()> {
    let mut st = store::load()?;
    let pos = st.accounts.iter().position(|a| a.alias == alias)
        .with_context(|| format!("no account '{alias}' (see `gitswitch list`)"))?;
    let acct = st.accounts.remove(pos);
    st.save()?;
    println!("Removed account '{alias}'.");
    if drop_ssh {
        if let Some(host) = acct.ssh_alias {
            ssh::remove_host_entry(&host)?;
            println!("Removed SSH Host {host} from ~/.ssh/config.");
        }
    } else if acct.auth == "ssh" && !yes {
        println!("Note: SSH Host {} left in ~/.ssh/config (pass --drop-ssh to remove it).", acct.ssh_alias.unwrap_or_default());
    }
    Ok(())
}

fn cmd_use(alias: &str, remote: &str, global: bool) -> Result<()> {
    let st = store::load()?;
    let a = st.accounts.iter().find(|a| a.alias == alias)
        .with_context(|| format!("no account '{alias}' (see `gitswitch list`)"))?;
    if global {
        git::set_global_config("user.name", &a.name)?;
        git::set_global_config("user.email", &a.email)?;
        println!("Set GLOBAL identity to {} <{}>.", a.name, a.email);
        return Ok(());
    }
    git::assert_in_repo().context("`gitswitch use` must run inside a git repo (or pass --global)")?;
    git::set_local_config("user.name", &a.name)?;
    git::set_local_config("user.email", &a.email)?;
    // Clear any stale sshCommand override — we use Host aliases in the remote URL instead.
    let _ = git::unset_local_config("core.sshCommand");
    match a.auth.as_str() {
        "gh" => {
            if let Ok(url) = git::remote_url(remote) {
                if let Some((o, r)) = git::parse_github_repo(&url) {
                    git::set_remote_url(remote, &git::https_url(&o, &r))?;
                    println!("Remote {remote} -> {}", git::https_url(&o, &r));
                }
            }
            println!("Repo now uses '{}' ({} <{}>) via gh/HTTPS.", a.alias, a.name, a.email);
        }
        _ => {
            let host = a.ssh_alias.clone().unwrap_or_else(|| format!("github-{alias}"));
            let key = a.ssh_key.clone().unwrap_or_default();
            if !key.is_empty() {
                ssh::ensure_host_entry(&host, &key)?;
            }
            if let Ok(url) = git::remote_url(remote) {
                if let Some((o, r)) = git::parse_github_repo(&url) {
                    let ssh_url = git::ssh_url(&host, &o, &r);
                    git::set_remote_url(remote, &ssh_url)?;
                    println!("Remote {remote} -> {ssh_url}");
                } else {
                    println!("(remote {remote} URL not recognized as GitHub; identity set but URL left as-is)");
                }
            } else {
                println!("(no remote '{remote}'; identity set, URL untouched)");
            }
            println!("Repo now uses '{}' ({} <{}>) via SSH Host {host}.", a.alias, a.name, a.email);
        }
    }
    Ok(())
}

fn cmd_exec(alias: &str, git_args: &[String]) -> Result<()> {
    let st = store::load()?;
    let a = st.accounts.iter().find(|a| a.alias == alias)
        .with_context(|| format!("no account '{alias}' (see `gitswitch list`)"))?;
    let mut cmd = Command::new("git");
    cmd.args(git_args);
    cmd.env("GIT_AUTHOR_NAME", &a.name);
    cmd.env("GIT_AUTHOR_EMAIL", &a.email);
    cmd.env("GIT_COMMITTER_NAME", &a.name);
    cmd.env("GIT_COMMITTER_EMAIL", &a.email);
    if a.auth == "ssh" {
        if let Some(k) = &a.ssh_key {
            let expanded = store::expand_tilde(&PathBuf::from(k));
            cmd.env("GIT_SSH_COMMAND", format!("ssh -i \"{}\" -o IdentitiesOnly=yes", expanded.display()));
        }
    }
    println!("$ git {}  (as {} <{}>)", git_args.join(" "), a.name, a.email);
    let status = cmd.status().context("failed to run git")?;
    if !status.success() {
        bail!("git exited with {}", status);
    }
    Ok(())
}

fn cmd_clone(alias: &str, repo: &str, dest: Option<PathBuf>) -> Result<()> {
    let st = store::load()?;
    let a = st.accounts.iter().find(|a| a.alias == alias)
        .with_context(|| format!("no account '{alias}' (see `gitswitch list`)"))?;
    let url = if a.auth == "gh" {
        if let Some((o, r)) = git::parse_github_repo(repo) { git::https_url(&o, &r) } else { repo.to_string() }
    } else {
        let host = a.ssh_alias.clone().unwrap_or_else(|| format!("github-{alias}"));
        let key = a.ssh_key.clone().unwrap_or_default();
        if !key.is_empty() { ssh::ensure_host_entry(&host, &key)?; }
        if let Some((o, r)) = git::parse_github_repo(repo) { git::ssh_url(&host, &o, &r) } else { repo.to_string() }
    };
    let mut cmd = Command::new("git");
    cmd.arg("clone").arg(&url);
    if let Some(d) = &dest { cmd.arg(d); }
    if a.auth == "ssh" {
        if let Some(k) = &a.ssh_key {
            let expanded = store::expand_tilde(&PathBuf::from(k));
            cmd.env("GIT_SSH_COMMAND", format!("ssh -i \"{}\" -o IdentitiesOnly=yes", expanded.display()));
        }
    }
    println!("$ git clone {url}");
    let status = cmd.status().context("git clone failed")?;
    if !status.success() { bail!("git clone exited with {status}"); }
    // Set identity inside the clone
    let dest_dir: PathBuf = match dest {
        Some(d) => d,
        None => {
            let name = git::parse_github_repo(&url).map(|(_, r)| r).unwrap_or_else(|| "repo".into());
            PathBuf::from(name.trim_end_matches(".git"))
        }
    };
    let st2 = Command::new("git").arg("-C").arg(&dest_dir)
        .args(["config", "user.name", &a.name]).status()?;
    let st3 = Command::new("git").arg("-C").arg(&dest_dir)
        .args(["config", "user.email", &a.email]).status()?;
    if st2.success() && st3.success() {
        println!("Set {} identity to {} <{}>.", dest_dir.display(), a.name, a.email);
    }
    Ok(())
}

fn cmd_gen_key(alias: &str, force: bool) -> Result<()> {
    let mut st = store::load()?;
    let idx = st.accounts.iter().position(|a| a.alias == alias);
    let (key_path, host): (PathBuf, String) = match idx {
        Some(i) => {
            let a = &st.accounts[i];
            if a.auth == "gh" { bail!("'{alias}' is a main (gh) account — no SSH key needed"); }
            let kp = a.ssh_key.clone().map(PathBuf::from).unwrap_or_else(|| dirs::home_dir().unwrap().join(".ssh").join(format!("id_{alias}")));
            (store::expand_tilde(&kp), a.ssh_alias.clone().unwrap_or_else(|| format!("github-{alias}")))
        }
        None => {
            // bare alias: create key + placeholder account shell? Just make the key.
            let kp = dirs::home_dir().context("no home")?.join(".ssh").join(format!("id_{alias}"));
            (kp, format!("github-{alias}"))
        }
    };
    if key_path.exists() && !force {
        bail!("key {} exists (pass --force to overwrite)", key_path.display());
    }
    if let Some(p) = key_path.parent() { std::fs::create_dir_all(p)?; }
    let status = Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-f", &key_path.to_string_lossy(), "-N", "", "-C", &format!("gitswitch-{alias}")])
        .status().context("ssh-keygen failed — is openssh installed?")?;
    if !status.success() { bail!("ssh-keygen exited with {status}"); }
    ssh::ensure_host_entry(&host, &key_path.to_string_lossy())?;
    println!("Key: {}", key_path.display());
    println!("SSH Host {host} ready.");
    println!("Add to GitHub:  gh ssh-key add {}.pub --title gitswitch-{alias}", key_path.display());
    // if account exists, normalize stored path
    if let Some(i) = idx {
        st.accounts[i].ssh_key = Some(key_path.to_string_lossy().into_owned());
        st.save()?;
    }
    Ok(())
}

fn cmd_doctor() -> Result<()> {
    println!("== gh ==");
    match Command::new("gh").args(["auth", "status"]).status() {
        Ok(s) if s.success() => println!("gh: logged in"),
        _ => println!("gh: NOT logged in (main account pushes will fail)"),
    }
    println!("\n== global git identity ==");
    println!("{} <{}>", git::global_config("user.name").unwrap_or_default(), git::global_config("user.email").unwrap_or_default());
    println!("\n== accounts ==");
    let st = store::load()?;
    if st.accounts.is_empty() { println!("(none)"); }
    for a in &st.accounts {
        let key_ok = match &a.ssh_key {
            None => "n/a (gh)".to_string(),
            Some(k) => {
                let p = store::expand_tilde(&PathBuf::from(k));
                if p.exists() { format!("key OK ({k})") } else { format!("MISSING key ({k})") }
            }
        };
        let host_ok = match &a.ssh_alias {
            None => "".to_string(),
            Some(h) => if ssh::host_exists(h).unwrap_or(false) { format!(", Host {h} OK") } else { format!(", Host {h} MISSING") },
        };
        println!("- {} [{}] {} <{}> — {key_ok}{host_ok}", a.alias, a.auth, a.name, a.email);
    }
    println!("\n== current repo ==");
    match git::repo_root() {
        Ok(r) => {
            println!("root: {}", r.display());
            println!("local: {} <{}>", git::local_config("user.name").unwrap_or_default(), git::local_config("user.email").unwrap_or_default());
            match git::remote_url("origin") {
                Ok(u) => println!("origin: {u}"),
                Err(_) => println!("origin: (not set)"),
            }
        }
        Err(_) => println!("(not inside a git repo)"),
    }
    Ok(())
}

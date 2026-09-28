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
    /// Guided first-time setup: detect the gh user + global git identity and
    /// register the main account. Interactive by default; agents use --auto.
    Setup {
        /// Non-interactive: take everything from `gh` + global git config
        #[arg(long)]
        auto: bool,
        /// Alias for the main account (default: main)
        #[arg(long, default_value = "main")]
        main_alias: String,
    },
    /// Check gh auth, keys, ssh config and current repo
    Doctor {
        /// Machine-readable output (for scripts and AI agents)
        #[arg(long)]
        json: bool,
        /// Repair what can be repaired: recreate missing SSH Host blocks and
        /// fix the current repo's remote URL when its identity matches an account
        #[arg(long)]
        fix: bool,
    },
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
        Cmd::Setup { auto, main_alias } => cmd_setup(auto, &main_alias),
        Cmd::Doctor { json, fix } => cmd_doctor(json, fix),
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
        push_main_account(&mut st, &alias, &username, &display, &email)?;
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

fn push_main_account(st: &mut store::Store, alias: &str, username: &str, name: &str, email: &str) -> Result<()> {
    if st.accounts.iter().any(|a| a.alias == alias) {
        bail!("account '{alias}' already exists (use `gitswitch remove {alias}` first)");
    }
    st.accounts.push(store::Account {
        alias: alias.to_string(),
        username: username.to_string(),
        name: name.to_string(),
        email: email.to_string(),
        auth: "gh".into(), ssh_key: None, ssh_alias: None,
    });
    st.save()
}

fn prompt(label: &str, default: &str) -> Result<String> {
    use std::io::{BufRead, Write};
    print!("{label} [{default}]: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let line = line.trim().to_string();
    Ok(if line.is_empty() { default.to_string() } else { line })
}

fn gh_login() -> Result<String> {
    let out = Command::new("gh").args(["api", "user", "--jq", ".login"]).output()
        .context("`gh` not found — install it and run `gh auth login` first")?;
    if !out.status.success() {
        bail!("not logged into gh (run `gh auth login` first)");
    }
    let login = String::from_utf8_lossy(&out.stdout).trim().trim_matches('"').to_string();
    if login.is_empty() { bail!("could not read gh username"); }
    Ok(login)
}

fn cmd_setup(auto: bool, main_alias: &str) -> Result<()> {
    let main_alias = store::norm_alias(main_alias)?;
    let login = gh_login()?;
    let mut st = store::load()?;
    if st.accounts.iter().any(|a| a.alias == main_alias) {
        println!("Account '{main_alias}' already registered — nothing to do.");
    } else {
        let gname = git::global_config("user.name").unwrap_or_default();
        let gemail = git::global_config("user.email").unwrap_or_default();
        let (username, name, email) = if auto {
            if gemail.is_empty() {
                bail!("global git user.email is empty — set it (`git config --global user.email you@mail`) or run setup interactively");
            }
            (login.clone(), if gname.is_empty() { login.clone() } else { gname }, gemail)
        } else {
            println!("Detected gh user: {login}");
            let username = prompt("GitHub username", &login)?;
            let name = prompt("Display name for commits", if gname.is_empty() { &username } else { &gname })?;
            let email = prompt("Commit email", &gemail)?;
            if email.is_empty() { bail!("email is required"); }
            (username, name, email)
        };
        push_main_account(&mut st, &main_alias, &username, &name, &email)?;
        println!("Registered main account '{main_alias}' ({name} <{email}>) via gh/HTTPS.");
    }
    println!("\nNext: add an extra account (repeat per account):");
    println!("  gitswitch add --alias work --username <gh-user> --email <mail>");
    println!("  gitswitch gen-key work");
    println!("  gh ssh-key add ~/.ssh/id_work.pub --title gitswitch-work");
    println!("Agents: see AGENTS.md / SKILL.md (`status --json`, `exec <alias> -- …`).");
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
    apply_account_to_repo(a, &a.alias.clone(), remote)
}

/// Set identity-related repo state for an account: clear stale sshCommand
/// overrides and rewrite the remote URL (SSH url for ssh accounts, HTTPS for gh).
/// Caller sets user.name/user.email (or relies on them already matching).
fn apply_account_to_repo(a: &store::Account, alias: &str, remote: &str) -> Result<()> {
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

fn cmd_doctor(json: bool, fix: bool) -> Result<()> {
    let gh_ok = Command::new("gh").args(["auth", "status"]).status().map(|s| s.success()).unwrap_or(false);
    let gname = git::global_config("user.name").unwrap_or_default();
    let gemail = git::global_config("user.email").unwrap_or_default();
    let st = store::load()?;

    struct AcctHealth { alias: String, auth: String, key_ok: bool, key: String, host_ok: bool, host: String }
    let mut health = Vec::new();
    for a in &st.accounts {
        let (key_ok, key) = match &a.ssh_key {
            None => (true, String::new()),
            Some(k) => (store::expand_tilde(&PathBuf::from(k)).exists(), k.clone()),
        };
        let (host_ok, host) = match &a.ssh_alias {
            None => (true, String::new()),
            Some(h) => (ssh::host_exists(h).unwrap_or(false), h.clone()),
        };
        // --fix: recreate missing SSH Host blocks (key path may not exist yet —
        // the block still helps, and gen-key refreshes it).
        if fix && a.auth == "ssh" && !host_ok && !host.is_empty() && !key.is_empty() {
            match ssh::ensure_host_entry(&host, &key) {
                Ok(()) => println!("fixed: recreated SSH Host {host} for '{}'", a.alias),
                Err(e) => println!("could not fix SSH Host {host}: {e:#}"),
            }
        }
        health.push(AcctHealth { alias: a.alias.clone(), auth: a.auth.clone(), key_ok, key, host_ok: if fix && a.auth == "ssh" && !host.is_empty() { ssh::host_exists(&host).unwrap_or(false) } else { host_ok }, host });
    }

    // --fix: if the current repo's local email matches an account, repair its
    // remote URL (same rewrite `use` performs, without touching identity).
    let in_repo = git::repo_root().is_ok();
    let lemail = if in_repo { git::local_config("user.email").unwrap_or_default() } else { String::new() };
    if fix && in_repo && !lemail.is_empty() {
        if let Some(a) = st.accounts.iter().find(|a| a.email == lemail) {
            let alias = a.alias.clone();
            if let Err(e) = apply_account_to_repo(a, &alias, "origin") {
                println!("could not fix remote URL: {e:#}");
            }
        }
    }

    if json {
        let v = serde_json::json!({
            "gh_logged_in": gh_ok,
            "global": { "name": gname, "email": gemail },
            "accounts": health.iter().map(|h| serde_json::json!({
                "alias": h.alias, "auth": h.auth,
                "key_ok": h.key_ok, "key": h.key,
                "host_ok": h.host_ok, "host": h.host,
            })).collect::<Vec<_>>(),
            "repo": match git::repo_root() {
                Ok(r) => serde_json::json!({
                    "root": r.to_string_lossy(),
                    "local": { "name": git::local_config("user.name").unwrap_or_default(),
                               "email": git::local_config("user.email").unwrap_or_default() },
                    "origin": git::remote_url("origin").ok(),
                }),
                Err(_) => serde_json::Value::Null,
            },
        });
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }

    println!("== gh ==");
    println!("{}", if gh_ok { "gh: logged in" } else { "gh: NOT logged in (main account pushes will fail)" });
    println!("\n== global git identity ==");
    println!("{gname} <{gemail}>");
    println!("\n== accounts ==");
    if health.is_empty() { println!("(none — run `gitswitch setup`)"); }
    for h in &health {
        let key_s = if h.auth == "gh" { "n/a (gh)".to_string() }
            else if h.key_ok { format!("key OK ({})", h.key) } else { format!("MISSING key ({})", h.key) };
        let host_s = if h.host.is_empty() { String::new() }
            else if h.host_ok { format!(", Host {} OK", h.host) } else { format!(", Host {} MISSING", h.host) };
        println!("- {} [{}] — {key_s}{host_s}", h.alias, h.auth);
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

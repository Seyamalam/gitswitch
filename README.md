# gitswitch — multiple GitHub accounts at the same time

Stay logged into all your GitHub accounts at once. Your **main** account keeps
working exactly as today (via `gh` + HTTPS); every extra account gets its own
SSH key. Switch a repo between accounts with one command, or run a single git
command as another account without switching anything.

## How it works

- Accounts live in `~/.config/gitswitch/accounts.toml`
  (macOS: `~/Library/Application Support/gitswitch`,
  Windows: `%APPDATA%\gitswitch`).
- Each SSH account gets a managed block in `~/.ssh/config`
  (`%USERPROFILE%\.ssh\config` on Windows):
  `Host github-<alias>` → `HostName github.com` + its `IdentityFile`.
- `gitswitch use <alias>` sets the repo's local `user.name`/`user.email` and
  rewrites the `origin` remote: an SSH URL for SSH accounts, an HTTPS URL for
  the `gh` account.
- `gitswitch exec <alias> -- <git …>` runs one git command with that account's
  identity and SSH key via environment variables — repo config untouched.
  This is what AI agents (and scripts) should use.

## Prerequisites

| Need | Linux | macOS | Windows |
|---|---|---|---|
| Git | distro package | Xcode CLT / `brew install git` | `winget install Git.Git` |
| `gh` CLI + logged in | `gh auth login` | `brew install gh && gh auth login` | `winget install GitHub.cli` then `gh auth login` |
| SSH client | openssh (preinstalled) | preinstalled | preinstalled (Win 10+) |
| To build | `cargo` ([rustup](https://rustup.rs)) | same | same |

Check yours: `git --version && gh auth status && ssh -V`.

## Install

Everything builds locally — no CI, no downloads of prebuilt binaries.
Pick the one-liner for your OS (installs Rust via rustup if needed):

```bash
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/Seyamalam/gitswitch/main/install.sh | bash
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/Seyamalam/gitswitch/main/install.ps1 | iex
```

Or manually (all OSes):

```bash
git clone https://github.com/Seyamalam/gitswitch.git
cd gitswitch
cargo install --path .
```

**Shell completions (optional):**

```bash
# bash
gitswitch completions bash >> ~/.bash_completion
# zsh
gitswitch completions zsh > ~/.zfunc/_gitswitch
# fish
gitswitch completions fish > ~/.config/fish/completions/gitswitch.fish
# powershell (add to $PROFILE)
gitswitch completions powershell | Out-String | Invoke-Expression
```

## Setup

### 1. Register your main account (2 min)

This is the account you're already logged into with `gh`. Nothing about your
current workflow changes.

```bash
gitswitch add --alias main --main
```

No flags needed — it reads your name/email from your global git config.
Verify with `gitswitch list`.

### 2. Add a second account (5 min)

```bash
# 1) register it (creates the ~/.ssh/config entry)
gitswitch add --alias work --username <github-username> --email <work@email.com>

# 2) generate its SSH key
gitswitch gen-key work

# 3) upload the public key to that GitHub account
gh ssh-key add ~/.ssh/id_work.pub --title gitswitch-work
# (log into the *work* account in the browser first if gh asks, or paste the
#  .pub contents at github.com → Settings → SSH and GPG keys)

# 4) test it (type "yes" if asked about the host key)
ssh -T git@github-work
# expected: "Hi <github-username>! You've successfully authenticated..."
```

Repeat for a third, fourth, … account. Each gets its own key and
`github-<alias>` host — they never interfere.

### 3. Use it in a repo

```bash
cd ~/my-project
gitswitch status                # what identity is this repo using?
gitswitch use work              # switch: identity + origin -> git@github-work:owner/repo.git
# ... work, commit, push normally ...
gitswitch use main              # switch back -> https://github.com/owner/repo.git
```

Cloning straight into the right account also works:

```bash
gitswitch clone work myorg/myrepo
gitswitch clone main myorg/myrepo ~/code/myrepo
```

## One-shot commands (no switching)

Run any git command **as** an account while the repo stays on its own identity.
Ideal for quick fixes, scripts, and AI agents:

```bash
gitswitch exec work -- commit -m "fix: ..."
gitswitch exec work -- push
gitswitch exec main -- pull --rebase
```

Everything after `--` is passed to `git` verbatim.

## For AI agents

If an AI coding agent works in your repos, point it at the repo's
[`AGENTS.md`](AGENTS.md) (full contract) and [`SKILL.md`](SKILL.md) (skill
format). The short version:

- Agents discover accounts with `gitswitch list --json` and the repo's account
  with `gitswitch status --json` (both machine-readable).
- Agents commit/push with `gitswitch exec <alias> -- …` — no config changes.
- If something's wrong, `gitswitch doctor` diagnoses gh login, keys, SSH
  config, and repo state in one shot.

## Commands

| Command | Purpose |
|---|---|
| `gitswitch list [--json]` | show accounts (`*` = current repo's; `--json` for scripts/agents) |
| `gitswitch status [--json] [--remote origin]` | show repo identity, remote URL, matched account |
| `gitswitch add --alias … [--main \| --ssh-key …]` | register an account (see `--help` for all flags) |
| `gitswitch gen-key <alias> [--force]` | create ed25519 key + SSH Host entry |
| `gitswitch use <alias> [--remote origin] [--global]` | switch current repo (or global) identity |
| `gitswitch exec <alias> -- <git args…>` | run one git command as that account |
| `gitswitch clone <alias> <owner/repo\|url> [dest]` | clone with the right URL + identity |
| `gitswitch remove <alias> [--drop-ssh]` | delete account (optionally its SSH block) |
| `gitswitch doctor` | check gh auth, keys, SSH config, current repo |
| `gitswitch completions <shell>` | print shell completions |
| `gitswitch --help` / `gitswitch <cmd> --help` | full help for everything |

## macOS & Windows notes

- **Config location:** `~/Library/Application Support/gitswitch` (macOS),
  `%APPDATA%\gitswitch` (Windows), `~/.config/gitswitch` (Linux).
- **SSH config:** same `Host github-<alias>` mechanism on all OSes; on Windows
  it's `%USERPROFILE%\.ssh\config`. Key paths with spaces (e.g.
  `C:\Users\First Last\...`) are quoted automatically.
- **macOS + passphrase keys:** if you set a passphrase during `gen-key`
  (default is none), add the key to the keychain once:
  `ssh-add --apple-use-keychain ~/.ssh/id_<alias>`.
- **Windows + `ssh -T`:** run it in PowerShell or Git Bash; accept the host key
  on first use.

## Troubleshooting

```bash
gitswitch doctor
```

| Symptom | Likely cause / fix |
|---|---|
| `Permission denied (publickey)` on push | Key not added to that GitHub account → re-run `gh ssh-key add ~/.ssh/id_<alias>.pub`; check `ssh -T git@github-<alias>` |
| Repo pushes to the wrong account | `gitswitch status` → `gitswitch use <alias>` to switch |
| `gh: NOT logged in` in doctor | `gh auth login` (main/HTTPS pushes need it) |
| Agent committed with wrong email | Have it use `gitswitch exec <alias> -- …` (see `AGENTS.md`) |
| SSH Host missing in config | `gitswitch use <alias>` re-creates it, or re-run `add` without `--no-ssh-setup` |

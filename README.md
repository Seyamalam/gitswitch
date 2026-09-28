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
gitswitch setup --auto
```

(`setup` detects your `gh` user and global git identity. Prefer the manual
route? `gitswitch add --alias main --main` does the same thing.)
Verify with `gitswitch list`.

### 2. Add a second account (5 min)

Agent-led (recommended) — the agent runs it and tells you exactly where to
click on GitHub:

```bash
gitswitch onboard --alias work --username <github-username> --email <work@email.com>
```

`onboard` registers the account, generates its key, prints numbered steps
(github.com → Settings → SSH keys, title `gitswitch-work`, key included),
waits for you, then verifies with a live `ssh -T` check.
Manual equivalent: `add` → `gen-key` → `gh ssh-key add ~/.ssh/id_work.pub`.

Repeat per account. Each gets its own key and `github-<alias>` host.

### 3. (Optional) Tokens — let an account open PRs and issues

SSH covers git push/pull; the `gh` CLI needs a token to act *as* that account:

1. Logged in as that account: github.com → Settings → Developer settings →
   Personal access tokens → Tokens (classic) → Generate new token (classic)
2. Note `gitswitch-<alias>`, scopes **repo** + **workflow**, Generate
3. Store it (input hidden, never shown): `gitswitch set-token <alias>`
4. Use it: `gitswitch gh <alias> -- pr create --title "…" --body "…"`

Tokens live in `accounts.toml` (mode 0600) and are never printed anywhere.

### 4. Use it in a repo

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

Everything after `--` is passed to `git` verbatim. For `gh` (PRs, issues,
releases) as an account:

```bash
gitswitch gh work -- pr create --title "Add feature" --body "…" --head work-feature
gitswitch gh work -- pr merge 3 --squash --delete-branch
gitswitch gh work -- issue list
```

Uses the account's stored token, falling back to your `gh` login.

## Mock-hackathon playbook (simulate multiple contributors)

Each "person" is an account with a distinct email — GitHub's contributors
graph keys off commit emails, so they show up as distinct contributors.
An agent can run the whole demo; you only click on GitHub during onboarding.

```bash
# 1) onboard one account per persona (agent prints your click-steps each time)
gitswitch onboard --alias alice --username alicegh --email alice@demo.com
gitswitch onboard --alias bob   --username bobgh   --email bob@demo.com

# 2) optional: tokens so each persona opens their own PRs (see §3 above)
gitswitch set-token alice
gitswitch set-token bob

# 3) the demo loop, per persona — repo config never changes:
git checkout -b alice-feature
# ... work happens (by you or an agent) ...
gitswitch exec alice -- commit -am "feat: alice's feature"
gitswitch exec alice -- push -u origin alice-feature
gitswitch gh alice -- pr create --title "Alice's feature" --body "…" --head alice-feature
gitswitch gh alice -- pr merge <n> --squash --delete-branch
# repeat as bob on bob-feature → two contributors, two merged PRs
```

Tips: open PRs with `--fill` to skip the editor; agents should confirm the
account + repo with you before each push/PR (see `AGENTS.md`).

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
| `gitswitch onboard --alias … --username … --email … [--no-wait]` | agent-led setup: register + key + GitHub steps + verify |
| `gitswitch verify <alias>` | live access check (`ssh -T`) |
| `gitswitch set-token <alias> [--token …]` | store a classic PAT for `gh` as that account |
| `gitswitch gh <alias> -- <gh args…>` | run gh as that account (stored token or login) |
| `gitswitch remove <alias> [--drop-ssh]` | delete account (optionally its SSH block) |
| `gitswitch setup [--auto]` | first-time setup: register main from `gh` + global git config (`--auto` = non-interactive, for agents) |
| `gitswitch doctor [--json] [--fix]` | check everything (`--json` for agents; `--fix` recreates SSH blocks + repairs repo URL) |
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

---
name: gitswitch
description: Commit and push with the correct GitHub account when several are configured. Use when you need to check which git identity a repo uses, or commit/push as a specific account (main via gh/HTTPS, extras via SSH) without touching repo config.
---

# gitswitch skill

`gitswitch` manages multiple GitHub accounts on one machine: one `main`
account via `gh`/HTTPS, extra accounts via SSH keys.

## Discover state first (JSON is machine-readable)

```bash
gitswitch list --json      # [{alias, username, email, auth, ssh_key, ssh_alias}]
gitswitch status --json    # {repo, local, global, remote, account} — account is null if unmanaged
```

## Commit / push as an account (preferred: no side effects)

`exec` runs any git command with that account's identity + SSH key via
environment variables. Repo config is left untouched.

```bash
gitswitch exec <alias> -- status
gitswitch exec <alias> -- add -A
gitswitch exec <alias> -- commit -m "<type>: <subject>"
gitswitch exec <alias> -- push
```

## Switch a repo's account (only when the user asks)

```bash
gitswitch use <alias>      # sets local user.name/email, rewrites origin URL
```

Never use `use --global`. Never rewrite remotes to "fix" auth — use `exec`.

## Diagnose and repair

```bash
gitswitch doctor            # gh login, keys, ~/.ssh/config entries, current repo
gitswitch doctor --json     # same, machine-readable
gitswitch doctor --fix      # recreate missing SSH blocks, repair repo remote URL
```

If auth fails, surface `doctor` output to the user instead of retrying.

## First-time setup (only when the user asks for it)

```bash
gitswitch setup --auto      # register main from gh login + global git config
```

Extra accounts need a username + email from the user — ask, don't invent.
After `gitswitch gen-key <alias>`, STOP: the user must upload the `.pub`
key to that GitHub account themselves. Never upload keys or run `gh auth login`.
See `AGENTS.md` for the full agent contract.

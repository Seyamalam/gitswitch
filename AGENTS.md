# AGENTS.md — instructions for AI coding agents

This repo is `gitswitch`, a CLI for using multiple GitHub accounts at once.
When you (the agent) need to commit or push, use `gitswitch` — do NOT change
the user's global git identity and do NOT rewrite their remotes unless asked.

## The 3 rules

1. **Discover, don't assume.** Run `gitswitch status --json` in the repo and
   `gitswitch list --json` to see available accounts before committing.
2. **Prefer `exec` (one-shot, no side effects).** It sets the author/committer
   and SSH key via environment variables only — repo config stays untouched:
   `gitswitch exec <alias> -- commit -m "msg"`
   `gitswitch exec <alias> -- push`
3. **Never run `gitswitch use --global`.** `use` rewrites the repo's remote URL.
   Only use it when the user explicitly asks to switch the repo's account.

## Recipes

```bash
gitswitch list --json                 # all accounts (alias, email, auth)
gitswitch status --json               # which account this repo uses -> {"account": "work"|"main"|null, ...}
gitswitch exec work -- status         # any git command as an account
gitswitch exec work -- commit -m "fix: ..."
gitswitch exec work -- push
gitswitch exec main -- push           # push via gh/HTTPS account
gitswitch doctor                      # something broken? start here
gitswitch doctor --json               # same, machine-readable
gitswitch doctor --fix                # recreate missing SSH blocks, repair this repo's remote URL
```

## First-time setup (you may run this for the user)

```bash
gitswitch setup --auto                # register main from gh login + global git config
# extra account (needs username + email from the user — ask, don't invent):
gitswitch onboard --alias work --username <gh-user> --email <mail> [--no-wait]
# onboard prints numbered GitHub steps (settings/keys) for the HUMAN — relay
# them verbatim, wait for confirmation, then run `gitswitch verify work`.
```

Never upload SSH keys, never run `gh auth login`, never invent emails.

## Tokens (PRs / issues as another account)

- `gitswitch gh <alias> -- <gh args…>` runs gh as that account (stored token
  or your login). Example: `gitswitch gh alice -- pr create --title "…" --fill`
- Tokens live in `~/.config/gitswitch/accounts.toml` (0600) and are NEVER
  printed — not even by `list --json` (it only shows `has_token`).
- To store one, have the user create it (github.com → that account →
  Settings → Developer settings → Personal access tokens → Tokens (classic) →
  Generate, note `gitswitch-<alias>`, scopes `repo` + `workflow`), then run
  `gitswitch set-token <alias>` and let the USER paste at the hidden prompt.
- Rules: never print, log, or echo a token; never pass one with --token in a
  shared transcript — prefer the hidden prompt. Tokens authorize pushes/PRs,
  so confirm the account + repo with the user before pushing or opening a PR.

## Hackathon simulation (multiple contributors)

Each contributor is an account. The loop per contributor, all without
touching repo config:
```bash
gitswitch exec alice -- commit -m "feat: …"
gitswitch exec alice -- push -u origin alice-feature
gitswitch gh alice -- pr create --title "…" --body "…" --head alice-feature
```
GitHub's contributors graph keys off commit emails, so distinct account
emails show distinct contributors. Merge with `gitswitch gh <alias> -- pr merge <n> --squash --delete-branch` (confirm first).

## Model

- Account `auth: "gh"` ("main") pushes over HTTPS using the user's `gh` login.
- Accounts with `auth: "ssh"` push over SSH via `~/.ssh/config` Host aliases
  (`github-<alias>`) + `GIT_SSH_COMMAND`. No extra setup needed from you.
- If `status --json` shows `"account": null`, the repo identity isn't managed
  yet — still use `exec <alias> -- ...` for your commits; don't "fix" it with
  `use` unless the user asked.
- If a push fails with auth errors, report the output of `gitswitch doctor`
  instead of retrying with different credentials.

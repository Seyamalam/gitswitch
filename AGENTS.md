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
gitswitch add --alias work --username <gh-user> --email <mail>
gitswitch gen-key work                # prints the `gh ssh-key add ...` command;
                                      # STOP here — the user must upload the key themselves
```

Never upload SSH keys, never run `gh auth login`, never invent emails.

## Model

- Account `auth: "gh"` ("main") pushes over HTTPS using the user's `gh` login.
- Accounts with `auth: "ssh"` push over SSH via `~/.ssh/config` Host aliases
  (`github-<alias>`) + `GIT_SSH_COMMAND`. No extra setup needed from you.
- If `status --json` shows `"account": null`, the repo identity isn't managed
  yet — still use `exec <alias> -- ...` for your commits; don't "fix" it with
  `use` unless the user asked.
- If a push fails with auth errors, report the output of `gitswitch doctor`
  instead of retrying with different credentials.

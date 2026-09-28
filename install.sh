#!/usr/bin/env bash
# gitswitch installer — builds locally from source (no CI, no binary downloads).
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Seyamalam/gitswitch/main/install.sh | bash
# Env:
#   GITSWITCH_VERSION  branch/tag to build (default: main)
#   GITSWITCH_SOURCE   local checkout to build instead of cloning (used for testing)
set -euo pipefail

REPO="Seyamalam/gitswitch"
VERSION="${GITSWITCH_VERSION:-main}"

command -v git >/dev/null || { echo "gitswitch: git is required but not installed." >&2; exit 1; }

if ! command -v cargo >/dev/null; then
  echo "gitswitch: installing Rust toolchain (rustup) ..." >&2
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi

if [ -n "${GITSWITCH_SOURCE:-}" ]; then
  SRC="$GITSWITCH_SOURCE"
else
  SRC="$(mktemp -d)/gitswitch"
  git clone --depth 1 --branch "$VERSION" "https://github.com/$REPO.git" "$SRC"
  trap 'rm -rf "$(dirname "$SRC")"' EXIT
fi

cargo install --locked --path "$SRC"
gitswitch --version

case ":$PATH:" in
  *":$HOME/.cargo/bin:"*) ;;
  *) echo "NOTE: \$HOME/.cargo/bin is not on your PATH. Add this to your shell profile:" >&2
     echo "  export PATH=\"\$HOME/.cargo/bin:\$PATH\"" >&2 ;;
esac

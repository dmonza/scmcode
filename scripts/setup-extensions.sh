#!/usr/bin/env bash
# Populate extensions/ with the built-in extensions scmCode ships.
#
# Source: Code - OSS (https://github.com/microsoft/vscode, MIT License), fetched at the
# commit pinned below. Only the extensions listed in scripts/builtin-extensions.txt are
# copied, together with Code - OSS's LICENSE.txt and ThirdPartyNotices.txt.
#
# This script never copies from an installed editor (Visual Studio Code, Cursor) or from
# ~/.vscode/extensions. Those are other vendors' distributions and Marketplace extensions,
# under licenses that do not allow scmCode to redistribute them.
#
# Re-running is cheap: extensions/.scmcode-source records what was installed, and the
# folder is rebuilt from scratch whenever the pinned commit or the list changes, or when
# extensions/ was filled by anything other than this script.
#
# Works with the bash 3.2 that ships with macOS and with Git Bash on Windows.
set -euo pipefail

CODE_OSS_REPO="https://github.com/microsoft/vscode.git"
CODE_OSS_VERSION="1.115.0"
# Tag 1.115.0. Fetching by commit makes the content immutable even if the tag moves.
CODE_OSS_COMMIT="41dd792b5e652393e7787322889ed5fdc58bd75b"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXTENSIONS_DIR="$REPO_ROOT/extensions"
ALLOWLIST="$REPO_ROOT/scripts/builtin-extensions.txt"
STAMP_FILE="$EXTENSIONS_DIR/.scmcode-source"

fail() {
  echo "setup-extensions: $*" >&2
  exit 1
}

command -v git >/dev/null 2>&1 || fail "git is required."
[ -f "$ALLOWLIST" ] || fail "missing $ALLOWLIST"

# Read the allowlist: one folder name per line, '#' starts a comment. tr strips the CR of a
# CRLF checkout.
EXTENSIONS=()
while IFS= read -r line || [ -n "$line" ]; do
  name="$(printf '%s' "${line%%#*}" | tr -d '[:space:]')"
  [ -z "$name" ] && continue
  case "$name" in
    *[!A-Za-z0-9._-]*) fail "invalid extension name in allowlist: '$name'" ;;
  esac
  EXTENSIONS+=("$name")
done < "$ALLOWLIST"
[ "${#EXTENSIONS[@]}" -gt 0 ] || fail "the allowlist is empty."

EXPECTED_STAMP="code-oss-version=$CODE_OSS_VERSION
code-oss-commit=$CODE_OSS_COMMIT
extensions=$(IFS=,; printf '%s' "${EXTENSIONS[*]}")"

if [ -f "$STAMP_FILE" ] && [ "$(tr -d '\r' < "$STAMP_FILE")" = "$EXPECTED_STAMP" ]; then
  echo "extensions/ is up to date (Code - OSS $CODE_OSS_VERSION, ${#EXTENSIONS[@]} extensions)."
  exit 0
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
SRC="$TMP_DIR/code-oss"

echo "Fetching Code - OSS $CODE_OSS_VERSION ($CODE_OSS_COMMIT), ${#EXTENSIONS[@]} extensions..."
git init -q "$SRC"
git -C "$SRC" remote add origin "$CODE_OSS_REPO"
SPARSE_PATHS=(/LICENSE.txt /ThirdPartyNotices.txt)
for ext in "${EXTENSIONS[@]}"; do
  SPARSE_PATHS+=("/extensions/$ext/")
done
# Patterns go through stdin: as arguments, Git Bash on Windows would rewrite "/LICENSE.txt"
# into a Windows path before git.exe sees it.
printf '%s\n' "${SPARSE_PATHS[@]}" | git -C "$SRC" sparse-checkout set --no-cone --stdin
git -C "$SRC" fetch -q --depth 1 --filter=blob:none origin "$CODE_OSS_COMMIT"
git -C "$SRC" -c advice.detachedHead=false checkout -q FETCH_HEAD

ACTUAL_COMMIT="$(git -C "$SRC" rev-parse HEAD)"
[ "$ACTUAL_COMMIT" = "$CODE_OSS_COMMIT" ] ||
  fail "fetched commit $ACTUAL_COMMIT, expected $CODE_OSS_COMMIT"

for ext in "${EXTENSIONS[@]}"; do
  [ -f "$SRC/extensions/$ext/package.json" ] ||
    fail "extension '$ext' is not in Code - OSS $CODE_OSS_VERSION. Update scripts/builtin-extensions.txt."
done
for f in LICENSE.txt ThirdPartyNotices.txt; do
  [ -f "$SRC/$f" ] || fail "Code - OSS $f is missing from the checkout."
done

case "$EXTENSIONS_DIR" in
  */extensions) ;;
  *) fail "refusing to replace unexpected path $EXTENSIONS_DIR" ;;
esac
if [ -e "$EXTENSIONS_DIR" ]; then
  echo "Replacing the previous contents of extensions/."
  rm -rf "$EXTENSIONS_DIR"
fi
mkdir -p "$EXTENSIONS_DIR"

for ext in "${EXTENSIONS[@]}"; do
  cp -R "$SRC/extensions/$ext" "$EXTENSIONS_DIR/$ext"
done
# Code - OSS's own license and notices travel with the extensions (they end up in dist/).
cp "$SRC/LICENSE.txt" "$EXTENSIONS_DIR/LICENSE-Code-OSS.txt"
cp "$SRC/ThirdPartyNotices.txt" "$EXTENSIONS_DIR/ThirdPartyNotices-Code-OSS.txt"
printf '%s\n' "$EXPECTED_STAMP" > "$STAMP_FILE"

echo "Installed ${#EXTENSIONS[@]} built-in extensions from Code - OSS $CODE_OSS_VERSION."

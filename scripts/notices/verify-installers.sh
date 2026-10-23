#!/usr/bin/env bash
# Check that every Linux or macOS package in a folder carries scmCode's legal notices:
# LICENSE, NOTICE.md and THIRD_PARTY_NOTICES.txt. Release builds run it on the collected
# artifacts (.github/workflows/release.yml). The Windows installers (.exe/.msi) are not
# checked yet; that step is still pending (docs/issues/ISSUE-0021).
#
#   bash scripts/notices/verify-installers.sh dist-artifacts
set -euo pipefail

ARTIFACTS="${1:?usage: verify-installers.sh <artifacts-folder>}"
REQUIRED=(LICENSE NOTICE.md THIRD_PARTY_NOTICES.txt)
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
failures=0
checked=0

# $1: package name; stdin: the package's file paths, one per line. Called with input
# redirection, not in a pipeline, so the counters below survive.
check_listing() {
  local pkg="$1" listing missing=()
  listing="$(cat)"
  for f in "${REQUIRED[@]}"; do
    if ! printf '%s\n' "$listing" | grep -Eq "(^|/)${f//./\\.}\$"; then
      missing+=("$f")
    fi
  done
  checked=$((checked + 1))
  if [ "${#missing[@]}" -gt 0 ]; then
    echo "MISSING in $pkg: ${missing[*]}"
    failures=$((failures + 1))
  else
    echo "ok: $pkg"
  fi
}

shopt -s nullglob
for pkg in "$ARTIFACTS"/*; do
  name="$(basename "$pkg")"
  case "$name" in
    *.deb)
      check_listing "$name" < <(dpkg-deb -c "$pkg" | awk '{print $NF}')
      ;;
    *.rpm)
      command -v rpm >/dev/null || { sudo apt-get install -y -qq rpm >/dev/null; }
      check_listing "$name" < <(rpm -qlp "$pkg" 2>/dev/null)
      ;;
    *.AppImage)
      dir="$WORK/appimage"
      rm -rf "$dir" && mkdir -p "$dir"
      cp "$pkg" "$dir/app.AppImage" && chmod +x "$dir/app.AppImage"
      (cd "$dir" && ./app.AppImage --appimage-extract >/dev/null)
      check_listing "$name" < <(cd "$dir/squashfs-root" && find . -type f)
      ;;
    *.dmg)
      mnt="$WORK/dmg"
      mkdir -p "$mnt"
      # The DMG carries LICENSE as a license agreement; answer it non-interactively.
      { yes || true; } | PAGER='cat' hdiutil attach -nobrowse -readonly -noverify -mountpoint "$mnt" "$pkg" >/dev/null
      check_listing "$name" < <(cd "$mnt" && find . -type f)
      hdiutil detach "$mnt" -quiet || hdiutil detach "$mnt" -force -quiet
      ;;
    *.app.tar.gz)
      check_listing "$name" < <(tar -tzf "$pkg")
      ;;
    *) ;;
  esac
done

if [ "$checked" -eq 0 ]; then
  echo "verify-installers: no Linux or macOS packages found in $ARTIFACTS" >&2
  exit 1
fi
if [ "$failures" -gt 0 ]; then
  echo "verify-installers: $failures package(s) lack the legal notices." >&2
  exit 1
fi
echo "All $checked package(s) include ${REQUIRED[*]}."

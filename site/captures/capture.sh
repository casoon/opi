#!/usr/bin/env bash
# Regenerates the terminal captures shown on the site: opi run on its own repository.
# Colour needs a terminal, so the coloured ones run under script(1) (BSD/macOS syntax).
#   site/captures/capture.sh   (after cargo build --release)
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
out="$repo/site/captures"
opi="$repo/target/release/opi"
cd "$repo"

tidy() { perl -pi -e 's/^\^D\x08\x08// if $. == 1; s/\r$//' "$1"; }
pty() { script -q "$out/$1" "$opi" "${@:2}" </dev/null >/dev/null 2>&1 || true; tidy "$out/$1"; }

"$opi" | cat > "$out/list.txt"
pty health.ansi --health
pty commit.ansi --check commit
pty security.ansi --security
"$opi" --updates 2>&1 | cat > "$out/updates.txt" || true
"$opi" --clean 2>&1 | cat > "$out/clean.txt" || true

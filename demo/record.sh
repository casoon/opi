#!/usr/bin/env bash
# Records the README's demo GIFs.
#
#   ./demo/record.sh            every tape
#   ./demo/record.sh hero       one of them
#
# The fixtures are copied out of the repository before recording: opi walks up
# to find a project, so a fixture left inside this repository inherits opi's
# own Cargo.toml and lists cargo tasks that are not its own.

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="${OPI_DEMO_DIR:-/tmp/opi-demo}"

command -v vhs >/dev/null || { echo "vhs is not installed: brew install vhs"; exit 1; }

echo "building opi…"
cargo build --release --manifest-path "$root/Cargo.toml" --quiet

rm -rf "$work"
mkdir -p "$work/bin"
cp -R "$root/demo/fixtures/." "$work/"
# The tapes call a bare `opi`, so the recording shows the tool and not a path.
cp "$root/target/release/opi" "$work/bin/opi"
export PATH="$work/bin:$PATH"

mkdir -p "$root/assets"
cd "$root/demo/tapes"

for tape in "${@:-hero ecosystems health direct}"; do
  for name in $tape; do
    echo "recording $name…"
    vhs "$name.tape"
    # VHS exits 0 and writes nothing when its capture stage fails.
    [ -s "$root/assets/$name.gif" ] || { echo "FAILED: assets/$name.gif was not written"; exit 1; }
  done
done

echo
ls -lh "$root/assets"

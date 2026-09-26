#!/usr/bin/env bash
# Records the README's demo GIFs.
#
#   ./demo/record.sh            every scene
#   ./demo/record.sh health     one of them
#
# Scenes in casts/ are rendered with castwright (animated SVG, real output via
# exec:). Scenes in tapes/ press keys inside the running interface, which
# castwright cannot do yet; they need vhs and are skipped without it.
#
# The fixtures are copied out of the repository before recording: opi walks up
# to find a project, so a fixture left inside this repository inherits opi's
# own Cargo.toml and lists cargo tasks that are not its own.

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="${OPI_DEMO_DIR:-/tmp/opi-demo}"

echo "building opi…"
cargo build --release --manifest-path "$root/Cargo.toml" --quiet

rm -rf "$work"
mkdir -p "$work/bin"
cp -R "$root/demo/fixtures/." "$work/"
# The tapes call a bare `opi`, so the recording shows the tool and not a path.
cp "$root/target/release/opi" "$work/bin/opi"
export PATH="$work/bin:$PATH"
# --updates compares against what is installed; without node_modules npm
# reports no current version and there is nothing to compare.
(cd "$work/pulse" && npm ci --ignore-scripts --no-audit --no-fund --silent)
# The push scene needs a repository with an upstream and one commit not yet
# pushed, so its pre-push hook has something to guard.
(
  cd "$work/pulse"
  git() { command git -c user.name=demo -c user.email=demo@example.com "$@"; }
  printf 'node_modules/\ntarget/\nCargo.lock\n' > .gitignore
  git init -q -b main && git add -A && git commit -qm "Start the dashboard"
  git init -q --bare "$work/pulse.git"
  git remote add origin "$work/pulse.git" && git push -q -u origin main
  printf '# pulse\n\nThe team dashboard.\n' > README.md
  git add README.md && git commit -qm "Describe the dashboard"
)

mkdir -p "$root/assets"

scenes=("$@")
if [ ${#scenes[@]} -eq 0 ]; then
  for f in "$root"/demo/casts/*.terminal.yaml "$root"/demo/tapes/*.tape; do
    name="$(basename "$f")"; name="${name%%.*}"
    [ "$name" = config ] || scenes+=("$name")
  done
fi

for name in "${scenes[@]}"; do
  cast="$root/demo/casts/$name.terminal.yaml"
  if [ -f "$cast" ]; then
    command -v castwright >/dev/null || { echo "castwright is not installed: npm i -g @casoon/castwright node-pty"; exit 1; }
    echo "rendering $name…"
    castwright build "$cast" -o "$root/assets" --format svg --loop-delay 6000 --allow-exec
    out="$root/assets/$name.svg"
  elif command -v vhs >/dev/null; then
    echo "recording $name…"
    (cd "$root/demo/tapes" && vhs "$name.tape")
    # A tape names its own file, and hero.tape's is opi.gif.
    out="$root/assets/$(sed -n 's/^Output "\(.*\)"/\1/p' "$root/demo/tapes/$name.tape" | xargs basename)"
  else
    echo "skipping $name: needs vhs (brew install vhs)"
    continue
  fi
  # VHS exits 0 and writes nothing when its capture stage fails.
  [ -s "$out" ] || { echo "FAILED: $out was not written"; exit 1; }
done

echo
ls -lh "$root/assets"

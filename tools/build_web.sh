#!/usr/bin/env bash
# Build the wasm demos into a static site: <out>/index.html plus <out>/{shmup,asteroids}/.
set -euo pipefail

out=${1:-dist/web}
cd "$(dirname "$0")/.."

cargo build --profile web --target wasm32-unknown-unknown -p shmup -p asteroids

rm -rf "$out"
mkdir -p "$out"
cp web/index.html "$out/"
for game in shmup asteroids; do
  wasm-bindgen --target web --no-typescript --out-dir "$out/$game" --out-name "$game" \
    "target/wasm32-unknown-unknown/web/$game.wasm"
  sed "s/{{GAME}}/$game/g" web/play.html > "$out/$game/index.html"
done

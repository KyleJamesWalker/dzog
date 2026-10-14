#!/usr/bin/env bash
# Build the wasm demos into a static site: <out>/index.html plus <out>/{asteroids,pong}/.
set -euo pipefail

out=${1:-dist/web}
cd "$(dirname "$0")/.."

cargo build --profile web --target wasm32-unknown-unknown -p asteroids -p pong

rm -rf "$out"
mkdir -p "$out"
cp web/index.html "$out/"
for game in asteroids pong; do
  wasm-bindgen --target web --no-typescript --out-dir "$out/$game" --out-name "$game" \
    "target/wasm32-unknown-unknown/web/$game.wasm"
  sed "s/{{GAME}}/$game/g" web/play.html > "$out/$game/index.html"
done

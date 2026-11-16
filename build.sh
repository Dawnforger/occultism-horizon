#!/usr/bin/env bash
# Builds and checks Occultism for Horizon on your own machine, step by step, and stops at the
# first failure with its log. Run from this folder:  ./build.sh
#
#   1. logic tests (no game needed)
#   2. the mod's content, generated from an Occultism checkout into mod/
#   3. the package, mod/ packed as occultism.hmod and checked as the registry checks it
#   4. the native build: the launcher compiles mod/ against its prebuilt game
#
# Built on horizon-rust/horizon main with the mod API (#17145), pinned in mod/horizon-mod.toml's
# [native] (85b5bcca), with that commit's cargo-horizon-mod. Step 4 waits on Horizon: no Nightly
# or launcher from that main exists yet, and native builds need its signed SDK releases.
set -euo pipefail
cd "$(dirname "$0")"
HERE="$PWD"
HORIZON_REV=85b5bcca7c79bb470b90bdb5914b413aa66fe7ae
WORK="$HERE/.work"
LOG="$WORK/build.log"
mkdir -p "$WORK"
: > "$LOG"

step() { printf '\n== %s\n' "$*" | tee -a "$LOG"; }
fail() { printf '\nFAILED: %s\nLast lines of %s:\n' "$*" "$LOG"; tail -n 40 "$LOG"; exit 1; }
need() { command -v "$1" >/dev/null || fail "$1 is not installed"; }

# No system Rust on PATH: use the launcher's own (horizon-launcher rust install).
LAUNCHER="$(command -v horizon-launcher || true)"
if ! command -v cargo >/dev/null && [ -n "$LAUNCHER" ]; then
  RUST="$("$LAUNCHER" rust status --json 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("installed") or "")')"
  [ -n "$RUST" ] && export PATH="$RUST/bin:$PATH"
fi
# Keep cargo-horizon-mod and its downloads in .work, not ~/.cargo.
export CARGO_HOME="${CARGO_HOME:-$WORK/cargo-home}"
export PATH="$CARGO_HOME/bin:$PATH"

need cargo; need git; need python3

step "1/4 Logic tests"
(cd tests/logic && cargo test 2>&1) >>"$LOG" || fail "logic tests"
grep -E "^test result" "$LOG" | head -n 1

step "2/4 Content from Occultism's files"
if [ ! -d "$WORK/occultism/.git" ]; then
  git clone --depth 1 https://github.com/klikli-dev/occultism "$WORK/occultism" >>"$LOG" 2>&1 || fail "cloning Occultism"
fi
python3 -I tools/build_content.py "$WORK/occultism" --out "$HERE/mod" | tee -a "$LOG" || fail "generating content"

step "3/4 Pack and check occultism.hmod"
# Horizon's own tool, at the pinned commit, so the package is checked as that Horizon checks it.
if ! grep -qs "horizon-rust/horizon?rev=$HORIZON_REV" "$CARGO_HOME/.crates.toml" || ! command -v cargo-horizon-mod >/dev/null; then
  cargo install --locked --force --git https://github.com/horizon-rust/horizon --rev "$HORIZON_REV" cargo-horizon-mod >>"$LOG" 2>&1 \
    || fail "installing cargo-horizon-mod"
fi
(cd mod && cargo horizon-mod build) >>"$LOG" 2>&1 || fail "packing the mod"
HMOD="$(ls mod/target/horizon-mod/*.hmod | head -n 1)"
cargo horizon-mod check "$HMOD" >>"$LOG" 2>&1 || fail "the registry's checks on $HMOD"
cp "$HMOD" "$HERE/occultism.hmod"
echo "packed: $HERE/occultism.hmod" | tee -a "$LOG"

step "4/4 Native build (the launcher compiles the mod against its prebuilt game)"
[ -n "$LAUNCHER" ] || fail "horizon-launcher is not on PATH (install the launcher first)"
echo "This compiles native code into your game. You are the one trusting it: say no to stop." | tee -a "$LOG"
read -r -p "Build the native mod now? [y/N] " ok
[ "${ok:-n}" = "y" ] || { echo "Stopped before the native build."; exit 0; }
"$LAUNCHER" native-build "$HERE/mod" --trust-native-code 2>&1 | tee -a "$LOG" || fail "native build"

cat <<EOF

All four steps passed.
Next: play the launcher's instance and walk TESTING.md. Paste $LOG back into the chat if
anything fails.
EOF

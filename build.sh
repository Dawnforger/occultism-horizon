#!/usr/bin/env bash
# Builds and checks the Demonic Altar proof of concept on your own machine, step by step, and
# stops at the first failure with its log. Run from this folder:  ./build.sh
#
#   1. logic tests (no game needed)
#   2. the content mod, generated from an Occultism checkout, packed as occultism.hmod and checked
#   3. the native mod, compiled into a game build by the Horizon launcher
#
# Pinned to Horizon f42ef42 (Nightly 2026-11-03, NATIVE_HOOKS_API 359), the commit the mod is built and tested on.
set -euo pipefail
cd "$(dirname "$0")"
HERE="$PWD"
HORIZON_REV=f42ef42ef6ceccb3c6a20bea2481d87949a51c81
WORK="$HERE/.work"
LOG="$WORK/build.log"
mkdir -p "$WORK"
: > "$LOG"

step() { printf '\n== %s\n' "$*" | tee -a "$LOG"; }
fail() { printf '\nFAILED: %s\nLast lines of %s:\n' "$*" "$LOG"; tail -n 40 "$LOG"; exit 1; }
need() { command -v "$1" >/dev/null || fail "$1 is not installed"; }

# No system Rust or launcher on PATH: use the launcher's own (horizon-launcher rust install).
LAUNCHER="$(command -v horizon-launcher || true)"
[ -n "$LAUNCHER" ] || [ ! -x "$HOME/Games/Horizon/horizon-launcher" ] || LAUNCHER="$HOME/Games/Horizon/horizon-launcher"
if ! command -v cargo >/dev/null && [ -n "$LAUNCHER" ]; then
  RUST="$("$LAUNCHER" rust status --json 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("installed") or "")')"
  [ -n "$RUST" ] && export PATH="$RUST/bin:$PATH"
fi
# Keep cargo-horizon-mod and its downloads in .work, not ~/.cargo.
export CARGO_HOME="${CARGO_HOME:-$WORK/cargo-home}"
export PATH="$CARGO_HOME/bin:$PATH"

need cargo; need git; need python3

step "1/4 Altar logic tests"
(cd tests/logic && cargo test 2>&1) >>"$LOG" || fail "logic tests"
grep -E "^test result" "$LOG" | head -n 1

step "2/4 Content mod from Occultism's files"
if [ ! -d "$WORK/occultism/.git" ]; then
  git clone --depth 1 https://github.com/klikli-dev/occultism "$WORK/occultism" >>"$LOG" 2>&1 || fail "cloning Occultism"
fi
python3 -I tools/build_content.py "$WORK/occultism" --out "$HERE/content" | tee -a "$LOG" || fail "generating content"

step "3/4 Pack and check occultism.hmod"
if ! command -v cargo-horizon-mod >/dev/null; then
  cargo install --locked --git https://github.com/horizon-rust/horizon --rev "$HORIZON_REV" cargo-horizon-mod >>"$LOG" 2>&1 \
    || fail "installing cargo-horizon-mod"
fi
(cd content && cargo horizon-mod build) >>"$LOG" 2>&1 || fail "packing the content mod"
HMOD="$(ls content/target/horizon-mod/*.hmod | head -n 1)"
cargo horizon-mod check "$HMOD" >>"$LOG" 2>&1 || fail "the registry's checks on $HMOD"
cp "$HMOD" "$HERE/occultism.hmod"
echo "packed: $HERE/occultism.hmod" | tee -a "$LOG"

step "4/4 Native build (compiles Horizon with the mod: 10-20 minutes the first time)"
[ -n "$LAUNCHER" ] || fail "horizon-launcher is not on PATH or in ~/Games/Horizon (install the Horizon launcher first)"
echo "This compiles native code into your game. You are the one trusting it: say no to stop." | tee -a "$LOG"
read -r -p "Build the native mod now? [y/N] " ok
[ "${ok:-n}" = "y" ] || { echo "Stopped before the native build."; exit 0; }
"$LAUNCHER" native-build "$HERE/native" --trust-native-code 2>&1 | tee -a "$LOG" || fail "native build (the launcher's own log: native/build.log in its data folder)"

cat <<EOF

All four steps passed.
Next: install $HERE/occultism.hmod from the launcher's Mods page (Install from file) into the
"Native mods" instance, play it, and walk TESTING.md. Paste $LOG back into the chat if anything fails.
EOF

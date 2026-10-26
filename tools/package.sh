#!/usr/bin/env bash
# Build dyna-island.exe (cross, mingw), a portable zip and the NSIS installer into dist/.
#
#   tools/package.sh                 # makensis must be on PATH
#   NSIS_HOST=user@host tools/package.sh   # build the installer on a Linux box with nsis
#
# Optional: SELFTEST_HOST=user@host also runs `dyna-island.exe --selftest` under Wine + Xvfb
# there and copies the screenshots back to dist/selftest/.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed -E 's/.*"(.*)".*/\1/')"
export PATH="$HOME/.cargo/bin:$PATH"

cd "$ROOT"
cargo test -p dyna-core --quiet
cargo build -p dyna-island --release --target x86_64-pc-windows-gnu
EXE="$ROOT/target/x86_64-pc-windows-gnu/release/dyna-island.exe"

mkdir -p "$ROOT/dist"
cp "$EXE" "$ROOT/dist/dyna-island.exe"
# Portable build inside its own folder, so dyna-island.exe never sits loose next to
# other downloads (Windows looks for some DLLs in the exe's folder first).
( cd "$ROOT/dist" && rm -rf DynaIsland "Dyna-Island-$VERSION-portable.zip" && mkdir DynaIsland && cp dyna-island.exe "$ROOT/THIRD-PARTY-NOTICES.txt" DynaIsland/ && zip -qr "Dyna-Island-$VERSION-portable.zip" DynaIsland && rm -rf DynaIsland )

if [[ -n "${NSIS_HOST:-}" ]]; then
  REMOTE_DIR="dyna-pkg"
  ssh -o BatchMode=yes "$NSIS_HOST" "rm -rf ~/$REMOTE_DIR && mkdir -p ~/$REMOTE_DIR/installer/art ~/$REMOTE_DIR/crates/dyna-island/res ~/$REMOTE_DIR/dist"
  scp -q -o BatchMode=yes "$ROOT/installer/dyna-island.nsi" "$NSIS_HOST:$REMOTE_DIR/installer/"
  scp -q -o BatchMode=yes "$ROOT/installer/art/nsis-welcome.bmp" "$ROOT/installer/art/nsis-header.bmp" "$NSIS_HOST:$REMOTE_DIR/installer/art/"
  scp -q -o BatchMode=yes "$ROOT/crates/dyna-island/res/dyna-island.ico" "$NSIS_HOST:$REMOTE_DIR/crates/dyna-island/res/"
  scp -q -o BatchMode=yes "$ROOT/THIRD-PARTY-NOTICES.txt" "$NSIS_HOST:$REMOTE_DIR/"
  scp -q -o BatchMode=yes "$EXE" "$NSIS_HOST:$REMOTE_DIR/dyna-island.exe"
  ssh -o BatchMode=yes "$NSIS_HOST" "cd ~/$REMOTE_DIR && makensis -V2 -DEXE=\$HOME/$REMOTE_DIR/dyna-island.exe -DOUTDIR=\$HOME/$REMOTE_DIR/dist installer/dyna-island.nsi"
  scp -q -o BatchMode=yes "$NSIS_HOST:$REMOTE_DIR/dist/Dyna-Island-Setup-$VERSION.exe" "$ROOT/dist/"
elif command -v makensis >/dev/null; then
  makensis -V2 -DEXE="$EXE" -DOUTDIR="$ROOT/dist" "$ROOT/installer/dyna-island.nsi"
else
  echo "makensis not found and NSIS_HOST not set: skipping the installer" >&2
fi

if [[ -n "${SELFTEST_HOST:-}" ]]; then
  ssh -o BatchMode=yes "$SELFTEST_HOST" "mkdir -p ~/dyna-test"
  scp -q -o BatchMode=yes "$EXE" "$SELFTEST_HOST:dyna-test/dyna-island.exe"
  ssh -o BatchMode=yes "$SELFTEST_HOST" 'cd ~/dyna-test && rm -rf st-out && export WINEPREFIX=$HOME/dyna-test/wp WINEDEBUG=-all && timeout 180 xvfb-run -a -s "-screen 0 1920x1080x24" wine ~/dyna-test/dyna-island.exe --selftest st-out; echo "selftest exit=$?"'
  rm -rf "$ROOT/dist/selftest" && mkdir -p "$ROOT/dist/selftest"
  scp -q -r -o BatchMode=yes "$SELFTEST_HOST:dyna-test/st-out/*" "$ROOT/dist/selftest/" || true
fi

ls -la "$ROOT/dist"

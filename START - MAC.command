#!/bin/bash
# De Story (Tauri v2 + Rust) - klik 2x file ini untuk membuka aplikasi di macOS.
# Modified by ikram.mf7 - Based on Bang Story by Bang Tutorial
cd "$(dirname "$0")" || exit 1

export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$PATH"
[ -s "$HOME/.nvm/nvm.sh" ] && . "$HOME/.nvm/nvm.sh"

npm run tauri:dev
exit 0

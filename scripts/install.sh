#!/usr/bin/env bash
# Build typer and install it for the current user:
#   ~/.local/bin/{typerd,typerctl,typer}, a systemd --user service and a .desktop entry.
# Waybar and Hyprland configs are never touched; the snippets are printed instead.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$HOME/.local/bin"
units="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

if ! id -nG | grep -qw input; then
  echo "warning: $USER is not in the 'input' group; typerd cannot read keyboards." >&2
  echo "         sudo usermod -aG input $USER  (then log out and back in)" >&2
fi

cd "$root"
cargo build --release -p typerd -p typerctl
(cd apps/web && bun run desktop:build -- --no-bundle)

mkdir -p "$apps"
install -Dm755 target/release/typerd target/release/typerctl target/release/typer -t "$bin"
install -Dm644 dist/typerd.service "$units/typerd.service"
# Launcher entry (Vicinae, rofi, ...). Absolute paths: launchers may not have ~/.local/bin on PATH.
sed -e "s|^Exec=typer\(ctl\)\?|Exec=$bin/typer\1|" dist/typer.desktop >"$apps/typer.desktop"

systemctl --user daemon-reload
systemctl --user enable typerd.service
systemctl --user restart typerd.service

cat <<EOF

typer installed. Daemon status: $(systemctl --user is-active typerd.service)

Packs: drop Mechvibes packs (folders with config.json) into
  ${XDG_DATA_HOME:-$HOME/.local/share}/typer/packs/  then run: typerctl reload

The tray icon appears in Waybar's tray (right-click opens the menu).
Hyprland rules -> $root/dist/hyprland.lua
Optional Waybar text module -> $root/dist/waybar-module.jsonc
EOF

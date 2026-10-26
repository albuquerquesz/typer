# typer

Mechanical keyboard sounds for Hyprland. Global capture via evdev, Mechvibes-compatible sound packs, and a Waybar module with a small control popup.

```
crates/typer-core   shared config, Mechvibes manifest parsing, IPC protocol
crates/typerd       daemon: evdev → realtime mixer (cpal); ~10 MB RSS
crates/typerctl     CLI for Waybar, binds and scripts
apps/web            Tauri + React menu (layer-shell, opens on demand, exits on blur)
dist/               systemd unit, .desktop entry, Waybar module, Hyprland (Lua) shortcut
```

Scaffolded with [kubojs](https://github.com/albuquerquesz/kubo).

## Install

Requirements: Rust, Bun, WebKitGTK 4.1, ALSA/PipeWire, and your user in the `input` group.

```sh
bun install
bun run install:local   # builds, copies binaries to ~/.local/bin, enables typerd.service
```

typer shows up in Waybar's `tray` module on its own (no Waybar config needed). Right-click opens a small menu right below the icon (a layer-shell surface, so no window rules). `dist/hyprland.lua` adds the `Super+Shift+T` on/off shortcut and turns off Hyprland's layer animation for the menu (it animates itself).

## Use

| Action                | Tray icon    | CLI                                    |
| --------------------- | ------------ | -------------------------------------- |
| Open / close the menu | right-click  | `typerctl popup`                       |
| On / off              | left-click   | `typerctl toggle`, `Super+Shift+T`     |
| Preview the sound     | middle-click | `typerctl preview`                     |
| Volume                | scroll       | `typerctl volume 40` / `+5` / `-5`     |
| Switch pack           | menu         | `typerctl packs`, `typerctl pack <id>` |

Prefer a text module with the volume instead of an icon? `dist/waybar-module.jsonc` is an optional custom module with the same actions.

## Sound packs

Two synthesized packs are built in. To add a [Mechvibes](https://mechvibes.com) pack, copy its folder (the one with `config.json`) into `~/.local/share/typer/packs/` and run `typerctl reload`. Both `single` (sprite) and `multi` (file per key) packs are supported, with per-key sounds for Enter, Space, Backspace and so on.

## Develop

```sh
bun run dev:daemon   # run typerd in the foreground
bun run dev:popup    # tauri dev for the popup
bun run test         # cargo tests
bun run lint:rust && bun run check
```

`TYPER_SOCKET`, `XDG_CONFIG_HOME` and `XDG_DATA_HOME` let you run an isolated daemon next to the installed one.

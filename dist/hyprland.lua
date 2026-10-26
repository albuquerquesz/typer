-- typer: on/off shortcut. The menu is a layer-shell surface and positions
-- itself under the tray icon, so it needs no window rules.
-- Paste into ~/.config/hypr/UserConfigs/keybinds.lua (or require this file).

hl.bind("SUPER + SHIFT + T", hl.dsp.exec_cmd("typerctl toggle"))

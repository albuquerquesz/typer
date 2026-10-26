-- typer: on/off shortcut. The menu is a layer-shell surface and positions
-- itself under the tray icon, so it needs no window rules.
-- Paste into ~/.config/hypr/UserConfigs/ (or require this file).

-- The menu animates itself; skip the compositor's layer animation on top of it.
hl.layer_rule({ match = { namespace = "typer" }, no_anim = true })

hl.bind("SUPER + SHIFT + T", hl.dsp.exec_cmd("typerctl toggle"))

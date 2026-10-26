-- typer: control popup pinned under the top bar, plus an on/off shortcut.
-- Paste into ~/.config/hypr/UserConfigs/window-rules.lua and keybinds.lua (or require this file).

hl.window_rule({
    match = { class = "^(typer)$" },
    float = true,
    pin = true,
    size = { "380", "300" },
    rounding = 12,
    move = { "monitor_w - 392", "44" },
})

hl.bind("SUPER + SHIFT + T", hl.dsp.exec_cmd("typerctl toggle"))

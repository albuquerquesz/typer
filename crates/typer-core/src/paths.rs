use std::env;
use std::path::PathBuf;

fn home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(fallback))
}

/// `$TYPER_SOCKET`, or `$XDG_RUNTIME_DIR/typer.sock`
pub fn socket() -> PathBuf {
    if let Some(path) = env::var_os("TYPER_SOCKET").filter(|v| !v.is_empty()) {
        return PathBuf::from(path);
    }
    let runtime = env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    runtime.join("typer.sock")
}

/// `~/.config/typer/config.toml`
pub fn config_file() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("typer").join("config.toml")
}

/// `~/.local/share/typer/packs`: user-installed Mechvibes packs, one directory each.
pub fn packs_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("typer").join("packs")
}

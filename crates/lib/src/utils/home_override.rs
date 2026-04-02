use std::path::PathBuf;
use std::sync::Mutex;

static HOME_OVERRIDE: std::sync::LazyLock<Mutex<Option<PathBuf>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Return the home directory, allowing tests to override it.
pub fn home_dir() -> Option<PathBuf> {
    let override_guard = HOME_OVERRIDE.lock().expect("home override mutex poisoned");
    override_guard.clone().or_else(dirs::home_dir)
}

/// Override the home directory for the scope of a test.
pub fn set_override(path: PathBuf) {
    let mut override_guard = HOME_OVERRIDE.lock().expect("home override mutex poisoned");
    *override_guard = Some(path);
}

/// Clear any previously set override.
pub fn clear_override() {
    let mut override_guard = HOME_OVERRIDE.lock().expect("home override mutex poisoned");
    *override_guard = None;
}

use std::path::PathBuf;
use std::sync::Mutex;

/// Lazy static storing an optional overridden home directory for tests.
///
/// This is private and used only by tests to control what `home_dir()` returns.
static HOME_OVERRIDE: std::sync::LazyLock<Mutex<Option<PathBuf>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Return the home directory, allowing tests to override it.
///
/// This function never panics: if the internal mutex has been poisoned it will
/// recover the inner value and continue using it.
pub fn home_dir() -> Option<PathBuf> {
    // Recover from a poisoned mutex instead of panicking.
    let override_guard = match HOME_OVERRIDE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    override_guard.clone().or_else(dirs::home_dir)
}

/// Override the home directory for the scope of a test.
///
/// This will set an in-memory override; if the mutex is poisoned it will be
/// recovered rather than causing a panic.
pub fn set_override(path: PathBuf) {
    let mut override_guard = match HOME_OVERRIDE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *override_guard = Some(path);
}

/// Clear any previously set override.
///
/// Recover from a poisoned mutex rather than panicking.
pub fn clear_override() {
    let mut override_guard = match HOME_OVERRIDE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *override_guard = None;
}

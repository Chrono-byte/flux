use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread::ThreadId;

static HOME_OVERRIDE: std::sync::LazyLock<Mutex<HashMap<ThreadId, Vec<PathBuf>>>> =
  std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Return the home directory, allowing tests to override it.
///
/// This function never panics: if the internal mutex has been poisoned it will
/// recover the inner value and continue using it.
pub fn home_dir() -> Option<PathBuf> {
  let override_guard = match HOME_OVERRIDE.lock() {
    Ok(guard) => guard,
    Err(poisoned) => poisoned.into_inner(),
  };
  override_guard
    .get(&std::thread::current().id())
    .and_then(|stack| stack.last().cloned())
    .or_else(dirs::home_dir)
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
  let stack = override_guard
    .entry(std::thread::current().id())
    .or_insert_with(Vec::new);
  stack.push(path);
}

/// Clear any previously set override.
///
/// Recover from a poisoned mutex rather than panicking.
pub fn clear_override() {
  let mut override_guard = match HOME_OVERRIDE.lock() {
    Ok(guard) => guard,
    Err(poisoned) => poisoned.into_inner(),
  };
  if let Some(stack) = override_guard.get_mut(&std::thread::current().id()) {
    let _ = stack.pop();
    if stack.is_empty() {
      let _ = override_guard.remove(&std::thread::current().id());
    }
  }
}

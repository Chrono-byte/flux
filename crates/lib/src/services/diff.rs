use std::path::Path;

use std::fmt::Write;

/// Produce a simple unified-diff style representation between two text snapshots.
///
/// This intentionally favors determinism and low dependencies over perfect diff quality.
/// For now we emit a single hunk that shows the whole file when there are changes.
///
/// # Errors
///
/// This function currently cannot fail but returns a Result for consistency.
#[must_use]
pub fn unified_diff(
  path: &Path,
  old: &str,
  new: &str,
) -> String {
  // Fast-path: no changes
  if old == new {
    return String::new();
  }

  let old_lines: Vec<&str> = if old.is_empty() {
    Vec::new()
  } else {
    old.lines().collect()
  };
  let new_lines: Vec<&str> = if new.is_empty() {
    Vec::new()
  } else {
    new.lines().collect()
  };

  let mut out = String::new();

  // Header similar to `diff -u`
  let _ = writeln!(out, "--- {}", path.display());
  let _ = writeln!(out, "+++ {}", path.display());
  let _ = writeln!(out, "@@ -1,{} +1,{} @@", old_lines.len(), new_lines.len());

  for line in old_lines {
    out.push('-');
    out.push_str(line);
    out.push('\n');
  }

  for line in new_lines {
    out.push('+');
    out.push_str(line);
    out.push('\n');
  }

  out
}

use crate::utils::error::Result;
use std::path::Path;

/// Produce a simple unified-diff style representation between two text snapshots.
///
/// This intentionally favors determinism and low dependencies over perfect diff quality.
/// For now we emit a single hunk that shows the whole file when there are changes.
pub fn unified_diff(path: &Path, old: &str, new: &str) -> Result<String> {
    // Fast-path: no changes
    if old == new {
        return Ok(String::new());
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
    out.push_str(&format!("--- {}\n", path.display()));
    out.push_str(&format!("+++ {}\n", path.display()));
    out.push_str(&format!(
        "@@ -1,{} +1,{} @@\n",
        old_lines.len(),
        new_lines.len()
    ));

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

    Ok(out)
}

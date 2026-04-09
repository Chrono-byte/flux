//! Render functions for the CLI.
//!
//! This module provides functions for rendering output to the console during CLI operations.
//!
//! Implemented post migration from lib => cli for rendering.

use colored::Colorize;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;

use flux_lib::commands::apply::StateDiff;
use flux_lib::commands::apply::{ApplyReport, ApplyStatus};
use flux_lib::commands::doctor::HealthReport;
use flux_lib::commands::migrate::MigrateReport;
use flux_lib::commands::status::StatusReport;
use flux_lib::commands::untracked::{Discrepancy, IssueType};
use flux_lib::commands::validate::{ValidationIssue, ValidationReport};
use flux_lib::file_manager::SyncReport;
use flux_lib::services::git::{
  CommitReport, GitStatusReport, PullReport, PushReport, RemoteListReport,
};
use flux_lib::types::FileChange;
use flux_lib::utils::dry_run::{DryRun, Operation};

/// Display a preview of changes that would be applied
pub(super) fn display_preview(diff: &StateDiff) {
  println!("\n{} Preview of changes:", "→".cyan().bold());

  if diff.files_to_sync.is_empty() {
    println!(
      "  {} System is already in sync with configuration",
      "✓".green()
    );
    return;
  }

  if !diff.files_to_sync.is_empty() {
    println!(
      "\n  {} Files to sync ({}):",
      "📁".cyan(),
      diff.files_to_sync.len()
    );
    for file in &diff.files_to_sync {
      println!("    • {}", file.dest_path.display());
    }
  }

  println!("\n  {} Total changes: {}", "→".cyan(), diff.total_changes());
}

/// Display a summary of the dry run operations.
pub(super) fn display_dry_run_summary(dry_run: &DryRun) {
  if dry_run.operations().is_empty() {
    println!("{}", "No operations to perform.".yellow());
    return;
  }

  println!(
    "\n{}",
    "DRY RUN - Operations that would be performed:"
      .bold()
      .cyan()
  );
  println!("{}", "=".repeat(60).cyan());

  for (i, op) in dry_run.operations().iter().enumerate() {
    println!("\n{}. {}", i + 1, format!("{op:?}").bright_white());
    match op {
      Operation::CreateSymlink { from, to } => {
        println!(
          "   {} {} -> {}",
          "Create symlink:".green(),
          from.display(),
          to.display()
        );
      }
      Operation::CreateDirectory { path } => {
        println!("   {} {}", "Create directory:".blue(), path.display());
      }
      Operation::CopyFile { from, to } => {
        println!(
          "   {} {} -> {}",
          "Copy file:".cyan(),
          from.display(),
          to.display()
        );
      }
      Operation::RemoveFile { path } => {
        println!("   {} {}", "Remove file:".red(), path.display());
      }
      Operation::GitCommit { message } => {
        println!("   {} {}", "Git commit:".magenta(), message);
      }
      Operation::GitStage { files } => {
        println!("   {} {} file(s)", "Git stage:".magenta(), files.len());
        for file in files {
          println!("      - {}", file.display());
        }
      }
      Operation::GitRemoteAdd { name, url } => {
        println!(
          "   {} Add remote '{}': {}",
          "Git remote:".bright_magenta(),
          name.cyan(),
          url
        );
      }
      Operation::GitRemoteRemove { name } => {
        println!(
          "   {} Remove remote '{}'",
          "Git remote:".bright_magenta(),
          name.cyan()
        );
      }
      Operation::GitRemoteSetUrl { name, url } => {
        println!(
          "   {} Set URL for remote '{}': {}",
          "Git remote:".bright_magenta(),
          name.cyan(),
          url
        );
      }
      Operation::GitPush {
        remote,
        branch,
        set_upstream,
      } => {
        println!(
          "   {} Push branch '{}' to remote '{}' (set_upstream: {})",
          "Git push:".bright_magenta(),
          branch.cyan(),
          remote.cyan(),
          if *set_upstream {
            "yes".green()
          } else {
            "no".yellow()
          }
        );
      }
      Operation::GitPull { remote, branch } => {
        println!(
          "   {} Pull branch '{}' from remote '{}'",
          "Git pull:".bright_magenta(),
          branch.cyan(),
          remote.cyan()
        );
      }
    }
  }

  println!("\n{}", "=".repeat(60).cyan());
  println!(
    "{}",
    format!("Total operations: {}", dry_run.operations().len()).bold()
  );
}

/// Display the result of an apply command.
pub(super) fn display_apply_report(report: &ApplyReport) {
  match report.status {
    ApplyStatus::NoChanges => {
      println!(
        "{}",
        "✓ System is already in sync with configuration".green()
      );
    }
    ApplyStatus::Cancelled => {
      println!("{}", "⊘ Apply cancelled".yellow());
    }
    ApplyStatus::DryRun => {
      println!(
        "{}",
        "⚠ DRY RUN MODE - No changes were applied"
          .yellow()
          .bold()
      );
    }
    ApplyStatus::Applied => {
      println!("{}", "✓ Configuration applied successfully".green());
      if let Some(transaction_id) = &report.transaction_id {
        println!("  Transaction ID: {transaction_id}");
      }
    }
  }
}

/// Display the result of a migrate command.
pub(super) fn display_migrate_report(report: &MigrateReport) {
  if report.discrepancies == 0 {
    println!(
      "{}",
      "✓ No discrepancies found - all files are correctly configured.".green()
    );
    return;
  }

  println!(
    "{} Found {} discrepancy(ies) to migrate",
    "→".cyan(),
    report.discrepancies
  );
  println!(
    "{} migrated, {} skipped",
    report.migrated.to_string().green(),
    report.skipped.to_string().yellow()
  );
}

/// Display the result of a sync operation.
pub(super) fn display_sync_report(report: &SyncReport) {
  if report.synced == 0 && report.skipped == 0 {
    println!("{}", "⊘ No files to sync".yellow());
    return;
  }

  println!(
    "{} Sync complete: {} synced, {} skipped",
    "✓".green(),
    report.synced,
    report.skipped
  );
}

/// Display the result of a commit operation.
pub(super) fn display_commit_report(report: &CommitReport) {
  if let Some(branch_name) = &report.branch_name {
    println!(
      "[{} {}] {}",
      branch_name.cyan(),
      report.short_id.cyan(),
      report.message
    );
  } else {
    println!("[{}] {}", report.short_id.cyan(), report.message);
  }
}

/// Display a list of remotes.
pub(super) fn display_remote_list(report: &RemoteListReport) {
  if report.remotes.is_empty() {
    println!("{}", "⊘ No remotes configured.".yellow());
    return;
  }

  println!();
  for remote in &report.remotes {
    println!("{}  {} (fetch)", remote.name.cyan(), remote.fetch_url);
    println!("{}  {} (push)", remote.name.cyan(), remote.push_url);
  }
  println!();
}

/// Display the result of a push operation.
pub(super) fn display_push_report(report: &PushReport) {
  if report.up_to_date {
    println!("Everything up-to-date");
    return;
  }

  println!("To {}", report.remote_url);
  match (&report.head_before, &report.head_after) {
    (Some(before), Some(after)) if before == after => {
      println!(
        "   {after}  {} -> {} (new branch)",
        report.branch, report.branch
      );
    }
    (Some(before), Some(after)) => {
      println!(
        "   {before}..{after}  {} -> {}",
        report.branch, report.branch
      );
    }
    (_, Some(after)) => {
      println!(
        "   {after}  {} -> {} (new branch)",
        report.branch, report.branch
      );
    }
    _ => {}
  }

  if report.set_upstream {
    println!(
      "  {} Set upstream to {}/{}",
      "✓".cyan(),
      report.remote.cyan(),
      report.branch.cyan()
    );
  }
}

/// Display the result of a pull operation.
pub(super) fn display_pull_report(report: &PullReport) {
  if report.up_to_date {
    println!("Already up-to-date.");
    return;
  }

  if let (Some(before), Some(after)) = (&report.head_before, &report.head_after) {
    println!("Updating {before}..{after}");
  }

  if report.was_fast_forward {
    println!("Fast-forward");
  } else {
    println!("Merge made by the 'recursive' strategy.");
  }

  if report.files_changed > 0 {
    let mut stats = format!(
      " {} file{} changed",
      report.files_changed,
      if report.files_changed == 1 { "" } else { "s" }
    );
    if report.insertions > 0 || report.deletions > 0 {
      let mut parts = Vec::new();
      if report.insertions > 0 {
        parts.push(format!(
          "{} insertion{}",
          report.insertions,
          if report.insertions == 1 { "" } else { "s" }
        ));
      }
      if report.deletions > 0 {
        parts.push(format!(
          "{} deletion{}",
          report.deletions,
          if report.deletions == 1 { "" } else { "s" }
        ));
      }
      let _ = write!(stats, ", {}", parts.join(", "));
    }
    println!("{stats}");
  }
}

/// Display the result of a git status operation.
pub(super) fn display_git_status(report: &GitStatusReport) {
  let Some(branch_name) = &report.branch_name else {
    println!("{}", "⊘ Not on any branch".yellow());
    return;
  };

  println!("\n{}", "Git Repository Status:".bold().cyan());
  println!("{}", "=".repeat(60).cyan());
  println!("  Branch: {}", branch_name.cyan());

  if let Some(upstream_name) = &report.upstream_name {
    println!("  Upstream: {}", upstream_name.dimmed());
  }

  if let (Some(short_id), Some(head_message)) = (&report.short_id, &report.head_message) {
    println!("  HEAD: {} {}", short_id.cyan(), head_message);
  }

  if let (Some(ahead), Some(behind)) = (report.ahead, report.behind)
    && (ahead > 0 || behind > 0)
  {
    let mut status_parts = Vec::new();
    if ahead > 0 {
      status_parts.push(
        format!("{ahead} commit(s) ahead")
          .green()
          .to_string(),
      );
    }
    if behind > 0 {
      status_parts.push(
        format!("{behind} commit(s) behind")
          .yellow()
          .to_string(),
      );
    }
    if !status_parts.is_empty() {
      println!("  {}", status_parts.join(", "));
    }
  }

  if report.changes.is_empty() {
    println!("\n  {} Working tree clean", "✓".green());
  } else {
    println!("\n  {} uncommitted change(s):", "→".yellow());
    for change in &report.changes {
      let (icon, path) = match change {
        FileChange::Added(path) => ("+", path),
        FileChange::Modified(path) => ("M", path),
        FileChange::Deleted(path) => ("-", path),
      };
      println!(
        "    {} {}",
        icon.green(),
        path.display().to_string().dimmed()
      );
    }
  }

  println!();
}

/// Format the diffs for display.
pub(super) fn format_diffs(diff: &StateDiff) -> String {
  if diff.files_to_sync.is_empty() {
    return String::new();
  }

  let mut out = String::new();
  let _ = writeln!(
    out,
    "\n{}",
    "Unified diffs for planned changes:".bold().cyan()
  );
  let _ = writeln!(out, "{}", "=".repeat(60).cyan());

  for file in &diff.files_to_sync {
    let current = fs::read_to_string(&file.dest_path).unwrap_or_default();

    let Ok(target) = fs::read_to_string(&file.repo_path) else {
      let _ = writeln!(
        out,
        "\n{} {} (skipping diff: cannot read repo file)",
        "⊘".yellow(),
        file.repo_path.display()
      );
      continue;
    };

    let udiff = flux_lib::services::unified_diff(&file.dest_path, &current, &target);
    if udiff.trim().is_empty() {
      continue;
    }

    let _ = writeln!(out, "\n{} {}", "File:".bold(), file.dest_path.display());
    let _ = writeln!(out, "{}", "-".repeat(60).cyan());
    let _ = writeln!(out, "{udiff}");
  }

  out
}

/// Display the diffs for the given state diff.
pub(super) fn display_diffs(diff: &StateDiff) {
  let output = format_diffs(diff);
  if output.is_empty() {
    return;
  }
  print!("{output}");
}

/// Display the validation report for the given validation report.
pub(super) fn display_validation(report: &ValidationReport) {
  if report.is_valid {
    println!("{}", "✓ Configuration is valid!".green().bold());
    return;
  }

  println!("\n{}", "Validation Issues:".bold().red());
  println!("{}", "=".repeat(60).red());

  for issue in &report.issues {
    match issue {
      ValidationIssue::MissingRepoFile(file) => {
        println!(
          "{} Missing repo file: {}",
          "✗".red(),
          file.repo_path.display()
        );
      }
      ValidationIssue::InvalidSymlink(file) => {
        println!(
          "{} Invalid symlink: {}",
          "⚠".yellow(),
          file.dest_path.display()
        );
      }
      ValidationIssue::OrphanedEntry(tool, file) => {
        println!("{} Orphaned file in {}: {}", "⊘".yellow(), tool, file);
      }
      ValidationIssue::MissingProfileDir(profile) => {
        println!("{} Missing profile directory: {}", "⚠".yellow(), profile);
      }
      ValidationIssue::InvalidConfig(msg) => {
        println!("{} {}", "✗".red(), msg);
      }
    }
  }

  println!("{}", "=".repeat(60).red());
  println!(
    "{} Found {} issue(s)",
    "Summary:".bold(),
    report.issues.len().to_string().red()
  );
}

/// Display the status reports for the given status reports.
pub(super) fn display_status(reports: &[StatusReport]) {
  if reports.is_empty() {
    println!("{}", "No tracked files found.".yellow());
    return;
  }

  let mut synced: Vec<&StatusReport> = Vec::new();
  let mut missing_symlinks: Vec<&StatusReport> = Vec::new();
  let mut broken_symlinks: Vec<&StatusReport> = Vec::new();
  let mut out_of_sync: Vec<&StatusReport> = Vec::new();
  let mut missing_repo: Vec<&StatusReport> = Vec::new();

  for report in reports {
    match report.status {
      flux_lib::commands::status::FileStatus::Synced => synced.push(report),
      flux_lib::commands::status::FileStatus::MissingSymlink => missing_symlinks.push(report),
      flux_lib::commands::status::FileStatus::BrokenSymlink => broken_symlinks.push(report),
      flux_lib::commands::status::FileStatus::OutOfSync => out_of_sync.push(report),
      flux_lib::commands::status::FileStatus::MissingRepo => missing_repo.push(report),
    }
  }

  synced.sort_by_key(|report| &report.file.dest_path);
  missing_symlinks.sort_by_key(|report| &report.file.dest_path);
  broken_symlinks.sort_by_key(|report| &report.file.dest_path);
  out_of_sync.sort_by_key(|report| &report.file.dest_path);
  missing_repo.sort_by_key(|report| &report.file.repo_path);

  let has_issues = !missing_symlinks.is_empty()
    || !broken_symlinks.is_empty()
    || !out_of_sync.is_empty()
    || !missing_repo.is_empty();

  if has_issues {
    if !missing_symlinks.is_empty() {
      println!("\n{}", "Missing symlinks:".yellow().bold());
      for report in &missing_symlinks {
        println!("  {}", format_file_path(&report.file.dest_path).yellow());
      }
    }

    if !broken_symlinks.is_empty() {
      println!("\n{}", "Broken symlinks:".yellow().bold());
      for report in &broken_symlinks {
        println!("  {}", format_file_path(&report.file.dest_path).yellow());
      }
    }

    if !out_of_sync.is_empty() {
      println!("\n{}", "Out of sync:".yellow().bold());
      for report in &out_of_sync {
        println!("  {}", format_file_path(&report.file.dest_path).yellow());
      }
    }

    if !missing_repo.is_empty() {
      println!("\n{}", "Missing repo files:".red().bold());
      for report in &missing_repo {
        println!("  {}", format_file_path(&report.file.repo_path).red());
      }
    }
  }

  let total = reports.len();
  let synced_count = synced.len();
  let issues_count = total - synced_count;

  if issues_count == 0 {
    println!("\n{}", "nothing to sync, working tree clean".green());
  } else {
    println!(
      "\n{} {} file(s) synced, {} file(s) need attention",
      "Summary:".bold(),
      synced_count.to_string().green(),
      issues_count.to_string().yellow()
    );
  }
}

/// Display the health report for the given health report.
pub(super) fn display_health_report(report: &HealthReport) {
  if report.is_healthy() {
    println!("{} All health checks passed!", "✓".green().bold());
    return;
  }

  println!("\n{} Health Check Report", "→".cyan().bold());
  println!("{}", "=".repeat(60).cyan());

  let by_category = report.by_category();
  let categories: Vec<_> = by_category.keys().collect();
  let mut sorted_categories = categories.clone();
  sorted_categories.sort();

  let empty_vec = Vec::new();
  for category in sorted_categories {
    let cat_issues = by_category.get(category).unwrap_or(&empty_vec);
    println!(
      "\n{} {} ({} issue(s))",
      "Category:".bold(),
      category.cyan(),
      cat_issues.len()
    );

    for issue in cat_issues {
      match issue {
        flux_lib::commands::doctor::HealthIssue::BrokenSymlink {
          path,
          target,
          reason,
        } => {
          println!("  {} Broken symlink: {}", "⚠".yellow(), path.display());
          println!("    Target: {}", target.display());
          println!("    Reason: {reason}");
          println!("    💡 Fix: Run `flux apply` to recreate symlink");
        }
        flux_lib::commands::doctor::HealthIssue::MissingRepoFile { file } => {
          println!(
            "  {} Missing repo file: {}",
            "✗".red(),
            file.repo_path.display()
          );
          println!("    💡 Fix: Add file to repository or remove from config");
        }
        flux_lib::commands::doctor::HealthIssue::ConfigDrift { file, summary } => {
          println!(
            "  {} Config drift: {}",
            "↻".yellow(),
            file.dest_path.display()
          );
          println!("    {summary}");
          println!("    💡 Fix: Run `flux apply` to sync with repository");
        }
        flux_lib::commands::doctor::HealthIssue::InvalidProfile { profile, reason } => {
          println!("  {} Invalid profile: {}", "⚠".yellow(), profile);
          println!("    Reason: {reason}");
          println!("    💡 Fix: Create profile directory or switch to valid profile");
        }
        flux_lib::commands::doctor::HealthIssue::BadGitRemote {
          remote,
          url,
          reason,
        } => {
          println!("  {} Bad git remote: {} ({})", "⚠".yellow(), remote, url);
          println!("    Reason: {reason}");
          println!("    💡 Fix: Update remote URL with `flux remote set-url {remote} <new-url>`");
        }
        flux_lib::commands::doctor::HealthIssue::UnsafePermissions {
          path,
          current_mode,
          recommended_mode,
        } => {
          println!("  {} Unsafe permissions: {}", "⚠".yellow(), path.display());
          println!("    Current: {current_mode}, Recommended: {recommended_mode}");
          println!(
            "    💡 Fix: Run `chmod {} {}`",
            recommended_mode,
            path.display()
          );
        }
      }
    }
  }

  println!("\n{}", "=".repeat(60).cyan());
  println!(
    "{} Found {} issue(s) across {} categories",
    "Summary:".bold(),
    report.issues.len().to_string().red(),
    by_category.len()
  );
}

/// Display the report for the given validation: state diff, and health report.
pub(super) fn display_doctor_report(
  report: &ValidationReport,
  diff: &StateDiff,
  health_report: &HealthReport,
) {
  println!("\n{} Doctor Report", "→".cyan().bold());
  println!("{}", "=".repeat(60).cyan());

  if report.is_valid && diff.is_empty() {
    println!(
      "{} Configuration is valid and system is in sync",
      "✓".green()
    );
  } else {
    if !report.is_valid {
      display_validation(report);
    }

    if !diff.is_empty() {
      println!(
        "\n{} System is not in sync with configuration",
        "⚠".yellow()
      );
      display_preview(diff);
    }
  }

  display_health_report(health_report);
}

/// Display the given discrepancies.
pub(super) fn display_discrepancies(discrepancies: &[Discrepancy]) {
  if discrepancies.is_empty() {
    println!(
      "{}",
      "✓ All tracked files are correctly configured.".green()
    );
    return;
  }

  println!("\n{}", "Discrepancies Found:".bold().red());
  println!("{}", "=".repeat(80).red());

  let mut by_tool: HashMap<String, Vec<&Discrepancy>> = HashMap::new();
  for discrepancy in discrepancies {
    by_tool
      .entry(discrepancy.file.tool.clone())
      .or_default()
      .push(discrepancy);
  }

  let mut tool_names: Vec<_> = by_tool.keys().collect();
  tool_names.sort();

  let empty_vec = Vec::new();
  for tool in tool_names {
    let tool_discrepancies = by_tool.get(tool).unwrap_or(&empty_vec);
    println!(
      "\n{} {} ({} issue(s))",
      "Tool:".bold().yellow(),
      tool.cyan(),
      tool_discrepancies.len()
    );

    for discrepancy in tool_discrepancies {
      let icon = match discrepancy.issue {
        IssueType::Missing => "⊘".red(),
        IssueType::NotSymlink => "⚠".yellow(),
        IssueType::WrongTarget | IssueType::ContentDiffers => "↻".yellow(),
        IssueType::MissingRepo => "✗".red(),
        IssueType::BrokenSymlink => "⚠".red(),
      };

      let profile_info = discrepancy.file.profile.as_ref().map_or_else(
        || "".normal(),
        |profile| format!(" [profile: {profile}]").yellow(),
      );

      println!("  {} {}", icon, discrepancy.message.bright_white());
      println!(
        "      Repo: {}",
        discrepancy
          .file
          .repo_path
          .display()
          .to_string()
          .bright_black()
      );
      println!(
        "      Dest: {}{}",
        discrepancy
          .file
          .dest_path
          .display()
          .to_string()
          .bright_black(),
        profile_info
      );
    }
  }

  println!("\n{}", "=".repeat(80).red());
  println!(
    "{} Found {} discrepancy(ies) in tracked files",
    "→".cyan(),
    discrepancies.len()
  );
  println!(
    "\n{}",
    "Tip: Run 'sync' command to fix these issues"
      .yellow()
      .italic()
  );
}

/// Format a file path as a string.
///
/// TODO: This really should do more than just display the path raw
fn format_file_path(path: &std::path::Path) -> String {
  path.display().to_string()
}

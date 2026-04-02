/// Diff utilities for comparing file states.
pub mod diff;
/// Git operations and repository management.
pub mod git;
/// History tracking for operations.
pub mod history;
/// Hook execution for pre/post operations.
pub mod hooks;
/// Transaction management for atomic operations.
pub mod transactions;
pub use diff::unified_diff;
pub use git::{
    add_remote, commit_changes, detect_changes, init_repo, list_remotes, pull_from_remote,
    push_to_remote, remove_remote, set_remote_url, show_git_status, stage_changes,
};
pub use history::read_events;
pub use transactions::{FileOperation, Transaction};

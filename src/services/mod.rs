pub mod git;
pub mod transactions;
pub use git::{
    add_remote, commit_changes, detect_changes, init_repo, list_remotes, pull_from_remote,
    push_to_remote, remove_remote, set_remote_url, show_git_status, stage_changes,
};
pub use transactions::{FileOperation, Transaction};

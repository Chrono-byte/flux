pub mod browser;
pub mod git;
pub mod transactions;

pub use browser::{
    detect_alacritty_configs, detect_firefox_profiles, detect_starship_configs,
    detect_zen_profiles, get_browser_profile_files,
};
pub use git::{
    add_remote, commit_changes, detect_changes, init_repo, list_remotes, pull_from_remote,
    push_to_remote, remove_remote, set_remote_url, stage_changes,
};
pub use transactions::{FileOperation, Transaction};

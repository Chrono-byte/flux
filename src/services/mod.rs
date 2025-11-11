pub mod browser;
pub mod git;
pub mod package_manager;
pub mod service_manager;

pub use browser::{
    detect_alacritty_configs, detect_firefox_profiles, detect_starship_configs,
    detect_zen_profiles, get_browser_profile_files,
};
pub use git::{
    add_remote, commit_changes, detect_changes, init_repo, list_remotes, push_to_remote,
    remove_remote, set_remote_url, stage_changes,
};
pub use package_manager::{DNFPackageManager, PackageManager};
pub use service_manager::{ServiceManager, SystemdServiceManager};


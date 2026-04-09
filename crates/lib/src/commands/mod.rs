/// Commands for the Flux CLI.
///
/// # Commands
/// Modules for the Flux CLI commands.
/// Apply command module.
pub mod apply;
/// Doctor command module.
pub mod doctor;
/// Migrate command module.
pub mod migrate;
/// Status command module.
pub mod status;
/// Uncheck command module.
pub mod untracked;
/// Validate command module.
pub mod validate;

pub use apply::{ApplyOptions, ApplyReport, ApplyStatus, apply_config, compare_states};
pub use doctor::run_health_checks;
pub use migrate::{MigrateReport, migrate_files};
pub use status::check_status;
pub use untracked::find_discrepancies;
pub use validate::validate_config;

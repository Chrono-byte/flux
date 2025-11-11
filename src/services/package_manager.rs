use crate::types::{InstalledPackage, PackageInfo, PackageSource};
use crate::utils::error::{DotfilesError, Result};
use futures_util::StreamExt;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use zbus::{Connection, proxy};

/// Abstract package manager interface
pub trait PackageManager: Send + Sync {
    /// Check if package is installed
    fn is_installed(&self, package: &str) -> Result<bool>;

    /// Get installed version
    fn get_version(&self, package: &str) -> Result<Option<String>>;

    /// Install package(s) - packages is Vec of (name, version) tuples
    fn install(&self, packages: &[(&str, &str)]) -> Result<()>;

    /// Remove package(s)
    fn remove(&self, packages: &[&str]) -> Result<()>;

    /// Update package(s)
    #[allow(dead_code)]
    fn update(&self, packages: &[&str]) -> Result<()>;

    /// List all installed packages
    fn list_installed(&self) -> Result<Vec<InstalledPackage>>;

    /// Search for package
    #[allow(dead_code)]
    fn search(&self, query: &str) -> Result<Vec<PackageInfo>>;

    /// Check for package conflicts
    #[allow(dead_code)]
    fn check_conflicts(&self, packages: &[&str]) -> Result<Vec<String>>;
}

/// DNF-based package manager for Fedora
pub struct DnfPackageManager {
    use_sudo: bool,
}

impl DnfPackageManager {
    pub fn new(use_sudo: bool) -> Self {
        Self { use_sudo }
    }

    /// Execute a DNF command and return the output
    fn dnf_command(&self, args: &[&str]) -> Result<String> {
        // Check if dnf is available
        if !self.is_dnf_available() {
            return Err(DotfilesError::Path(
                "DNF not found. This feature requires DNF package manager (Fedora/RHEL).\n  💡 Install DNF or run on a Fedora-based system.".to_string()
            ));
        }

        let mut cmd = if self.use_sudo {
            let mut c = Command::new("sudo");
            c.arg("dnf");
            c
        } else {
            Command::new("dnf")
        };

        let output = cmd.args(args).output().map_err(|e| {
            DotfilesError::Path(format!(
                "Failed to execute DNF command: {}\n  💡 Make sure DNF is installed and in PATH",
                e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DotfilesError::Path(format!(
                "DNF command failed: {}\n  Command: dnf {}",
                stderr,
                args.join(" ")
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Check if DNF is available on the system
    fn is_dnf_available(&self) -> bool {
        Command::new("which")
            .arg("dnf")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

impl PackageManager for DnfPackageManager {
    fn is_installed(&self, package: &str) -> Result<bool> {
        match self.dnf_command(&["list", "installed", package]) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false), // Package not installed returns error, we treat as false
        }
    }

    fn get_version(&self, package: &str) -> Result<Option<String>> {
        let output = match self.dnf_command(&["info", "installed", package]) {
            Ok(out) => out,
            Err(_) => return Ok(None), // Not installed
        };

        // Parse version from output
        // Format: "Version      : 2.41.0"
        for line in output.lines() {
            if line.starts_with("Version")
                && let Some(version) = line.split(':').nth(1)
            {
                return Ok(Some(version.trim().to_string()));
            }
        }

        Ok(None)
    }

    fn install(&self, packages: &[(&str, &str)]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        let mut args = vec!["install", "-y"];
        let specs: Vec<String> = packages
            .iter()
            .map(|(name, version)| {
                if version == &"latest" {
                    name.to_string()
                } else {
                    format!("{}-{}", name, version)
                }
            })
            .collect();

        let spec_strs: Vec<&str> = specs.iter().map(|s| s.as_str()).collect();
        args.extend(spec_strs);

        self.dnf_command(&args)?;
        Ok(())
    }

    fn remove(&self, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        let mut args = vec!["remove", "-y"];
        args.extend(packages);
        self.dnf_command(&args)?;
        Ok(())
    }

    fn update(&self, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            // Update all packages
            self.dnf_command(&["upgrade", "-y"])?;
        } else {
            let mut args = vec!["upgrade", "-y"];
            args.extend(packages);
            self.dnf_command(&args)?;
        }
        Ok(())
    }

    fn list_installed(&self) -> Result<Vec<InstalledPackage>> {
        let output = self.dnf_command(&["list", "installed", "--quiet"])?;

        let mut packages = Vec::new();

        // Skip header lines and parse package list
        let mut in_packages = false;
        for line in output.lines() {
            // Skip until we hit the "Installed Packages" section
            if line.contains("Installed Packages") {
                in_packages = true;
                continue;
            }

            if !in_packages {
                continue;
            }

            // Parse line format: "package.arch    version    repo"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                // Extract package name (remove .arch suffix)
                let name_with_arch = parts[0];
                let name = if let Some(dot_pos) = name_with_arch.rfind('.') {
                    &name_with_arch[..dot_pos]
                } else {
                    name_with_arch
                };

                packages.push(InstalledPackage {
                    name: name.to_string(),
                    version: parts[1].to_string(),
                    source: if parts.len() >= 3 {
                        parts[2].to_string()
                    } else {
                        "unknown".to_string()
                    },
                });
            }
        }

        Ok(packages)
    }

    fn search(&self, query: &str) -> Result<Vec<PackageInfo>> {
        let output = self.dnf_command(&["search", query])?;

        let mut packages = Vec::new();

        for line in output.lines() {
            if line.contains(".") && line.contains(":") {
                // This is a package name line: "package.arch : Description"
                if let Some(colon_pos) = line.find(':') {
                    let name_part = &line[..colon_pos].trim();
                    let desc_part = &line[colon_pos + 1..].trim();

                    // Extract package name (remove .arch)
                    let name = if let Some(dot_pos) = name_part.rfind('.') {
                        &name_part[..dot_pos]
                    } else {
                        name_part
                    };

                    packages.push(PackageInfo {
                        name: name.to_string(),
                        available_version: "unknown".to_string(), // DNF search doesn't show version
                        description: desc_part.to_string(),
                        source: PackageSource::Fedora,
                    });
                }
            }
        }

        Ok(packages)
    }

    fn check_conflicts(&self, packages: &[&str]) -> Result<Vec<String>> {
        // Use dnf repoquery to check for conflicts
        // This is a simplified implementation
        let mut conflicts = Vec::new();

        for package in packages {
            match self.dnf_command(&["repoquery", "--conflicts", package]) {
                Ok(output) => {
                    if !output.trim().is_empty() {
                        conflicts.push(format!("{}: {}", package, output.trim()));
                    }
                }
                Err(_) => {
                    // Ignore errors for conflict checking
                    continue;
                }
            }
        }

        Ok(conflicts)
    }
}

/// PackageKit D-Bus proxy interface
#[proxy(
    interface = "org.freedesktop.PackageKit",
    default_service = "org.freedesktop.PackageKit",
    default_path = "/org/freedesktop/PackageKit"
)]
trait PackageKit {
    /// Get package details
    fn get_details(&self, package_ids: &[&str]) -> zbus::Result<Vec<Vec<String>>>;

    /// Get packages with filter
    fn get_packages(&self, filter: u64) -> zbus::Result<Vec<String>>;

    /// Install packages
    fn install_packages(
        &self,
        transaction_flags: u64,
        package_ids: &[&str],
    ) -> zbus::Result<String>;

    /// Remove packages
    fn remove_packages(
        &self,
        transaction_flags: u64,
        package_ids: &[&str],
        allow_deps: bool,
    ) -> zbus::Result<String>;

    /// Update packages
    fn update_packages(&self, transaction_flags: u64, package_ids: &[&str])
    -> zbus::Result<String>;

    /// Search for packages
    fn search_names(
        &self,
        transaction_flags: u64,
        filters: u64,
        values: &[&str],
    ) -> zbus::Result<String>;

    /// Resolve packages
    fn resolve(&self, filters: u64, packages: &[&str]) -> zbus::Result<Vec<String>>;
}

/// PackageKit Transaction D-Bus proxy interface
#[proxy(
    interface = "org.freedesktop.PackageKit.Transaction",
    default_service = "org.freedesktop.PackageKit"
)]
trait Transaction {
    /// Set the role for the transaction
    fn set_role(&self, role: u32) -> zbus::Result<()>;

    /// Set whether to allow reinstall
    fn set_allow_reinstall(&self, allow_reinstall: bool) -> zbus::Result<()>;

    /// Set whether to only download
    fn set_only_download(&self, only_download: bool) -> zbus::Result<()>;

    /// Set whether to simulate
    fn set_simulate(&self, simulate: bool) -> zbus::Result<()>;

    /// Run the transaction
    fn run(&self) -> zbus::Result<()>;

    /// Cancel the transaction
    fn cancel(&self) -> zbus::Result<()>;

    /// Get the role
    #[zbus(property)]
    fn role(&self) -> zbus::Result<u32>;

    /// Get the status
    #[zbus(property)]
    fn status(&self) -> zbus::Result<u32>;

    /// Get the percentage
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<u32>;

    /// Get the package
    #[zbus(property)]
    fn package(&self) -> zbus::Result<String>;

    /// Get the item progress
    #[zbus(property)]
    fn item_progress(&self) -> zbus::Result<(String, u32, u32)>;

    /// Signal: Package
    #[zbus(signal)]
    fn package(&self, info: u32, package_id: &str, summary: &str) -> zbus::Result<()>;

    /// Signal: ErrorCode
    #[zbus(signal)]
    fn error_code(&self, code: u32, details: &str) -> zbus::Result<()>;

    /// Signal: Finished
    #[zbus(signal)]
    fn finished(&self, exit: u32, runtime: u32) -> zbus::Result<()>;
}

/// PackageKit transaction status codes
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionStatus {
    Unknown = 0,
    Wait = 1,
    Running = 2,
    Query = 3,
    Info = 4,
    Remove = 5,
    RefreshCache = 6,
    Download = 7,
    Install = 8,
    Update = 9,
    Cleanup = 10,
    Obsolete = 11,
    DepResolve = 12,
    SigCheck = 13,
    TestCommit = 14,
    Commit = 15,
    Request = 16,
    Finished = 17,
    Cancel = 18,
    DownloadRepository = 19,
    DownloadPackagelist = 20,
    DownloadFilelist = 21,
    DownloadChangelog = 22,
    DownloadGroup = 23,
    DownloadUpdateinfo = 24,
    Repackaging = 25,
    LoadingCache = 26,
    ScanApplications = 27,
    GeneratePackageList = 28,
    WaitingForLock = 29,
    WaitingForAuth = 30,
    ScanProcessList = 31,
    CheckExecutableFiles = 32,
    CheckLibraries = 33,
    CopyFiles = 34,
}

/// PackageKit exit codes
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitCode {
    Success = 0,
    Failed = 1,
    Cancelled = 2,
    KeyRequired = 3,
    EulaRequired = 4,
    Killed = 5,
    MediaChangeRequired = 6,
    NotFound = 7,
    AlreadyInstalled = 8,
    NotInstalled = 9,
    AlreadyObsolete = 10,
    NotAvailable = 11,
    NoNetwork = 12,
    NoLicense = 13,
    ConfigFilesChanged = 14,
    PackageIdInvalid = 15,
    PackageNotInstalled = 16,
    PackageNotAvailable = 17,
    PackageObsolete = 18,
    PackageAlreadyInstalled = 19,
    PackageAlreadyObsolete = 20,
    PackageNotInRepository = 21,
    UpdateNotFound = 22,
    CannotWriteRepoConfig = 23,
    RepoInvalid = 24,
    CannotRemoveSystemPackage = 25,
    CannotUpdateSystemPackage = 26,
    CannotInstallSourcePackage = 27,
    MediaRequired = 28,
    NotSupported = 29,
    InternalError = 30,
    GpgFailure = 31,
    PackageIdMismatch = 32,
    PackageNotTrusted = 33,
    PackageFileNotFound = 34,
    PackageFileInvalid = 35,
    PackageFileBlacklisted = 36,
    RepoConfigurationError = 37,
    InvalidPackageFile = 38,
    PackageFileAccessDenied = 39,
    RepoNotAvailable = 40,
    CannotFetchSource = 41,
    CannotRemoveLastRepo = 42,
    CannotDisableRepo = 43,
    RestrictedDownload = 44,
    PackageCorrupt = 45,
    AllPackagesAlreadyInstalled = 46,
    FileNotFound = 47,
    NoMoreMirrorsToTry = 48,
    NoDistroUpgradeData = 49,
    IncompatibleArchitecture = 50,
    NoSpaceOnDevice = 51,
    MediaCheckRequired = 52,
    NotAuthorized = 53,
    UpdateNotSecurity = 54,
}

/// Transaction result
#[derive(Debug)]
struct TransactionResult {
    success: bool,
    error: Option<String>,
}

/// PackageKit-based package manager (GNOME)
/// Uses D-Bus interface for PackageKit operations
pub struct PackageKitPackageManager {
    use_sudo: bool,
    runtime: tokio::runtime::Runtime,
}

impl PackageKitPackageManager {
    pub fn new(use_sudo: bool) -> Self {
        let runtime =
            tokio::runtime::Runtime::new().expect("Failed to create tokio runtime for PackageKit");

        Self { use_sudo, runtime }
    }

    /// Check if PackageKit D-Bus service is available
    pub fn is_packagekit_available(&self) -> bool {
        // Check if PackageKit D-Bus service is running
        match std::process::Command::new("dbus-send")
            .arg("--system")
            .arg("--print-reply")
            .arg("--dest=org.freedesktop.PackageKit")
            .arg("/org/freedesktop/PackageKit")
            .arg("org.freedesktop.DBus.Properties.Get")
            .arg("string:org.freedesktop.PackageKit")
            .arg("string:VersionMajor")
            .output()
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Get D-Bus connection (async helper)
    async fn get_connection_async(&self) -> Result<Connection> {
        let connection = if self.use_sudo {
            // For system operations, use system bus
            Connection::system().await
        } else {
            // For user operations, use session bus
            Connection::session().await
        }
        .map_err(|e| {
            DotfilesError::Path(format!(
                "Failed to connect to D-Bus: {}\n  💡 Make sure PackageKit is running",
                e
            ))
        })?;

        Ok(connection)
    }

    /// Get PackageKit proxy (async helper)
    async fn get_proxy_async(&self) -> Result<PackageKitProxy<'static>> {
        let connection = self.get_connection_async().await?;
        PackageKitProxy::new(&connection).await
            .map_err(|e| DotfilesError::Path(
                format!("Failed to create PackageKit proxy: {}\n  💡 Make sure PackageKit service is available", e)
            ))
    }

    /// Convert package name to PackageKit ID format
    /// Format: "name;version;arch;data"
    fn package_name_to_id(&self, name: &str, version: Option<&str>) -> String {
        if let Some(ver) = version {
            format!("{};{};;", name, ver)
        } else {
            format!("{};;;", name)
        }
    }

    /// Extract package name from PackageKit ID
    fn package_id_to_name(&self, id: &str) -> String {
        id.split(';').next().unwrap_or(id).to_string()
    }

    /// Run async code in sync context
    fn block_on<F>(&self, future: F) -> F::Output
    where
        F: std::future::Future,
    {
        self.runtime.block_on(future)
    }

    /// Wait for transaction to complete by monitoring signals
    async fn wait_for_transaction(
        &self,
        connection: &Connection,
        transaction_path: &str,
    ) -> Result<TransactionResult> {
        // Create transaction proxy
        let transaction_proxy = TransactionProxy::builder(connection)
            .path(transaction_path)
            .map_err(|e| DotfilesError::Path(format!("Failed to create transaction proxy: {}", e)))?
            .build()
            .await
            .map_err(|e| {
                DotfilesError::Path(format!("Failed to build transaction proxy: {}", e))
            })?;

        // Create channels for communication
        let (tx, rx) = oneshot::channel::<TransactionResult>();
        let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

        // Monitor transaction signals - only Finished and ErrorCode
        let mut finished_stream = transaction_proxy.receive_finished().await.map_err(|e| {
            DotfilesError::Path(format!("Failed to receive finished signals: {}", e))
        })?;

        let mut error_stream = transaction_proxy
            .receive_error_code()
            .await
            .map_err(|e| DotfilesError::Path(format!("Failed to receive error signals: {}", e)))?;

        // Start the transaction
        transaction_proxy
            .run()
            .await
            .map_err(|e| DotfilesError::Path(format!("Failed to start transaction: {}", e)))?;

        // Monitor signals until transaction completes
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            #[allow(clippy::never_loop)]
            loop {
                tokio::select! {
                    Some(msg) = finished_stream.next() => {
                        match msg.args() {
                            Ok(args) => {
                                let exit = *args.exit();
                                let runtime = *args.runtime();
                                log::debug!("Transaction finished: exit={}, runtime={}", exit, runtime);

                                if exit == ExitCode::Success as u32 {
                                    let mut sender = tx_clone.lock().await;
                                    if let Some(s) = sender.take() {
                                        let _ = s.send(TransactionResult {
                                            success: true,
                                            error: None,
                                        });
                                    }
                                } else {
                                    let error_msg = format!("Transaction failed with exit code: {}", exit);
                                    let mut sender = tx_clone.lock().await;
                                    if let Some(s) = sender.take() {
                                        let _ = s.send(TransactionResult {
                                            success: false,
                                            error: Some(error_msg),
                                        });
                                    }
                                }
                                break;
                            }
                            Err(e) => {
                                log::warn!("Failed to parse finished signal args: {}", e);
                                let mut sender = tx_clone.lock().await;
                                if let Some(s) = sender.take() {
                                    let _ = s.send(TransactionResult {
                                        success: false,
                                        error: Some(format!("Failed to parse transaction result: {}", e)),
                                    });
                                }
                                break;
                            }
                        }
                    }
                    Some(msg) = error_stream.next() => {
                        match msg.args() {
                            Ok(args) => {
                                let code = *args.code();
                                let details = args.details().to_string();
                                log::warn!("Transaction error: code={}, details={}", code, details);

                                let error_msg = format!("PackageKit error {}: {}", code, details);
                                let mut sender = tx_clone.lock().await;
                                if let Some(s) = sender.take() {
                                    let _ = s.send(TransactionResult {
                                        success: false,
                                        error: Some(error_msg),
                                    });
                                }
                                break;
                            }
                            Err(e) => {
                                log::warn!("Failed to parse error signal args: {}", e);
                                let mut sender = tx_clone.lock().await;
                                if let Some(s) = sender.take() {
                                    let _ = s.send(TransactionResult {
                                        success: false,
                                        error: Some(format!("Failed to parse error: {}", e)),
                                    });
                                }
                                break;
                            }
                        }
                    }
                }
            }
        });

        // Wait for result with timeout
        match tokio::time::timeout(Duration::from_secs(300), rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err(DotfilesError::Path(
                "Transaction monitoring channel closed unexpectedly".to_string(),
            )),
            Err(_) => Err(DotfilesError::Path(
                "Transaction timed out after 5 minutes".to_string(),
            )),
        }
    }
}

// PackageKit filter constants
#[allow(dead_code)]
const FILTER_INSTALLED: u64 = 1 << 0;
#[allow(dead_code)]
const FILTER_ARCH: u64 = 1 << 1;
#[allow(dead_code)]
const FILTER_NEWEST: u64 = 1 << 2;
#[allow(dead_code)]
const FILTER_NOT_INSTALLED: u64 = 1 << 3;
#[allow(dead_code)]
const FILTER_DEVELOPMENT: u64 = 1 << 4;
#[allow(dead_code)]
const FILTER_NOT_DEVELOPMENT: u64 = 1 << 5;
#[allow(dead_code)]
const FILTER_GUI: u64 = 1 << 6;
#[allow(dead_code)]
const FILTER_NOT_GUI: u64 = 1 << 7;
#[allow(dead_code)]
const FILTER_FREE: u64 = 1 << 8;
#[allow(dead_code)]
const FILTER_NOT_FREE: u64 = 1 << 9;
#[allow(dead_code)]
const FILTER_VISIBLE: u64 = 1 << 10;
#[allow(dead_code)]
const FILTER_NOT_VISIBLE: u64 = 1 << 11;
#[allow(dead_code)]
const FILTER_SUPPORTED: u64 = 1 << 12;
#[allow(dead_code)]
const FILTER_NOT_SUPPORTED: u64 = 1 << 13;
#[allow(dead_code)]
const FILTER_BASENAME: u64 = 1 << 14;
#[allow(dead_code)]
const FILTER_NOT_BASENAME: u64 = 1 << 15;
#[allow(dead_code)]
const FILTER_APPLICATION: u64 = 1 << 16;
#[allow(dead_code)]
const FILTER_NOT_APPLICATION: u64 = 1 << 17;
#[allow(dead_code)]
const FILTER_SOURCE: u64 = 1 << 18;
#[allow(dead_code)]
const FILTER_NOT_SOURCE: u64 = 1 << 19;
#[allow(dead_code)]
const FILTER_COLLECTIONS: u64 = 1 << 20;
#[allow(dead_code)]
const FILTER_NOT_COLLECTIONS: u64 = 1 << 21;
#[allow(dead_code)]
const FILTER_DOWNLOADED: u64 = 1 << 22;
#[allow(dead_code)]
const FILTER_NOT_DOWNLOADED: u64 = 1 << 23;
#[allow(dead_code)]
const FILTER_NONE: u64 = 0;

// Transaction flag constants
const TRANSACTION_FLAG_NONE: u64 = 0;
#[allow(dead_code)]
const TRANSACTION_FLAG_ONLY_TRUSTED: u64 = 1 << 0;
#[allow(dead_code)]
const TRANSACTION_FLAG_SIMULATE: u64 = 1 << 1;
#[allow(dead_code)]
const TRANSACTION_FLAG_ONLY_DOWNLOAD: u64 = 1 << 2;
#[allow(dead_code)]
const TRANSACTION_FLAG_ALLOW_REINSTALL: u64 = 1 << 3;
#[allow(dead_code)]
const TRANSACTION_FLAG_ONLY_DEPENDENCIES: u64 = 1 << 4;
#[allow(dead_code)]
const TRANSACTION_FLAG_FORCE_REINSTALL: u64 = 1 << 5;

impl PackageManager for PackageKitPackageManager {
    fn is_installed(&self, package: &str) -> Result<bool> {
        self.block_on(async {
            let proxy = self.get_proxy_async().await?;
            let packages = proxy.get_packages(FILTER_INSTALLED).await.map_err(|e| {
                DotfilesError::Path(format!("Failed to get installed packages: {}", e))
            })?;

            // Check if package is in the list
            for pkg_id in packages {
                let name = self.package_id_to_name(&pkg_id);
                if name == package {
                    return Ok(true);
                }
            }
            Ok(false)
        })
    }

    fn get_version(&self, package: &str) -> Result<Option<String>> {
        self.block_on(async {
            let proxy = self.get_proxy_async().await?;
            let packages = proxy.get_packages(FILTER_INSTALLED).await.map_err(|e| {
                DotfilesError::Path(format!("Failed to get installed packages: {}", e))
            })?;

            // Find package and extract version
            for pkg_id in packages {
                let parts: Vec<&str> = pkg_id.split(';').collect();
                if parts.len() >= 2 && parts[0] == package {
                    return Ok(Some(parts[1].to_string()));
                }
            }
            Ok(None)
        })
    }

    fn install(&self, packages: &[(&str, &str)]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        self.block_on(async {
            let connection = self.get_connection_async().await?;
            let proxy = self.get_proxy_async().await?;

            // Convert package names to PackageKit IDs
            let package_ids: Vec<String> = packages
                .iter()
                .map(|(name, version)| {
                    let version_str = if *version == "latest" {
                        None
                    } else {
                        Some(*version)
                    };
                    self.package_name_to_id(name, version_str)
                })
                .collect();

            let package_id_refs: Vec<&str> = package_ids.iter().map(|s| s.as_str()).collect();

            // Start transaction and get transaction path
            let transaction_path = proxy
                .install_packages(TRANSACTION_FLAG_NONE, &package_id_refs)
                .await
                .map_err(|e| {
                    DotfilesError::Path(format!("Failed to start install transaction: {}", e))
                })?;

            // Wait for transaction to complete
            let transaction_result = self
                .wait_for_transaction(&connection, &transaction_path)
                .await?;

            if !transaction_result.success {
                return Err(DotfilesError::Path(
                    transaction_result
                        .error
                        .unwrap_or_else(|| "Installation failed".to_string()),
                ));
            }

            Ok(())
        })
    }

    fn remove(&self, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        self.block_on(async {
            let connection = self.get_connection_async().await?;
            let proxy = self.get_proxy_async().await?;

            // Convert package names to PackageKit IDs
            let package_ids: Vec<String> = packages
                .iter()
                .map(|name| self.package_name_to_id(name, None))
                .collect();

            let package_id_refs: Vec<&str> = package_ids.iter().map(|s| s.as_str()).collect();

            // Start transaction and get transaction path
            let transaction_path = proxy
                .remove_packages(
                    TRANSACTION_FLAG_NONE,
                    &package_id_refs,
                    false, // allow_deps
                )
                .await
                .map_err(|e| {
                    DotfilesError::Path(format!("Failed to start remove transaction: {}", e))
                })?;

            // Wait for transaction to complete
            let transaction_result = self
                .wait_for_transaction(&connection, &transaction_path)
                .await?;

            if !transaction_result.success {
                return Err(DotfilesError::Path(
                    transaction_result
                        .error
                        .unwrap_or_else(|| "Removal failed".to_string()),
                ));
            }

            Ok(())
        })
    }

    fn update(&self, packages: &[&str]) -> Result<()> {
        self.block_on(async {
            let connection = self.get_connection_async().await?;
            let proxy = self.get_proxy_async().await?;

            if packages.is_empty() {
                // Update all packages - PackageKit doesn't have a direct "update all"
                // We'd need to get all installed packages first
                return Err(DotfilesError::Path(
                    "PackageKit: Updating all packages not yet implemented".to_string(),
                ));
            }

            // Convert package names to PackageKit IDs
            let package_ids: Vec<String> = packages
                .iter()
                .map(|name| self.package_name_to_id(name, None))
                .collect();

            let package_id_refs: Vec<&str> = package_ids.iter().map(|s| s.as_str()).collect();

            // Start transaction and get transaction path
            let transaction_path = proxy
                .update_packages(TRANSACTION_FLAG_NONE, &package_id_refs)
                .await
                .map_err(|e| {
                    DotfilesError::Path(format!("Failed to start update transaction: {}", e))
                })?;

            // Wait for transaction to complete
            let transaction_result = self
                .wait_for_transaction(&connection, &transaction_path)
                .await?;

            if !transaction_result.success {
                return Err(DotfilesError::Path(
                    transaction_result
                        .error
                        .unwrap_or_else(|| "Update failed".to_string()),
                ));
            }

            Ok(())
        })
    }

    fn list_installed(&self) -> Result<Vec<InstalledPackage>> {
        self.block_on(async {
            let proxy = self.get_proxy_async().await?;
            let packages = proxy.get_packages(FILTER_INSTALLED).await.map_err(|e| {
                DotfilesError::Path(format!("Failed to get installed packages: {}", e))
            })?;

            let mut installed = Vec::new();

            // Parse PackageKit IDs: "name;version;arch;data"
            for pkg_id in packages {
                let parts: Vec<&str> = pkg_id.split(';').collect();
                if parts.len() >= 2 {
                    installed.push(InstalledPackage {
                        name: parts[0].to_string(),
                        version: parts[1].to_string(),
                        source: if parts.len() >= 3 && !parts[2].is_empty() {
                            parts[2].to_string()
                        } else {
                            "unknown".to_string()
                        },
                    });
                }
            }

            Ok(installed)
        })
    }

    fn search(&self, query: &str) -> Result<Vec<PackageInfo>> {
        self.block_on(async {
            let proxy = self.get_proxy_async().await?;

            // Search for packages by name
            let _transaction_path = proxy
                .search_names(TRANSACTION_FLAG_NONE, FILTER_NONE, &[query])
                .await
                .map_err(|e| DotfilesError::Path(format!("Failed to search packages: {}", e)))?;

            // Note: PackageKit search returns a transaction path, and results come via signals
            // For now, we'll return an empty list and note that full implementation
            // requires signal monitoring
            Ok(Vec::new())
        })
    }

    fn check_conflicts(&self, packages: &[&str]) -> Result<Vec<String>> {
        self.block_on(async {
            let proxy = self.get_proxy_async().await?;
            let mut conflicts = Vec::new();

            for package in packages {
                match proxy.resolve(FILTER_NONE, &[package]).await {
                    Ok(_) => {
                        // No conflicts
                    }
                    Err(e) => {
                        conflicts.push(format!("{}: {}", package, e));
                    }
                }
            }

            Ok(conflicts)
        })
    }
}

/// Package manager type enum for selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManagerType {
    Dnf,
    PackageKit,
    Auto, // Auto-detect based on availability
}

impl PackageManagerType {
    /// Create a package manager instance based on the type
    pub fn create_manager(&self, use_sudo: bool) -> Box<dyn PackageManager> {
        match self {
            PackageManagerType::Dnf => Box::new(DnfPackageManager::new(use_sudo)),
            PackageManagerType::PackageKit => Box::new(PackageKitPackageManager::new(use_sudo)),
            PackageManagerType::Auto => {
                // Try PackageKit first (preferred for GNOME), then DNF
                if PackageKitPackageManager::new(use_sudo).is_packagekit_available() {
                    Box::new(PackageKitPackageManager::new(use_sudo))
                } else if DnfPackageManager::new(use_sudo).is_dnf_available() {
                    Box::new(DnfPackageManager::new(use_sudo))
                } else {
                    // Fallback to DNF (will error when used)
                    Box::new(DnfPackageManager::new(use_sudo))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dnf_manager_creation() {
        let manager = DnfPackageManager::new(false);
        assert!(!manager.use_sudo);

        let manager_sudo = DnfPackageManager::new(true);
        assert!(manager_sudo.use_sudo);
    }

    #[test]
    fn test_packagekit_manager_creation() {
        let manager = PackageKitPackageManager::new(false);
        assert!(!manager.use_sudo);

        let manager_sudo = PackageKitPackageManager::new(true);
        assert!(manager_sudo.use_sudo);
    }
}

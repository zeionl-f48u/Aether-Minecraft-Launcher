//! Java runtime detection and management.
//!
//! This module provides functionality to discover Java installations on the system,
//! query their versions and architectures, and represent them as structured data.

use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;
use tokio::fs;
use tokio::process::Command;
use log::{debug, warn};

#[cfg(target_os = "windows")]
use winreg::RegKey;
#[cfg(target_os = "windows")]
use winreg::enums::{HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER};

// ============================================================================
//  Error Handling
// ============================================================================

/// Errors that can occur during Java runtime detection.
#[derive(Debug, thiserror::Error)]
pub enum JavaError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to execute Java process: {0}")]
    Process(String),

    #[error("Failed to parse Java version from output")]
    VersionParse,

    #[error("Invalid Java executable path: {0}")]
    InvalidPath(String),

    #[error("Java executable not found")]
    NotFound,
}

pub type JavaResult<T> = Result<T, JavaError>;

// ============================================================================
//  Data Structures
// ============================================================================

/// Architecture of a Java runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86,
    X86_64,
    AArch64,
    Other,
}

/// Represents a discovered Java runtime with its metadata.
#[derive(Debug, Clone)]
pub struct JavaRuntime {
    java_path: String,
    java_version: String,
    java_main_version: u8,
    java_64bit: bool,
    java_arch: Arch,
}

impl JavaRuntime {
    /// Creates a new `JavaRuntime` instance.
    pub fn new(
        java_path: String,
        java_version: String,
        java_main_version: u8,
        java_64bit: bool,
        java_arch: Arch,
    ) -> Self {
        Self {
            java_path,
            java_version,
            java_main_version,
            java_64bit,
            java_arch,
        }
    }

    /// Returns the absolute path to the Java executable.
    pub fn java_path(&self) -> &str {
        &self.java_path
    }

    /// Returns the full version string of the Java runtime.
    pub fn java_version(&self) -> &str {
        &self.java_version
    }

    /// Returns the main version number (e.g., 8, 11, 17).
    pub fn java_main_version(&self) -> u8 {
        self.java_main_version
    }

    /// Returns `true` if the Java runtime is 64-bit.
    pub fn java_64bit(&self) -> bool {
        self.java_64bit
    }

    /// Returns the architecture of the Java runtime.
    pub fn java_arch(&self) -> Arch {
        self.java_arch
    }

    /// Creates a `JavaRuntime` from a given Java executable path.
    ///
    /// This function executes `java -version` and parses the output to extract
    /// version and architecture information.
    pub async fn from_java_path(path: &Path) -> JavaResult<Self> {
        let resolved = locate_path(path).await?;
        let (version_output, is_64bit) = query_java_version_output(&resolved).await?;
        let version = parse_version_string(&version_output)?;
        let main_version = extract_main_version(&version)?;
        let arch = get_exec_arch(&resolved).await?;

        Ok(JavaRuntime {
            java_path: resolved.to_string_lossy().into_owned(),
            java_version: version,
            java_main_version: main_version,
            java_64bit: is_64bit,
            java_arch: arch,
        })
    }
}

// ============================================================================
//  Path Resolution
// ============================================================================

/// Resolves a path to its canonical form, following symlinks.
async fn locate_path(path: &Path) -> JavaResult<PathBuf> {
    match fs::canonicalize(path).await {
        Ok(p) => Ok(p),
        Err(e) => {
            debug!("Failed to canonicalize {:?}: {}", path, e);
            // Return the original path if it exists
            if fs::try_exists(path).await.unwrap_or(false) {
                Ok(path.to_path_buf())
            } else {
                Err(JavaError::InvalidPath(path.to_string_lossy().into_owned()))
            }
        }
    }
}

// ============================================================================
//  Java Version Query
// ============================================================================

/// Executes `java -version` and returns the output (stderr) and 64-bit flag.
async fn query_java_version_output(java_path: &Path) -> JavaResult<(String, bool)> {
    let mut cmd = Command::new(java_path);
    cmd.arg("-version");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
        cmd.creation_flags(0x00000200 | 0x08000000);
    }

    let output = cmd.output().await.map_err(|e| {
        JavaError::Process(format!("Failed to execute {:?}: {}", java_path, e))
    })?;

    if !output.status.success() {
        // Java prints version to stderr even on success, so we check stderr first.
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            let is_64bit = stderr.contains("64-Bit");
            return Ok((stderr.to_string(), is_64bit));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.is_empty() {
            let is_64bit = stdout.contains("64-Bit");
            return Ok((stdout.to_string(), is_64bit));
        }

        return Err(JavaError::Process(format!(
            "Java process exited with {:?} but produced no output",
            output.status.code()
        )));
    }

    // On success, version is printed to stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    let is_64bit = stderr.contains("64-Bit");
    Ok((stderr.to_string(), is_64bit))
}

/// Parses the raw version string from `java -version` output.
fn parse_version_string(output: &str) -> JavaResult<String> {
    // Look for the first line that contains a version number in quotes.
    for line in output.lines() {
        if let Some(start) = line.find('"') {
            if let Some(end) = line[start + 1..].find('"') {
                return Ok(line[start + 1..start + 1 + end].to_string());
            }
        }
    }
    Err(JavaError::VersionParse)
}

/// Extracts the main version number from a version string.
///
/// Supports both old-style ("1.8") and new-style ("11", "17.0.2") version formats.
fn extract_main_version(version: &str) -> JavaResult<u8> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(\d+)(?:\.\d+)*").unwrap());

    let numbers: Vec<u8> = re
        .captures_iter(version)
        .filter_map(|cap| cap[1].parse().ok())
        .collect();

    match numbers.as_slice() {
        [1, minor, ..] => Ok(*minor),
        [major, ..] if *major > 1 => Ok(*major),
        _ => Err(JavaError::VersionParse),
    }
}

// ============================================================================
//  Architecture Detection
// ============================================================================

/// Detects the architecture of a given executable using the `file` command.
async fn get_exec_arch(path: &Path) -> JavaResult<Arch> {
    #[cfg(target_os = "windows")]
    {
        // On Windows, we don't have `file` command reliably; fallback to a simple heuristic.
        // We'll later rely on the 64-bit flag from version output, but here we can return X86_64
        // as a safe default for 64-bit systems. This is not perfect but acceptable for our use case.
        return Ok(Arch::X86_64);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let output = Command::new("file")
            .arg("-b")
            .arg(path)
            .output()
            .await
            .map_err(|e| JavaError::Process(format!("Failed to execute file command: {}", e)))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        if output_str.contains("x86-64") || output_str.contains("64-bit") {
            Ok(Arch::X86_64)
        } else if output_str.contains("ARM") || output_str.contains("aarch64") {
            Ok(Arch::AArch64)
        } else if output_str.contains("80386") || output_str.contains("32-bit") {
            Ok(Arch::X86)
        } else {
            Ok(Arch::Other)
        }
    }
}

// ============================================================================
//  Java Search
// ============================================================================

const EXECUTABLE_NAME: &str = if cfg!(windows) { "java.exe" } else { "java" };

/// Searches the system for all available Java runtimes.
///
/// This function scans common installation directories, the PATH environment variable,
/// and platform-specific locations. It returns a list of valid `JavaRuntime` instances.
pub async fn search_for_java() -> Vec<JavaRuntime> {
    let candidate_paths = collect_all_candidates().await;
    debug!("Found {} candidate Java paths", candidate_paths.len());

    // Process candidates concurrently with a limited degree of parallelism.
    use futures::stream::{self, StreamExt};
    let runtimes: Vec<JavaRuntime> = stream::iter(candidate_paths)
        .map(|path| async move {
            match JavaRuntime::from_java_path(&path).await {
                Ok(runtime) => Some(runtime),
                Err(e) => {
                    debug!("Skipping Java at {:?}: {}", path, e);
                    None
                }
            }
        })
        .buffer_unordered(4) // Process up to 4 candidates concurrently
        .filter_map(|opt| async { opt })
        .collect()
        .await;

    // Sort by main version descending, then by path for determinism.
    let mut sorted = runtimes;
    sorted.sort_by(|a, b| {
        b.java_main_version
            .cmp(&a.java_main_version)
            .then(a.java_path.cmp(&b.java_path))
    });

    // Deduplicate by path.
    let mut seen = HashSet::new();
    sorted.retain(|rt| seen.insert(rt.java_path.clone()));

    debug!("Found {} unique Java runtimes", sorted.len());
    sorted
}

// ============================================================================
//  Candidate Collection (Platform-Specific)
// ============================================================================

/// Collects all candidate Java executable paths from the system.
async fn collect_all_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // Platform-specific search paths
    #[cfg(target_os = "windows")]
    {
        candidates.extend(find_registry_java().await);
        candidates.extend(find_common_windows_dirs().await);
        candidates.extend(find_minecraft_runtime().await);
        candidates.extend(find_path_env().await);
        candidates.extend(find_jabba_home().await);
    }

    #[cfg(target_os = "linux")]
    {
        candidates.extend(find_common_linux_dirs().await);
        candidates.extend(find_path_env().await);
        candidates.extend(find_minecraft_runtime().await);
        candidates.extend(find_jabba_home().await);
    }

    #[cfg(target_os = "macos")]
    {
        candidates.extend(find_common_macos_dirs().await);
        candidates.extend(find_path_env().await);
        candidates.extend(find_minecraft_runtime().await);
        candidates.extend(find_jabba_home().await);
    }

    // Remove duplicates and sort
    let mut unique: HashSet<_> = candidates.into_iter().collect();
    let mut result: Vec<_> = unique.drain().collect();
    result.sort();
    result
}

// ----------------------------------------------------------------------------
//  Windows-Specific Search
// ----------------------------------------------------------------------------

#[cfg(target_os = "windows")]
async fn find_registry_java() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Check both HKLM and HKCU
    for (hive, subkey_path) in [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\JavaSoft"),
        (HKEY_CURRENT_USER, r"SOFTWARE\JavaSoft"),
    ] {
        let hklm = match RegKey::predef(hive).open_subkey(subkey_path) {
            Ok(key) => key,
            Err(_) => continue,
        };

        // Subkeys to check for Java installations
        for subkey_name in ["Java Runtime Environment", "Java Development Kit", "JRE", "JDK"] {
            if let Ok(subkey) = hklm.open_subkey(subkey_name) {
                // First try to get the CurrentVersion
                if let Ok(current_version) = subkey.get_value::<String, _>("CurrentVersion") {
                    if let Ok(version_key) = subkey.open_subkey(&current_version) {
                        if let Ok(home) = version_key.get_value::<String, _>("JavaHome") {
                            let home_path = PathBuf::from(home);
                            if let Some(java_path) = check_java_in_dir(&home_path).await {
                                paths.push(java_path);
                            }
                        }
                    }
                }

                // Also try to enumerate all subkeys (for multi-version installations)
                for subkey_result in subkey.enum_keys() {
                    if let Ok(version_name) = subkey_result {
                        if let Ok(version_key) = subkey.open_subkey(&version_name) {
                            if let Ok(home) = version_key.get_value::<String, _>("JavaHome") {
                                let home_path = PathBuf::from(home);
                                if let Some(java_path) = check_java_in_dir(&home_path).await {
                                    paths.push(java_path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    paths
}

#[cfg(target_os = "windows")]
async fn find_common_windows_dirs() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let program_files = std::env::var("ProgramFiles").ok();
    let program_files_x86 = std::env::var("ProgramFiles(x86)").ok();
    let program_files_arm = std::env::var("ProgramFiles(ARM)").ok();

    let roots = [
        program_files.as_deref(),
        program_files_x86.as_deref(),
        program_files_arm.as_deref(),
    ];

    let vendor_dirs = [
        "Java",
        "BellSoft",
        "AdoptOpenJDK",
        "Zulu",
        "Microsoft",
        "Eclipse Foundation",
        "Semeru",
    ];

    for root in roots.iter().flatten() {
        let root_path = PathBuf::from(root);
        for vendor in &vendor_dirs {
            let vendor_path = root_path.join(vendor);
            // Fast path: check <root>/<vendor>/bin/java.exe directly
            let direct = vendor_path.join("bin").join(EXECUTABLE_NAME);
            if fs::try_exists(&direct).await.unwrap_or(false) {
                paths.push(direct);
                continue;
            }

            // Slow path: scan subdirectories
            if let Ok(mut entries) = fs::read_dir(&vendor_path).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(java_path) = check_java_in_dir(&path).await {
                            paths.push(java_path);
                        }
                    }
                }
            }
        }
    }

    paths
}

// ----------------------------------------------------------------------------
//  Linux-Specific Search
// ----------------------------------------------------------------------------

#[cfg(target_os = "linux")]
async fn find_common_linux_dirs() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let dirs = [
        "/usr/java",
        "/usr/lib/jvm",
        "/usr/lib32/jvm",
        "/usr/lib64/jvm",
    ];

    for dir in &dirs {
        let dir_path = PathBuf::from(dir);
        // Fast path: check <dir>/bin/java directly
        let direct = dir_path.join("bin").join(EXECUTABLE_NAME);
        if fs::try_exists(&direct).await.unwrap_or(false) {
            paths.push(direct);
            continue;
        }

        // Slow path: scan subdirectories
        if let Ok(mut entries) = fs::read_dir(&dir_path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir().unwrap_or(false) {
                    if let Some(java_path) = check_java_in_dir(&path).await {
                        paths.push(java_path);
                    }
                }
            }
        }
    }

    paths
}

// ----------------------------------------------------------------------------
//  macOS-Specific Search
// ----------------------------------------------------------------------------

#[cfg(target_os = "macos")]
async fn find_common_macos_dirs() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let dirs = [
        "/Library/Java/JavaVirtualMachines",
        "/System/Library/Java/JavaVirtualMachines",
    ];

    for dir in &dirs {
        let dir_path = PathBuf::from(dir);
        if let Ok(mut entries) = fs::read_dir(&dir_path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir().unwrap_or(false) {
                    // macOS Java Virtual Machines have Contents/Home/bin/java
                    let home = path.join("Contents").join("Home");
                    if let Some(java_path) = check_java_in_dir(&home).await {
                        paths.push(java_path);
                    }
                }
            }
        }
    }

    // Additional macOS-specific paths
    let extra_paths = [
        "/usr/bin/java",
        "/usr/libexec/java_home",
    ];

    for extra in &extra_paths {
        let extra_path = PathBuf::from(extra);
        if fs::try_exists(&extra_path).await.unwrap_or(false) {
            // For /usr/libexec/java_home, we need to execute it to get the actual JAVA_HOME
            if extra == "/usr/libexec/java_home" {
                if let Ok(output) = Command::new(extra).output().await {
                    if output.status.success() {
                        let home = String::from_utf8_lossy(&output.stdout).trim();
                        let home_path = PathBuf::from(home);
                        if let Some(java_path) = check_java_in_dir(&home_path).await {
                            paths.push(java_path);
                        }
                    }
                }
            } else {
                paths.push(extra_path);
            }
        }
    }

    paths
}

// ----------------------------------------------------------------------------
//  Common Search Functions
// ----------------------------------------------------------------------------

/// Checks if a directory contains a `bin/java` executable and returns its path.
async fn check_java_in_dir(dir: &Path) -> Option<PathBuf> {
    let java_path = dir.join("bin").join(EXECUTABLE_NAME);
    if fs::try_exists(&java_path).await.ok()? {
        Some(java_path)
    } else {
        None
    }
}

/// Searches the PATH environment variable for Java executables.
async fn find_path_env() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let java_path = dir.join(EXECUTABLE_NAME);
            if fs::try_exists(&java_path).await.unwrap_or(false) {
                paths.push(java_path);
            }
        }
    }
    paths
}

/// Searches the Minecraft launcher runtime directory.
async fn find_minecraft_runtime() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let mc_runtime = PathBuf::from(appdata)
                .join(".minecraft")
                .join("runtime");
            let runtime_dirs = [
                mc_runtime.join("jre-legacy"),
                mc_runtime.join("java-runtime-alpha"),
                mc_runtime.join("java-runtime-beta"),
                mc_runtime.join("java-runtime-gamma"),
            ];
            for dir in &runtime_dirs {
                if let Some(java_path) = check_java_in_dir(dir).await {
                    paths.push(java_path);
                }
            }
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let mc_runtime = PathBuf::from(home)
                .join(".minecraft")
                .join("runtime");
            let runtime_dirs = [
                mc_runtime.join("jre-legacy"),
                mc_runtime.join("java-runtime-alpha"),
                mc_runtime.join("java-runtime-beta"),
                mc_runtime.join("java-runtime-gamma"),
            ];
            for dir in &runtime_dirs {
                if let Some(java_path) = check_java_in_dir(dir).await {
                    paths.push(java_path);
                }
            }
        }
    }

    paths
}

/// Searches the JABBA_HOME directory for Java executables.
async fn find_jabba_home() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(jabba_home) = std::env::var("JABBA_HOME") {
        let jabba_path = PathBuf::from(jabba_home);
        if let Some(java_path) = check_java_in_dir(&jabba_path).await {
            paths.push(java_path);
        }
    }
    paths
}

// ============================================================================
//  Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_main_version() {
        assert_eq!(extract_main_version("1.8.0_202").unwrap(), 8);
        assert_eq!(extract_main_version("11.0.12").unwrap(), 11);
        assert_eq!(extract_main_version("17.0.2").unwrap(), 17);
        assert_eq!(extract_main_version("openjdk 17.0.5-internal").unwrap(), 17);
        assert_eq!(extract_main_version("1.7.0_80").unwrap(), 7);
        assert!(extract_main_version("invalid").is_err());
    }

    #[test]
    fn test_parse_version_string() {
        let output = "java version \"1.8.0_202\"\nJava(TM) SE Runtime Environment...";
        assert_eq!(parse_version_string(output).unwrap(), "1.8.0_202");

        let output = "openjdk version \"17.0.2\" 2022-01-18";
        assert_eq!(parse_version_string(output).unwrap(), "17.0.2");
    }
}
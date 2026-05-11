use super::InstallMethod;

/// Detected system package manager
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    /// apt (Debian, Ubuntu)
    Apt,
    /// dnf (Fedora, RHEL)
    Dnf,
    /// pacman (Arch Linux)
    Pacman,
    /// zypper (openSUSE)
    Zypper,
}

impl std::fmt::Display for PackageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Apt => write!(f, "apt"),
            Self::Dnf => write!(f, "dnf"),
            Self::Pacman => write!(f, "pacman"),
            Self::Zypper => write!(f, "zypper"),
        }
    }
}

/// Detects the system package manager by checking for known binaries.
#[must_use]
pub fn detect_package_manager() -> Option<PackageManager> {
    use std::path::Path;
    if Path::new("/usr/bin/apt").exists() {
        Some(PackageManager::Apt)
    } else if Path::new("/usr/bin/dnf").exists() {
        Some(PackageManager::Dnf)
    } else if Path::new("/usr/bin/pacman").exists() {
        Some(PackageManager::Pacman)
    } else if Path::new("/usr/bin/zypper").exists() {
        Some(PackageManager::Zypper)
    } else {
        None
    }
}

/// Returns the install command for a system package, if available
/// for the given package manager.
#[must_use]
pub fn get_system_install_command(
    method: &InstallMethod,
    manager: PackageManager,
) -> Option<String> {
    if let InstallMethod::SystemPackage {
        apt,
        dnf,
        pacman,
        zypper,
    } = method
    {
        let pkg = match manager {
            PackageManager::Apt => *apt,
            PackageManager::Dnf => *dnf,
            PackageManager::Pacman => *pacman,
            PackageManager::Zypper => *zypper,
        }?;
        let cmd = match manager {
            PackageManager::Apt => format!("sudo apt install {pkg}"),
            PackageManager::Dnf => format!("sudo dnf install {pkg}"),
            PackageManager::Pacman => format!("sudo pacman -S {pkg}"),
            PackageManager::Zypper => format!("sudo zypper install {pkg}"),
        };
        Some(cmd)
    } else {
        None
    }
}

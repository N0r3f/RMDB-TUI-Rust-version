use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum LinuxDistribution {
    Debian,
    Ubuntu,
    Fedora,
    RHEL,
    CentOS,
    Arch,
    OpenSUSE,
    Alpine,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DistributionInfo {
    pub distro: LinuxDistribution,
    pub version: Option<String>,
    pub package_manager: PackageManager,
}

#[derive(Debug, Clone)]
pub enum PackageManager {
    Apt,      // Debian, Ubuntu
    Yum,      // RHEL, CentOS (ancien)
    Dnf,      // Fedora, RHEL 8+, CentOS 8+
    Pacman,   // Arch
    Zypper,   // OpenSUSE
    Apk,      // Alpine
}

impl DistributionInfo {
    pub fn detect() -> Self {
        // Détecter la distribution
        let distro = Self::detect_distribution();
        let version = Self::detect_version_for_distro(&distro);
        let package_manager = Self::detect_package_manager(&distro);

        Self {
            distro,
            version,
            package_manager,
        }
    }

    fn detect_distribution() -> LinuxDistribution {
        // Vérifier /etc/os-release en premier (standard systemd)
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if line.starts_with("ID=") {
                    let id = line.split('=').nth(1)
                        .map(|s| s.trim_matches('"').to_lowercase())
                        .unwrap_or_default();
                    
                    match id.as_str() {
                        "debian" => return LinuxDistribution::Debian,
                        "ubuntu" => return LinuxDistribution::Ubuntu,
                        "fedora" => return LinuxDistribution::Fedora,
                        "rhel" | "redhat" => return LinuxDistribution::RHEL,
                        "centos" => return LinuxDistribution::CentOS,
                        "arch" | "archlinux" => return LinuxDistribution::Arch,
                        "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" => return LinuxDistribution::OpenSUSE,
                        "alpine" => return LinuxDistribution::Alpine,
                        _ => {}
                    }
                }
                
                // Vérifier aussi ID_LIKE pour les variantes
                if line.starts_with("ID_LIKE=") {
                    let id_like = line.split('=').nth(1)
                        .map(|s| s.trim_matches('"').to_lowercase())
                        .unwrap_or_default();
                    
                    if id_like.contains("rhel") || id_like.contains("fedora") {
                        if id_like.contains("centos") {
                            return LinuxDistribution::CentOS;
                        }
                        return LinuxDistribution::RHEL;
                    }
                    if id_like.contains("debian") {
                        if id_like.contains("ubuntu") {
                            return LinuxDistribution::Ubuntu;
                        }
                        return LinuxDistribution::Debian;
                    }
                }
            }
        }

        // Fallback: vérifier des fichiers spécifiques
        if std::path::Path::new("/etc/debian_version").exists() {
            return LinuxDistribution::Debian;
        }
        if std::path::Path::new("/etc/redhat-release").exists() {
            if let Ok(content) = std::fs::read_to_string("/etc/redhat-release") {
                let content_lower = content.to_lowercase();
                if content_lower.contains("centos") {
                    return LinuxDistribution::CentOS;
                }
                if content_lower.contains("fedora") {
                    return LinuxDistribution::Fedora;
                }
                return LinuxDistribution::RHEL;
            }
        }
        if std::path::Path::new("/etc/arch-release").exists() {
            return LinuxDistribution::Arch;
        }
        if std::path::Path::new("/etc/alpine-release").exists() {
            return LinuxDistribution::Alpine;
        }

        LinuxDistribution::Unknown
    }

    fn detect_version_for_distro(distro: &LinuxDistribution) -> Option<String> {
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if line.starts_with("VERSION_ID=") {
                    return line.split('=')
                        .nth(1)
                        .map(|s| s.trim_matches('"').to_string());
                }
            }
        }

        // Fallback pour certaines distributions
        match distro {
            LinuxDistribution::Debian => {
                std::fs::read_to_string("/etc/debian_version")
                    .ok()
                    .map(|s| s.trim().to_string())
            }
            _ => None,
        }
    }

    fn detect_package_manager(distro: &LinuxDistribution) -> PackageManager {
        match distro {
            LinuxDistribution::Debian | LinuxDistribution::Ubuntu => PackageManager::Apt,
            LinuxDistribution::Fedora | LinuxDistribution::RHEL | LinuxDistribution::CentOS => {
                // RHEL 8+ et CentOS 8+ utilisent dnf, les anciennes versions utilisent yum
                // Vérifier si dnf est disponible
                if Command::new("dnf")
                    .arg("--version")
                    .output()
                    .is_ok()
                {
                    PackageManager::Dnf
                } else {
                    PackageManager::Yum
                }
            }
            LinuxDistribution::Arch => PackageManager::Pacman,
            LinuxDistribution::OpenSUSE => PackageManager::Zypper,
            LinuxDistribution::Alpine => PackageManager::Apk,
            LinuxDistribution::Unknown => {
                // Essayer de détecter le gestionnaire de paquets disponible
                if Command::new("apt-get").arg("--version").output().is_ok() {
                    PackageManager::Apt
                } else if Command::new("dnf").arg("--version").output().is_ok() {
                    PackageManager::Dnf
                } else if Command::new("yum").arg("--version").output().is_ok() {
                    PackageManager::Yum
                } else if Command::new("pacman").arg("--version").output().is_ok() {
                    PackageManager::Pacman
                } else if Command::new("zypper").arg("--version").output().is_ok() {
                    PackageManager::Zypper
                } else if Command::new("apk").arg("--version").output().is_ok() {
                    PackageManager::Apk
                } else {
                    PackageManager::Apt // Par défaut
                }
            }
        }
    }

    pub fn install_command(&self, packages: &[&str]) -> String {
        let packages_str = packages.join(" ");
        match &self.package_manager {
            PackageManager::Apt => format!("apt-get install -y {}", packages_str),
            PackageManager::Yum => format!("yum install -y {}", packages_str),
            PackageManager::Dnf => format!("dnf install -y {}", packages_str),
            PackageManager::Pacman => format!("pacman -S --noconfirm {}", packages_str),
            PackageManager::Zypper => format!("zypper install -y {}", packages_str),
            PackageManager::Apk => format!("apk add {}", packages_str),
        }
    }

    pub fn update_command(&self) -> String {
        match &self.package_manager {
            PackageManager::Apt => "apt-get update".to_string(),
            PackageManager::Yum => "yum check-update || true".to_string(),
            PackageManager::Dnf => "dnf check-update || true".to_string(),
            PackageManager::Pacman => "pacman -Sy".to_string(),
            PackageManager::Zypper => "zypper refresh".to_string(),
            PackageManager::Apk => "apk update".to_string(),
        }
    }

    pub fn package_name(&self, base_package: &str) -> String {
        // Adapter les noms de paquets selon la distribution
        match (base_package, &self.distro) {
            ("lxc-templates", LinuxDistribution::RHEL | LinuxDistribution::CentOS | LinuxDistribution::Fedora) => {
                "lxc-templates".to_string() // Même nom sur RHEL/Fedora
            }
            ("lxc-templates", _) => "lxc-templates".to_string(),
            _ => base_package.to_string(),
        }
    }

    pub fn lxc_config_path(&self) -> String {
        // Sur RHEL/CentOS, LXC peut utiliser des chemins différents
        match self.distro {
            LinuxDistribution::RHEL | LinuxDistribution::CentOS | LinuxDistribution::Fedora => {
                // Vérifier si le répertoire existe
                if std::path::Path::new("/etc/lxc").exists() {
                    "/etc/lxc/default.conf".to_string()
                } else {
                    // Créer dans le home de l'utilisateur
                    format!("{}/.config/lxc/default.conf", std::env::var("HOME").unwrap_or_default())
                }
            }
            _ => {
                format!("{}/.config/lxc/default.conf", std::env::var("HOME").unwrap_or_default())
            }
        }
    }

    pub fn lxc_container_path(&self) -> String {
        match self.distro {
            LinuxDistribution::RHEL | LinuxDistribution::CentOS | LinuxDistribution::Fedora => {
                if std::path::Path::new("/var/lib/lxc").exists() {
                    "/var/lib/lxc".to_string()
                } else {
                    format!("{}/.local/share/lxc", std::env::var("HOME").unwrap_or_default())
                }
            }
            _ => {
                if std::path::Path::new("/var/lib/lxc").exists() {
                    "/var/lib/lxc".to_string()
                } else {
                    format!("{}/.local/share/lxc", std::env::var("HOME").unwrap_or_default())
                }
            }
        }
    }

    pub fn needs_root_for_lxc(&self) -> bool {
        // Sur RHEL/CentOS, LXC nécessite souvent root ou des mappings UID
        matches!(self.distro, LinuxDistribution::RHEL | LinuxDistribution::CentOS)
    }
}

impl std::fmt::Display for LinuxDistribution {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LinuxDistribution::Debian => write!(f, "debian"),
            LinuxDistribution::Ubuntu => write!(f, "ubuntu"),
            LinuxDistribution::Fedora => write!(f, "fedora"),
            LinuxDistribution::RHEL => write!(f, "rhel"),
            LinuxDistribution::CentOS => write!(f, "centos"),
            LinuxDistribution::Arch => write!(f, "arch"),
            LinuxDistribution::OpenSUSE => write!(f, "opensuse"),
            LinuxDistribution::Alpine => write!(f, "alpine"),
            LinuxDistribution::Unknown => write!(f, "unknown"),
        }
    }
}


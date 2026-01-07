/// Module principal d'installation de RMDB
/// Gère l'installation sur host, container Alpine, et VM Rocky Linux
use crate::pres::executor::{CommandExecutor, CommandOutput, ExecError};
use crate::data::distribution::DistributionInfo;
use crate::deployment::logger::DeploymentLogger;

/// Versions requises pour RMDB
pub const REQUIRED_RUST_VERSION: &str = "1.70.0";
pub const REQUIRED_GO_VERSION: &str = "1.21.0";

/// Mode d'utilisation de RMDB
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationMode {
    /// Client graphique desktop (Mode normal)
    DesktopGUI,
    /// Via le navigateur web (Mode serveur)
    WebServer,
    /// En terminal TUI (Mode avancé)
    TerminalTUI,
}

impl InstallationMode {
    /// Nom d'affichage du mode
    pub fn display_name(&self) -> &'static str {
        match self {
            InstallationMode::DesktopGUI => "Client graphique desktop (Mode normal)",
            InstallationMode::WebServer => "Via le navigateur web (Mode serveur)",
            InstallationMode::TerminalTUI => "En terminal TUI (Mode avancé)",
        }
    }

    /// Nom court du mode
    pub fn short_name(&self) -> &'static str {
        match self {
            InstallationMode::DesktopGUI => "desktop",
            InstallationMode::WebServer => "web",
            InstallationMode::TerminalTUI => "tui",
        }
    }
}

/// Type d'installation RMDB
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstallationType {
    /// Installation sur le système hôte
    Host,
    /// Installation dans un container Alpine Linux
    ContainerAlpine,
    /// Installation dans une VM Rocky Linux
    VMRocky,
}

/// Configuration d'installation
pub struct InstallationConfig {
    pub installation_type: InstallationType,
    pub rmdb_source_path: String,
    pub container_name: Option<String>,
    pub vm_name: Option<String>,
    pub alpine_version: Option<String>,
    pub rocky_version: Option<String>,
    pub install_rust: bool,
    pub install_go: bool,
    pub logger: Option<DeploymentLogger>,
    pub installation_mode: Option<InstallationMode>,
}

impl InstallationConfig {
    pub fn new(installation_type: InstallationType, rmdb_source_path: String) -> Self {
        Self {
            installation_type,
            rmdb_source_path,
            container_name: None,
            vm_name: None,
            alpine_version: Some("3.19".to_string()),
            rocky_version: Some("9".to_string()),
            install_rust: true,
            install_go: true,
            logger: None,
            installation_mode: None,
        }
    }

    pub fn with_logger(mut self, logger: DeploymentLogger) -> Self {
        self.logger = Some(logger);
        self
    }

    pub fn with_container_name(mut self, name: String) -> Self {
        self.container_name = Some(name);
        self
    }

    pub fn with_vm_name(mut self, name: String) -> Self {
        self.vm_name = Some(name);
        self
    }

    pub fn with_alpine_version(mut self, version: String) -> Self {
        self.alpine_version = Some(version);
        self
    }

    pub fn with_rocky_version(mut self, version: String) -> Self {
        self.rocky_version = Some(version);
        self
    }

    pub fn with_rust_install(mut self, install: bool) -> Self {
        self.install_rust = install;
        self
    }

    pub fn with_go_install(mut self, install: bool) -> Self {
        self.install_go = install;
        self
    }

    pub fn with_installation_mode(mut self, mode: InstallationMode) -> Self {
        self.installation_mode = Some(mode);
        self
    }
}

/// Gestionnaire d'installation principal
pub struct RMDBInstaller {
    config: InstallationConfig,
    distribution: DistributionInfo,
}

impl RMDBInstaller {
    pub fn new(config: InstallationConfig) -> Self {
        Self {
            config,
            distribution: DistributionInfo::detect(),
        }
    }

    /// Exécute l'installation complète selon le type choisi
    pub fn install(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        match self.config.installation_type {
            InstallationType::Host => self.install_on_host(executor),
            InstallationType::ContainerAlpine => self.install_in_container(executor),
            InstallationType::VMRocky => self.install_in_vm(executor),
        }
    }

    /// Installation sur le système hôte
    fn install_on_host(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        self.log_info("Début de l'installation de RMDB sur le système hôte");

        // Étape 1: Installer Rust si nécessaire
        if self.config.install_rust {
            self.log_info("Vérification/installation de Rust...");
            let rust_installer = RustInstaller::new();
            rust_installer.install(executor)?;
        }

        // Étape 2: Installer Go si nécessaire
        if self.config.install_go {
            self.log_info("Vérification/installation de Go...");
            let go_installer = GoInstaller::new();
            go_installer.install(executor)?;
        }

        // Étape 3: Installer RMDB sur le host
        self.log_info("Installation de RMDB sur le système hôte...");
        let host_deployment = crate::deployment::host::HostDeployment::new();
        host_deployment.install_rmdb(executor, &self.config.rmdb_source_path)?;

        self.log_info("Installation sur le système hôte terminée avec succès");
        Ok(CommandOutput {
            stdout: "RMDB installé avec succès sur le système hôte".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }

    /// Installation dans un container Alpine
    fn install_in_container(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        self.log_info("Début de l'installation de RMDB dans un container Alpine");

        let container_name = self.config.container_name.as_ref()
            .ok_or_else(|| ExecError::Failed("Nom du container requis".to_string()))?;
        let alpine_version = self.config.alpine_version.as_ref()
            .ok_or_else(|| ExecError::Failed("Version Alpine requise".to_string()))?;

        // Étape 1: Installer Rust et Go sur le host si nécessaire
        if self.config.install_rust {
            self.log_info("Vérification/installation de Rust sur le host...");
            let rust_installer = RustInstaller::new();
            rust_installer.install(executor)?;
        }

        if self.config.install_go {
            self.log_info("Vérification/installation de Go sur le host...");
            let go_installer = GoInstaller::new();
            go_installer.install(executor)?;
        }

        // Étape 2: Créer le container Alpine
        self.log_info(&format!("Création du container Alpine {}...", container_name));
        let logger = if let Some(ref _existing_logger) = self.config.logger {
            DeploymentLogger::new().unwrap_or_else(|_| DeploymentLogger::default())
        } else {
            DeploymentLogger::new().unwrap_or_else(|_| DeploymentLogger::default())
        };
        let lxc_deployment = crate::deployment::lxc::LXCDeployment::new(
            container_name.clone(),
            alpine_version.clone(),
        ).with_logger(logger);

        lxc_deployment.create_container(executor)?;

        // Étape 3: Installer RMDB dans le container
        self.log_info("Installation de RMDB dans le container...");
        lxc_deployment.install_rmdb_in_container(executor, &self.config.rmdb_source_path)?;

        self.log_info("Installation dans le container terminée avec succès");
        Ok(CommandOutput {
            stdout: format!("RMDB installé avec succès dans le container {}", container_name),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }

    /// Installation dans une VM Rocky Linux
    fn install_in_vm(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        self.log_info("Début de l'installation de RMDB dans une VM Rocky Linux");

        let vm_name = self.config.vm_name.as_ref()
            .ok_or_else(|| ExecError::Failed("Nom de la VM requise".to_string()))?;
        let rocky_version = self.config.rocky_version.as_ref()
            .ok_or_else(|| ExecError::Failed("Version Rocky requise".to_string()))?;

        // Étape 1: Installer Rust et Go sur le host si nécessaire
        if self.config.install_rust {
            self.log_info("Vérification/installation de Rust sur le host...");
            let rust_installer = RustInstaller::new();
            rust_installer.install(executor)?;
        }

        if self.config.install_go {
            self.log_info("Vérification/installation de Go sur le host...");
            let go_installer = GoInstaller::new();
            go_installer.install(executor)?;
        }

        // Étape 2: Créer la VM Rocky Linux
        self.log_info(&format!("Création de la VM Rocky Linux {}...", vm_name));
        let vm_deployment = crate::deployment::vm::VMDeployment::new(
            vm_name.clone(),
            rocky_version.clone(),
        );

        vm_deployment.create_vm(executor)?;

        // Étape 3: Installer RMDB dans la VM
        self.log_info("Installation de RMDB dans la VM...");
        vm_deployment.install_rmdb_in_vm(executor, &self.config.rmdb_source_path)?;

        self.log_info("Installation dans la VM terminée avec succès");
        Ok(CommandOutput {
            stdout: format!("RMDB installé avec succès dans la VM {}", vm_name),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }

    fn log_info(&self, message: &str) {
        if let Some(ref logger) = self.config.logger {
            logger.info(message);
        }
    }
}

/// Installateur de Rust
pub struct RustInstaller {
    distribution: DistributionInfo,
}

impl RustInstaller {
    pub fn new() -> Self {
        Self {
            distribution: DistributionInfo::detect(),
        }
    }

    /// Vérifie si Rust est installé avec la bonne version
    pub fn check_rust_installed(&self, executor: &CommandExecutor) -> Result<bool, ExecError> {
        let cmd = "command -v rustc >/dev/null 2>&1 && rustc --version 2>/dev/null || echo 'not_installed'";
        let output = executor.run_shell(cmd, false)?;
        
        if output.stdout.contains("not_installed") {
            return Ok(false);
        }

        // Extraire la version de Rust
        let version_str = output.stdout.trim();
        if let Some(version) = version_str.split_whitespace().nth(1) {
            // Comparer les versions (simplifié)
            Ok(self.compare_versions(version, REQUIRED_RUST_VERSION))
        } else {
            Ok(false)
        }
    }

    /// Installe Rust via rustup
    pub fn install(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        // Vérifier si Rust est déjà installé avec la bonne version
        if let Ok(true) = self.check_rust_installed(executor) {
            return Ok(CommandOutput {
                stdout: "Rust est déjà installé avec la bonne version".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
            });
        }

        // Vérifier si rustup est installé
        let rustup_check = "command -v rustup >/dev/null 2>&1 && echo 'installed' || echo 'not_installed'";
        let rustup_output = executor.run_shell(rustup_check, false)?;

        if rustup_output.stdout.contains("not_installed") {
            // Installer rustup
            let install_rustup = "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable";
            executor.run_shell(install_rustup, false)?;

            // Ajouter rustup au PATH
            let add_path = "source $HOME/.cargo/env";
            executor.run_shell(add_path, false)?;
        }

        // Installer/mettre à jour Rust vers la version requise
        let install_rust = format!("rustup install stable && rustup default stable && rustup update stable");
        executor.run_shell(&install_rust, false)?;

        Ok(CommandOutput {
            stdout: format!("Rust {} installé avec succès", REQUIRED_RUST_VERSION),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }

    /// Compare deux versions
    fn compare_versions(&self, installed: &str, required: &str) -> bool {
        // Nettoyer les versions (enlever les préfixes "rustc", etc.)
        let installed_clean = installed.trim().trim_start_matches("rustc").trim();
        let required_clean = required.trim();

        // Comparaison simplifiée des versions (sémantique basique)
        let installed_parts: Vec<&str> = installed_clean.split('.').collect();
        let required_parts: Vec<&str> = required_clean.split('.').collect();
        
        for i in 0..installed_parts.len().max(required_parts.len()) {
            let installed_num: u32 = installed_parts.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            let required_num: u32 = required_parts.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            
            if installed_num > required_num {
                return true;
            } else if installed_num < required_num {
                return false;
            }
        }
        
        true
    }
}

/// Installateur de Go
pub struct GoInstaller {
    distribution: DistributionInfo,
}

impl GoInstaller {
    pub fn new() -> Self {
        Self {
            distribution: DistributionInfo::detect(),
        }
    }

    /// Vérifie si Go est installé avec la bonne version
    pub fn check_go_installed(&self, executor: &CommandExecutor) -> Result<bool, ExecError> {
        let cmd = "command -v go >/dev/null 2>&1 && go version 2>/dev/null || echo 'not_installed'";
        let output = executor.run_shell(cmd, false)?;
        
        if output.stdout.contains("not_installed") {
            return Ok(false);
        }

        // Extraire la version de Go
        let version_str = output.stdout.trim();
        if let Some(version) = version_str.split_whitespace().nth(2) {
            // Go version format: "go1.21.0" -> "1.21.0"
            let version_clean = version.trim_start_matches("go");
            Ok(self.compare_versions(version_clean, REQUIRED_GO_VERSION))
        } else {
            Ok(false)
        }
    }

    /// Installe Go avec la version requise
    pub fn install(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        // Vérifier si Go est déjà installé avec la bonne version
        if let Ok(true) = self.check_go_installed(executor) {
            return Ok(CommandOutput {
                stdout: "Go est déjà installé avec la bonne version".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
            });
        }

        // Télécharger et installer Go depuis le site officiel
        let go_version = REQUIRED_GO_VERSION;
        
        // Détecter l'architecture
        let (go_os, go_arch) = self.detect_go_architecture(executor)?;
        let go_tarball = format!("go{}.{}-{}.tar.gz", go_version, go_os, go_arch);
        let go_url = format!("https://go.dev/dl/{}", go_tarball);

        // Télécharger Go
        let download_cmd = format!(
            "cd /tmp && curl -L -o {} {}",
            go_tarball, go_url
        );
        executor.run_shell(&download_cmd, false)?;

        // Supprimer l'ancienne installation si elle existe
        let remove_old = "sudo rm -rf /usr/local/go";
        let _ = executor.run_shell(remove_old, true);

        // Extraire Go
        let extract_cmd = format!(
            "sudo tar -C /usr/local -xzf /tmp/{}",
            go_tarball
        );
        executor.run_shell(&extract_cmd, true)?;

        // Ajouter Go au PATH dans /etc/profile.d/go.sh
        let path_setup = r#"#!/bin/bash
export PATH=$PATH:/usr/local/go/bin
"#;
        let setup_cmd = format!(
            "sudo bash -c 'cat > /etc/profile.d/go.sh << \"EOF\"\n{}\nEOF' && sudo chmod +x /etc/profile.d/go.sh",
            path_setup
        );
        executor.run_shell(&setup_cmd, true)?;

        // Nettoyer
        let cleanup = format!("rm -f /tmp/{}", go_tarball);
        let _ = executor.run_shell(&cleanup, false);

        Ok(CommandOutput {
            stdout: format!("Go {} installé avec succès", go_version),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }

    /// Compare deux versions
    fn compare_versions(&self, installed: &str, required: &str) -> bool {
        // Nettoyer les versions (enlever les préfixes "go", etc.)
        let installed_clean = installed.trim().trim_start_matches("go").trim();
        let required_clean = required.trim();

        // Comparaison simplifiée des versions (sémantique basique)
        let installed_parts: Vec<&str> = installed_clean.split('.').collect();
        let required_parts: Vec<&str> = required_clean.split('.').collect();
        
        for i in 0..installed_parts.len().max(required_parts.len()) {
            let installed_num: u32 = installed_parts.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            let required_num: u32 = required_parts.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            
            if installed_num > required_num {
                return true;
            } else if installed_num < required_num {
                return false;
            }
        }
        
        true
    }

    /// Détecte l'architecture pour le téléchargement de Go
    fn detect_go_architecture(&self, executor: &CommandExecutor) -> Result<(String, String), ExecError> {
        // Détecter l'OS
        let os = if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            return Err(ExecError::Failed("OS non supporté pour l'installation de Go".to_string()));
        };

        // Détecter l'architecture
        let arch_cmd = "uname -m";
        let arch_output = executor.run_shell(arch_cmd, false)?;
        let arch_str = arch_output.stdout.trim().to_lowercase();

        let arch = match arch_str.as_str() {
            "x86_64" | "amd64" => "amd64",
            "aarch64" | "arm64" => "arm64",
            "armv7l" | "armv6l" => "armv6l",
            "ppc64le" => "ppc64le",
            "s390x" => "s390x",
            _ => {
                return Err(ExecError::Failed(format!("Architecture non supportée : {}", arch_str)));
            }
        };

        Ok((os.to_string(), arch.to_string()))
    }
}


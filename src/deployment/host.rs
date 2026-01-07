use crate::pres::executor::{CommandExecutor, CommandOutput, ExecError};
use crate::data::distribution::DistributionInfo;

/// Gestion de l'installation et de la configuration de RMDB sur le système hôte
pub struct HostDeployment {
    distribution: DistributionInfo,
}

impl HostDeployment {
    pub fn new() -> Self {
        Self {
            distribution: DistributionInfo::detect(),
        }
    }

    /// Vérifie si Go est installé
    pub fn check_go_installed(&self, executor: &CommandExecutor) -> bool {
        let cmd = "command -v go >/dev/null 2>&1 && echo 'installed' || echo 'not_installed'";
        if let Ok(output) = executor.run_shell(cmd, false) {
            output.stdout.contains("installed")
        } else {
            false
        }
    }

    /// Installe Go si nécessaire
    pub fn install_go(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        if self.check_go_installed(executor) {
            return Ok(CommandOutput {
                stdout: "Go est déjà installé".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
            });
        }

        let packages = match self.distribution.package_manager {
            crate::data::distribution::PackageManager::Apt => vec!["golang-go"],
            crate::data::distribution::PackageManager::Dnf | crate::data::distribution::PackageManager::Yum => vec!["golang"],
            crate::data::distribution::PackageManager::Pacman => vec!["go"],
            crate::data::distribution::PackageManager::Zypper => vec!["go"],
            crate::data::distribution::PackageManager::Apk => vec!["go"],
        };

        let packages_refs: Vec<&str> = packages.iter().map(|s| *s).collect();
        let install_cmd = self.distribution.install_command(&packages_refs);
        executor.run_shell(&install_cmd, true)
    }

    /// Vérifie si RMDB est installé sur le système hôte
    pub fn check_rmdb_installed(&self, executor: &CommandExecutor) -> bool {
        let cmd = "test -f /usr/local/bin/rmdbd && echo 'installed' || echo 'not_installed'";
        if let Ok(output) = executor.run_shell(cmd, true) {
            output.stdout.contains("installed")
        } else {
            false
        }
    }

    /// Installe RMDB sur le système hôte
    pub fn install_rmdb(&self, executor: &CommandExecutor, rmdb_source_path: &str) -> Result<CommandOutput, ExecError> {
        // Vérifier que le répertoire source existe
        let source_path = std::path::Path::new(rmdb_source_path);
        if !source_path.exists() {
            return Err(ExecError::Failed(format!("Le répertoire source RMDB n'existe pas: {}", rmdb_source_path)));
        }

        // Étape 1: Vérifier/installer Go
        if !self.check_go_installed(executor) {
            self.install_go(executor)?;
        }

        // Étape 2: Copier les fichiers source vers un répertoire temporaire de compilation
        let build_dir = "/tmp/rmdb_build";
        let mkdir_cmd = format!("mkdir -p {}", build_dir);
        executor.run_shell(&mkdir_cmd, true)?;

        let copy_cmd = format!(
            "cd {} && tar -czf - --exclude='.git' --exclude='*.log' --exclude='*.db' --exclude='target' . | tar -xzf - -C {}",
            rmdb_source_path, build_dir
        );
        executor.run_shell(&copy_cmd, true)?;

        // Étape 3: Télécharger les dépendances Go
        let go_mod_cmd = format!("cd {} && go mod download", build_dir);
        executor.run_shell(&go_mod_cmd, false)?;

        // Étape 4: Compiler RMDB
        let build_cmd = format!(
            "cd {}/cmd/rmdbd && CGO_ENABLED=0 go build -trimpath -ldflags \"-s -w\" -o /usr/local/bin/rmdbd . && chmod +x /usr/local/bin/rmdbd",
            build_dir
        );
        let build_result = executor.run_shell(&build_cmd, true)?;

        // Étape 5: Créer la configuration
        let config_cmd = format!(
            "mkdir -p /etc/rmdbd && cp {}/configs/rmdbd.example.json /etc/rmdbd/config.json 2>/dev/null || true",
            build_dir
        );
        executor.run_shell(&config_cmd, true)?;

        // Étape 6: Créer les répertoires de données
        let data_dirs = "/var/lib/rmdb/www /var/lib/rmdb/tftpboot /var/lib/rmdb/images /var/lib/rmdb/vms /var/lib/rmdb/overlays /var/lib/rmdb/backups /var/lib/rmdb/audits /var/lib/rmdb/checksums /var/lib/rmdb/metrics";
        let mkdir_data_cmd = format!("mkdir -p {}", data_dirs);
        executor.run_shell(&mkdir_data_cmd, true)?;

        // Étape 7: Créer le service systemd ou OpenRC
        self.create_service(executor)?;

        // Nettoyer le répertoire temporaire
        let _ = executor.run_shell(&format!("rm -rf {}", build_dir), true);

        Ok(build_result)
    }

    /// Crée le service systemd ou OpenRC pour RMDB
    fn create_service(&self, executor: &CommandExecutor) -> Result<(), ExecError> {
        // Vérifier si systemd est disponible
        let has_systemd = executor.run_shell("command -v systemctl >/dev/null 2>&1 && echo 'yes' || echo 'no'", false)
            .map(|o| o.stdout.contains("yes"))
            .unwrap_or(false);

        if has_systemd {
            // Créer le service systemd
            let service_content = r#"[Unit]
Description=RMDB Server
After=network.target

[Service]
Type=simple
User=root
ExecStart=/usr/local/bin/rmdbd -config /etc/rmdbd/config.json
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
"#;

            let service_file = "/etc/systemd/system/rmdbd.service";
            let write_cmd = format!("cat > {} << 'SERVICEEOF'\n{}\nSERVICEEOF", service_file, service_content);
            executor.run_shell(&write_cmd, true)?;

            // Recharger systemd
            executor.run_shell("systemctl daemon-reload", true)?;
        } else {
            // Créer le service OpenRC
            let service_content = r#"#!/sbin/openrc-run
command="/usr/local/bin/rmdbd"
command_args="-config /etc/rmdbd/config.json"
pidfile="/var/run/rmdbd.pid"
command_background=true

depend() {
    need net
    after firewall
}
"#;

            let service_file = "/etc/init.d/rmdbd";
            let write_cmd = format!("cat > {} << 'SERVICEEOF'\n{}\nSERVICEEOF && chmod +x {}", service_file, service_content, service_file);
            executor.run_shell(&write_cmd, true)?;
        }

        Ok(())
    }

    /// Démarre le service RMDB
    pub fn start_rmdb(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        let has_systemd = executor.run_shell("command -v systemctl >/dev/null 2>&1 && echo 'yes' || echo 'no'", false)
            .map(|o| o.stdout.contains("yes"))
            .unwrap_or(false);

        if has_systemd {
            executor.run_shell("systemctl start rmdbd", true)
        } else {
            executor.run_shell("rc-service rmdbd start", true)
        }
    }

    /// Arrête le service RMDB
    pub fn stop_rmdb(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        let has_systemd = executor.run_shell("command -v systemctl >/dev/null 2>&1 && echo 'yes' || echo 'no'", false)
            .map(|o| o.stdout.contains("yes"))
            .unwrap_or(false);

        if has_systemd {
            executor.run_shell("systemctl stop rmdbd", true)
        } else {
            executor.run_shell("rc-service rmdbd stop", true)
        }
    }

    /// Redémarre le service RMDB
    pub fn restart_rmdb(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        let has_systemd = executor.run_shell("command -v systemctl >/dev/null 2>&1 && echo 'yes' || echo 'no'", false)
            .map(|o| o.stdout.contains("yes"))
            .unwrap_or(false);

        if has_systemd {
            executor.run_shell("systemctl restart rmdbd", true)
        } else {
            executor.run_shell("rc-service rmdbd restart", true)
        }
    }

    /// Obtient le statut du service RMDB
    pub fn get_rmdb_status(&self, executor: &CommandExecutor) -> Result<String, ExecError> {
        let has_systemd = executor.run_shell("command -v systemctl >/dev/null 2>&1 && echo 'yes' || echo 'no'", false)
            .map(|o| o.stdout.contains("yes"))
            .unwrap_or(false);

        if has_systemd {
            let cmd = "systemctl is-active rmdbd 2>/dev/null || echo 'inactive'";
            let output = executor.run_shell(cmd, true)?;
            Ok(output.stdout.trim().to_string())
        } else {
            let cmd = "rc-service rmdbd status 2>/dev/null | grep -q 'started' && echo 'active' || echo 'inactive'";
            let output = executor.run_shell(cmd, true)?;
            Ok(output.stdout.trim().to_string())
        }
    }

    /// Active le service RMDB au démarrage
    pub fn enable_rmdb(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        let has_systemd = executor.run_shell("command -v systemctl >/dev/null 2>&1 && echo 'yes' || echo 'no'", false)
            .map(|o| o.stdout.contains("yes"))
            .unwrap_or(false);

        if has_systemd {
            executor.run_shell("systemctl enable rmdbd", true)
        } else {
            executor.run_shell("rc-update add rmdbd default", true)
        }
    }

    /// Désactive le service RMDB au démarrage
    pub fn disable_rmdb(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        let has_systemd = executor.run_shell("command -v systemctl >/dev/null 2>&1 && echo 'yes' || echo 'no'", false)
            .map(|o| o.stdout.contains("yes"))
            .unwrap_or(false);

        if has_systemd {
            executor.run_shell("systemctl disable rmdbd", true)
        } else {
            executor.run_shell("rc-update del rmdbd default", true)
        }
    }

    /// Désinstalle RMDB du système hôte
    pub fn uninstall_rmdb(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        // Arrêter le service
        let _ = self.stop_rmdb(executor);
        let _ = self.disable_rmdb(executor);

        // Supprimer le binaire
        let rm_binary = "rm -f /usr/local/bin/rmdbd";
        executor.run_shell(rm_binary, true)?;

        // Supprimer le service
        let has_systemd = executor.run_shell("command -v systemctl >/dev/null 2>&1 && echo 'yes' || echo 'no'", false)
            .map(|o| o.stdout.contains("yes"))
            .unwrap_or(false);

        if has_systemd {
            executor.run_shell("rm -f /etc/systemd/system/rmdbd.service && systemctl daemon-reload", true)?;
        } else {
            executor.run_shell("rm -f /etc/init.d/rmdbd", true)?;
        }

        Ok(CommandOutput {
            stdout: "RMDB désinstallé".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }
}


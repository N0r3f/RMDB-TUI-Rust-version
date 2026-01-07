use crate::pres::executor::{CommandExecutor, CommandOutput, ExecError};
use crate::deployment::logger::DeploymentLogger;
use crate::data::distribution::DistributionInfo;
use std::process::Command;
use std::fs;
use std::path::Path;

pub struct LXCDeployment {
    container_name: String,
    alpine_version: String,
    pub logger: Option<DeploymentLogger>,
    distribution: DistributionInfo,
}

impl LXCDeployment {
    pub fn new(container_name: String, alpine_version: String) -> Self {
        Self {
            container_name,
            alpine_version,
            logger: None,
            distribution: DistributionInfo::detect(),
        }
    }

    pub fn with_logger(mut self, logger: DeploymentLogger) -> Self {
        self.logger = Some(logger);
        self
    }

    fn log_info(&self, message: &str) {
        if let Some(ref logger) = self.logger {
            logger.info(message);
        }
    }

    fn log_warn(&self, message: &str) {
        if let Some(ref logger) = self.logger {
            logger.warn(message);
        }
    }

    fn log_error(&self, message: &str) {
        if let Some(ref logger) = self.logger {
            logger.error(message);
        }
    }

    #[allow(dead_code)]
    fn log_debug(&self, message: &str) {
        if let Some(ref logger) = self.logger {
            logger.debug(message);
        }
    }

    fn log_command(&self, cmd: &str) {
        if let Some(ref logger) = self.logger {
            logger.command(cmd);
        }
    }

    fn log_command_output(&self, output: &CommandOutput) {
        if let Some(ref logger) = self.logger {
            logger.command_output(&output.stdout, &output.stderr, output.exit_code);
        }
    }

    pub fn check_lxc_installed(&self) -> bool {
        Command::new("sh")
            .args(["-lc", "command -v lxc-create >/dev/null 2>&1"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn check_lxc_templates(&self) -> bool {
        // Vérifier que les templates sont disponibles
        // Les templates peuvent être dans /usr/share/lxc/templates ou /usr/lib/lxc/templates
        let template_paths = vec![
            "/usr/share/lxc/templates/lxc-alpine",
            "/usr/lib/lxc/templates/lxc-alpine",
            "/usr/lib64/lxc/templates/lxc-alpine",
            "/usr/libexec/lxc/templates/lxc-alpine",  // RHEL/CentOS
        ];
        
        // Vérifier d'abord si les fichiers existent
        if template_paths.iter().any(|path| {
            std::path::Path::new(path).exists()
        }) {
            return true;
        }
        
        // Vérifier via lxc-create si le template alpine est disponible
        // Cette méthode fonctionne même si les fichiers ne sont pas aux emplacements standards
        // Tester directement si lxc-create peut lister le template alpine
        let check_cmd = "lxc-create -t alpine --help 2>&1 | head -1 | grep -q -i alpine || lxc-create --help 2>&1 | grep -q alpine || find /usr -name 'lxc-alpine' 2>/dev/null | head -1 | grep -q alpine || test -f /usr/share/lxc/templates/lxc-alpine || test -f /usr/lib/lxc/templates/lxc-alpine || test -f /usr/lib64/lxc/templates/lxc-alpine || test -f /usr/libexec/lxc/templates/lxc-alpine";
        
        Command::new("sh")
            .args(["-lc", check_cmd])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn check_container_exists(&self) -> bool {
        // Essayer plusieurs méthodes pour vérifier l'existence du container
        // 1. lxc-ls (LXC 1.x)
        let output1 = Command::new("lxc-ls")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok());
        
        if let Some(ref s) = output1 {
            if s.lines().any(|line| line.trim() == self.container_name) {
                return true;
            }
        }
        
        // 2. lxc-ls --fancy (LXC 2.x+)
        let output2 = Command::new("lxc-ls")
            .args(["--fancy"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok());
        
        if let Some(ref s) = output2 {
            if s.lines().any(|line| line.contains(&self.container_name)) {
                return true;
            }
        }
        
        // 3. lxc list (LXC 3.x+)
        let output3 = Command::new("lxc")
            .args(["list", "--format", "csv", "-c", "n"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok());
        
        if let Some(ref s) = output3 {
            if s.lines().any(|line| line.trim() == self.container_name) {
                return true;
            }
        }
        
        // 4. Vérifier directement si le répertoire du container existe
        let container_paths = vec![
            format!("/var/lib/lxc/{}", self.container_name),
            format!("{}/.local/share/lxc/{}", std::env::var("HOME").unwrap_or_default(), self.container_name),
        ];
        
        container_paths.iter().any(|path| std::path::Path::new(path).exists())
    }

    pub fn create_container(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        self.log_info(&format!("Début de la création du container '{}'", self.container_name));

        if !self.check_lxc_installed() {
            let packages = vec!["lxc", "lxc-templates"];
            let install_cmd = self.distribution.install_command(&packages.iter().map(|s| *s).collect::<Vec<_>>());
            let msg = format!("LXC n'est pas installé. Installez-le avec: {}", install_cmd);
            self.log_error(&msg);
            return Err(ExecError::MissingTool(msg));
        }
        self.log_info("LXC est installé");

        // Ne pas bloquer si la détection échoue - les templates peuvent être installés
        // mais à un emplacement non standard (notamment sur RHEL)
        if !self.check_lxc_templates() {
            self.log_warn("Les templates LXC n'ont pas été détectés, mais on continue quand même");
        } else {
            self.log_info("Templates LXC détectés");
        }

        // Vérifier l'existence avec sudo (comme la création)
        if self.check_container_exists_with_executor(executor) {
            let msg = format!("Le container {} existe déjà", self.container_name);
            self.log_error(&msg);
            return Err(ExecError::Failed(msg));
        }
        self.log_info("Le container n'existe pas encore, on peut le créer");

        // Sur RHEL/CentOS, créer la configuration LXC par défaut si nécessaire
        if self.distribution.needs_root_for_lxc() {
            self.setup_lxc_config_for_rhel(executor)?;
        }

        // Essayer différentes syntaxes selon la version de LXC
        // LXC 1.x utilise: lxc-create -n name -t template -- --release version
        // LXC 2.x+ peut nécessiter une syntaxe différente ou utiliser download
        // Essayer d'abord avec le template alpine, puis avec download si échec
        let cmd1 = format!(
            "lxc-create -n {} -t alpine -- --release v{}",
            self.container_name, self.alpine_version
        );
        
        self.log_command(&cmd1);
        let result1 = executor.run_shell(&cmd1, true);
        
        // Si la première commande échoue, essayer avec download (LXC 2.x+)
        let final_result = if result1.is_err() || (result1.is_ok() && result1.as_ref().unwrap().exit_code != Some(0)) {
            self.log_warn("La première méthode de création a échoué, essai avec 'download'");
            let cmd2 = format!(
                "lxc-create -n {} -t download -- --dist alpine --release {} --arch amd64",
                self.container_name, self.alpine_version
            );
            self.log_command(&cmd2);
            let result2 = executor.run_shell(&cmd2, true);
            
            if let Ok(ref output) = result2 {
                self.log_command_output(output);
                if output.exit_code == Some(0) {
                    self.log_info("Container créé avec succès (méthode download)");
                } else {
                    self.log_error("Échec de la création du container (méthode download)");
                    self.log_error(&format!("stderr: {}", output.stderr));
                }
            }
            result2
        } else {
            if let Ok(ref output) = result1 {
                self.log_command_output(output);
                if output.exit_code == Some(0) {
                    self.log_info("Container créé avec succès (méthode alpine)");
                } else {
                    self.log_error(&format!("stderr: {}", output.stderr));
                }
            }
            result1
        };

        // Vérifier RÉELLEMENT que le container existe après création
        if let Ok(ref output) = final_result {
            if output.exit_code == Some(0) {
                self.log_info("Vérification de l'existence réelle du container...");
                std::thread::sleep(std::time::Duration::from_millis(500));
                
                // Essayer plusieurs fois avec des délais croissants
                let mut verified = false;
                for attempt in 1..=5 {
                    if self.check_container_exists_with_executor(executor) {
                        verified = true;
                        self.log_info(&format!("Container vérifié et existant (tentative {})", attempt));
                        break;
                    }
                    if attempt < 5 {
                        std::thread::sleep(std::time::Duration::from_millis(500 * attempt as u64));
                    }
                }
                
                if !verified {
                    self.log_error("ATTENTION: Le container semble créé mais n'est pas détectable!");
                    self.log_error("Cela peut indiquer un problème de permissions ou de configuration LXC");
                    // Ne pas échouer, mais logger l'avertissement
                }
            }
        }

        final_result
    }

    /// Vérifie l'existence du container en utilisant l'executor (avec sudo si nécessaire)
    pub fn check_container_exists_with_executor(&self, executor: &CommandExecutor) -> bool {
        // PRIORITÉ: Vérifier d'abord le système de fichiers (source de vérité)
        // Si le répertoire n'existe pas, le container n'existe pas vraiment
        let container_paths = vec![
            format!("/var/lib/lxc/{}", self.container_name),
            format!("{}/.local/share/lxc/{}", std::env::var("HOME").unwrap_or_default(), self.container_name),
            format!("/var/lib/lxd/containers/{}", self.container_name),
        ];
        
        let mut filesystem_exists = false;
        for path in &container_paths {
            let cmd_check = format!("test -d {} && echo 'found' || echo 'not found'", path);
            if let Ok(output) = executor.run_shell(&cmd_check, true) {
                if output.stdout.contains("found") {
                    filesystem_exists = true;
                    break;
                }
            }
        }
        
        // Si le répertoire n'existe pas dans le système de fichiers, le container n'existe pas
        if !filesystem_exists {
            return false;
        }
        
        // Si le répertoire existe, vérifier qu'il s'agit bien d'un container LXC valide
        // en vérifiant la présence d'un fichier config ou d'un rootfs
        let mut has_valid_structure = false;
        for path in &container_paths {
            if filesystem_exists {
                // Vérifier la présence d'un fichier config ou d'un répertoire rootfs
                let config_check = format!("test -f {}/config && echo 'yes' || echo 'no'", path);
                let rootfs_check = format!("test -d {}/rootfs && echo 'yes' || echo 'no'", path);
                
                if let (Ok(config_out), Ok(rootfs_out)) = (
                    executor.run_shell(&config_check, true),
                    executor.run_shell(&rootfs_check, true)
                ) {
                    if config_out.stdout.contains("yes") || rootfs_out.stdout.contains("yes") {
                        has_valid_structure = true;
                        break;
                    }
                }
            }
        }
        
        // Si le répertoire existe mais n'a pas la structure d'un container LXC valide,
        // ce n'est pas un container géré par LXC
        if !has_valid_structure {
            return false;
        }
        
        // Maintenant vérifier via les commandes LXC pour confirmer que LXC le gère
        // 1. Essayer lxc-ls avec sudo
        let cmd_ls = format!("sudo -n lxc-ls -1 2>/dev/null | grep -q '^{}$' && echo 'found' || echo 'not found'", self.container_name);
        if let Ok(output) = executor.run_shell(&cmd_ls, true) {
            if output.stdout.contains("found") {
                return true;
            }
        }
        
        // 2. Essayer lxc-ls sans sudo (au cas où)
        let cmd_ls_nosudo = format!("lxc-ls -1 2>/dev/null | grep -q '^{}$' && echo 'found' || echo 'not found'", self.container_name);
        if let Ok(output) = executor.run_shell(&cmd_ls_nosudo, false) {
            if output.stdout.contains("found") {
                return true;
            }
        }
        
        // 3. Essayer lxc list
        let cmd_list = format!("lxc list --format csv -c n 2>/dev/null | grep -q '^{}$' && echo 'found' || echo 'not found'", self.container_name);
        if let Ok(output) = executor.run_shell(&cmd_list, false) {
            if output.stdout.contains("found") {
                return true;
            }
        }
        
        // Si le répertoire a une structure valide mais n'est pas détecté par les commandes LXC,
        // c'est probablement un container corrompu ou incomplet - on le considère comme existant
        // mais la création échouera de toute façon, donc on retourne true pour éviter les doublons
        has_valid_structure
    }
    
    /// Vérifie strictement que le container n'existe plus (vérification du système de fichiers uniquement)
    pub fn check_container_fully_removed(executor: &CommandExecutor, name: &str) -> bool {
        let container_paths = vec![
            format!("/var/lib/lxc/{}", name),
            format!("{}/.local/share/lxc/{}", std::env::var("HOME").unwrap_or_default(), name),
            format!("/var/lib/lxd/containers/{}", name),
        ];
        
        for path in &container_paths {
            let cmd_check = format!("test -d {} && echo 'exists' || echo 'not exists'", path);
            if let Ok(output) = executor.run_shell(&cmd_check, true) {
                if output.stdout.contains("exists") {
                    return false; // Le répertoire existe encore
                }
            }
        }
        
        true // Aucun répertoire trouvé, le container est vraiment supprimé
    }
    
    /// Nettoie les entrées fantômes d'un container (détecté par lxc-ls mais n'existant pas dans le système de fichiers)
    pub fn cleanup_ghost_container(executor: &CommandExecutor, name: &str) -> Result<CommandOutput, ExecError> {
        // Essayer de nettoyer les caches LXC
        let cleanup_cmds = vec![
            format!("sudo -n lxc-ls -1 2>/dev/null | grep -q '^{}$' || true", name), // Vérifier si détecté
            format!("sudo -n rm -f /var/lib/lxc/.lxc-lock-{} 2>/dev/null || true", name), // Supprimer les locks
            format!("sudo -n rm -f /var/lib/lxc/{}/.lxc-lock 2>/dev/null || true", name), // Supprimer les locks dans le répertoire
        ];
        
        for cmd in &cleanup_cmds {
            let _ = executor.run_shell(cmd, true);
        }
        
        // Retourner un succès (même si certaines commandes échouent)
        Ok(CommandOutput {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }

    /// Trouve le chemin du fichier de configuration du container
    fn find_container_config_path(&self, executor: &CommandExecutor) -> Option<String> {
        let possible_paths = vec![
            format!("/var/lib/lxc/{}/config", self.container_name),
            format!("/var/lib/lxd/containers/{}/config", self.container_name),
            format!("{}/.local/share/lxc/{}/config", std::env::var("HOME").unwrap_or_default(), self.container_name),
        ];
        
        for path in &possible_paths {
            let cmd_check = format!("test -f {} && echo 'found' || echo 'not found'", path);
            if let Ok(output) = executor.run_shell(&cmd_check, true) {
                if output.stdout.contains("found") {
                    return Some(path.clone());
                }
            }
        }
        
        None
    }

    pub fn start_container(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        self.log_info(&format!("Démarrage du container '{}'", self.container_name));
        
        // Trouver le chemin du fichier de configuration
        let config_path = self.find_container_config_path(executor);
        let cmd = if let Some(config) = &config_path {
            // Utiliser -f pour spécifier explicitement le fichier de configuration
            format!("lxc-start -f {} -n {}", config, self.container_name)
        } else {
            // Utiliser -P pour spécifier le répertoire racine des containers
            format!("lxc-start -P /var/lib/lxc -n {}", self.container_name)
        };
        
        self.log_command(&cmd);
        let mut result = executor.run_shell(&cmd, true);
        
        // Si échec et qu'on n'a pas de config, vérifier que le fichier config existe vraiment
        if let Ok(ref output) = result {
            if output.exit_code != Some(0) && config_path.is_none() {
                // Vérifier si le fichier config existe vraiment
                let config_check = format!("test -f /var/lib/lxc/{}/config && echo 'exists' || echo 'missing'", self.container_name);
                if let Ok(check_output) = executor.run_shell(&config_check, true) {
                    if check_output.stdout.contains("exists") {
                        // Le fichier existe, utiliser -f explicitement
                        let cmd_alt = format!("lxc-start -f /var/lib/lxc/{}/config -n {}", self.container_name, self.container_name);
                        self.log_info("Tentative avec fichier de configuration explicite");
                        result = executor.run_shell(&cmd_alt, true);
                    } else {
                        self.log_error(&format!("Le fichier de configuration /var/lib/lxc/{}/config n'existe pas", self.container_name));
                    }
                }
            }
        }
        
        if let Ok(ref output) = result {
            self.log_command_output(output);
            if output.exit_code == Some(0) {
                self.log_info("Container démarré avec succès");
            } else {
                self.log_error("Échec du démarrage du container");
            }
        }
        result
    }

    /// Vérifie complètement que le container existe, est accessible et fonctionne
    pub fn verify_container(&self, executor: &CommandExecutor) -> Result<ContainerVerification, ExecError> {
        let mut verification = ContainerVerification {
            exists: false,
            detectable_by_ls: false,
            detectable_by_list: false,
            detectable_by_filesystem: false,
            can_get_status: false,
            can_attach: false,
            is_running: false,
            errors: Vec::new(),
        };

        // 1. Vérifier l'existence via check_container_exists_with_executor
        verification.exists = self.check_container_exists_with_executor(executor);
        if !verification.exists {
            verification.errors.push("Le container n'existe pas selon check_container_exists_with_executor()".to_string());
        }

        // 2. Vérifier via lxc-ls
        let cmd_ls = format!("lxc-ls -1 2>/dev/null | grep -q '^{}$' && echo 'found' || echo 'not found'", self.container_name);
        if let Ok(output) = executor.run_shell(&cmd_ls, false) {
            if output.stdout.contains("found") {
                verification.detectable_by_ls = true;
            } else {
                verification.errors.push("Container non détecté par lxc-ls".to_string());
            }
        }

        // Essayer aussi avec sudo
        if !verification.detectable_by_ls {
            let cmd_ls_sudo = format!("sudo -n lxc-ls -1 2>/dev/null | grep -q '^{}$' && echo 'found' || echo 'not found'", self.container_name);
            if let Ok(output) = executor.run_shell(&cmd_ls_sudo, true) {
                if output.stdout.contains("found") {
                    verification.detectable_by_ls = true;
                }
            }
        }

        // 3. Vérifier via lxc list
        let cmd_list = format!("lxc list {} --format csv -c n 2>/dev/null | grep -q '^{}$' && echo 'found' || echo 'not found'", self.container_name, self.container_name);
        if let Ok(output) = executor.run_shell(&cmd_list, false) {
            if output.stdout.contains("found") {
                verification.detectable_by_list = true;
            } else {
                verification.errors.push("Container non détecté par lxc list".to_string());
            }
        }

        // 4. Vérifier via le système de fichiers
        let container_paths = vec![
            format!("/var/lib/lxc/{}", self.container_name),
            format!("{}/.local/share/lxc/{}", std::env::var("HOME").unwrap_or_default(), self.container_name),
        ];
        for path in &container_paths {
            if std::path::Path::new(path).exists() {
                verification.detectable_by_filesystem = true;
                break;
            }
        }
        if !verification.detectable_by_filesystem {
            verification.errors.push("Container non trouvé dans le système de fichiers".to_string());
        }

        // 5. Vérifier qu'on peut obtenir le statut
        match self.get_container_status(executor) {
            Ok(status) => {
                verification.can_get_status = true;
                verification.is_running = status == "RUNNING";
                if !verification.is_running {
                    verification.errors.push(format!("Container existe mais n'est pas en cours d'exécution (statut: {})", status));
                }
            }
            Err(e) => {
                verification.errors.push(format!("Impossible d'obtenir le statut: {}", e));
            }
        }

        // 6. Vérifier qu'on peut accéder au container via lxc-attach
        let cmd_attach = format!("lxc-attach -n {} -- echo 'test' 2>&1", self.container_name);
        match executor.run_shell(&cmd_attach, true) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    verification.can_attach = true;
                } else {
                    verification.errors.push(format!("lxc-attach échoue avec code: {:?}, stderr: {}", output.exit_code, output.stderr));
                }
            }
            Err(e) => {
                verification.errors.push(format!("Erreur lors de l'accès via lxc-attach: {}", e));
            }
        }

        Ok(verification)
    }

    pub fn stop_container(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        // Trouver le chemin du fichier de configuration
        let config_path = self.find_container_config_path(executor);
        let cmd = if let Some(config) = &config_path {
            format!("lxc-stop -f {} -n {}", config, self.container_name)
        } else {
            // Utiliser -P pour spécifier le répertoire racine des containers
            format!("lxc-stop -P /var/lib/lxc -n {}", self.container_name)
        };
        
        let mut result = executor.run_shell(&cmd, true);
        
        // Si échec et qu'on n'a pas de config, vérifier que le fichier config existe vraiment
        if let Ok(ref output) = result {
            if output.exit_code != Some(0) && config_path.is_none() {
                // Vérifier si le fichier config existe vraiment
                let config_check = format!("test -f /var/lib/lxc/{}/config && echo 'exists' || echo 'missing'", self.container_name);
                if let Ok(check_output) = executor.run_shell(&config_check, true) {
                    if check_output.stdout.contains("exists") {
                        // Le fichier existe, utiliser -f explicitement
                        let cmd_alt = format!("lxc-stop -f /var/lib/lxc/{}/config -n {}", self.container_name, self.container_name);
                        result = executor.run_shell(&cmd_alt, true);
                    }
                }
            }
        }
        
        result
    }

    pub fn destroy_container(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        let cmd = format!("lxc-destroy -n {}", self.container_name);
        executor.run_shell(&cmd, true)
    }

    // Fonctions de gestion générale des containers (tous les containers, pas seulement rmdb)
    
    /// Liste tous les containers LXC disponibles
    pub fn list_all_containers(executor: &CommandExecutor) -> Result<Vec<ContainerInfo>, ExecError> {
        let mut containers = Vec::new();
        let mut found_names = std::collections::HashSet::new();
        
        // Essayer plusieurs méthodes selon la version de LXC et combiner les résultats
        // IMPORTANT: Toujours essayer avec sudo car sur RHEL/CentOS, les containers
        // créés avec sudo ne sont visibles qu'avec sudo
        
        // 1. lxc-ls avec sudo (PRIORITAIRE - car création se fait avec sudo)
        let cmd1_sudo = "sudo -n lxc-ls -1 2>&1";
        if let Ok(output) = executor.run_shell(cmd1_sudo, true) {
            // Filtrer les lignes qui sont des erreurs (commencent par "sudo:" ou contiennent "error")
            for line in output.stdout.lines() {
                let line = line.trim();
                // Ignorer les messages d'erreur et les lignes vides
                // Un nom de container valide : alphanumérique, tirets, underscores, pas d'espaces, pas de caractères spéciaux
                let is_valid_name = line.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                    && !line.starts_with('-')
                    && !line.ends_with('-')
                    && !line.contains(' ')
                    && !line.contains(':');
                
                if !line.is_empty() 
                    && !line.starts_with("sudo:") 
                    && !line.to_lowercase().contains("error")
                    && !line.to_lowercase().contains("permission denied")
                    && !line.to_lowercase().contains("bash:")
                    && !line.to_lowercase().contains("commande")
                    && !line.to_lowercase().contains("inconnue")
                    && !line.to_lowercase().contains("command not found")
                    && line.len() < 50
                    && line.len() > 0
                    && is_valid_name
                    && !found_names.contains(line) {
                    found_names.insert(line.to_string());
                    let status = Self::get_container_status_by_name(executor, line).unwrap_or_else(|_| "UNKNOWN".to_string());
                    containers.push(ContainerInfo {
                        name: line.to_string(),
                        status,
                    });
                }
            }
        }
        
        // 2. lxc-ls sans sudo (au cas où certains containers sont accessibles sans sudo)
        let cmd1 = "lxc-ls -1 2>&1";
        if let Ok(output) = executor.run_shell(cmd1, false) {
            for line in output.stdout.lines() {
                let line = line.trim();
                if !line.is_empty() 
                    && !line.starts_with("sudo:") 
                    && !line.to_lowercase().contains("error")
                    && !line.to_lowercase().contains("permission denied")
                    && !line.to_lowercase().contains("bash:")
                    && !line.to_lowercase().contains("commande")
                    && !line.to_lowercase().contains("inconnue")
                    && !line.to_lowercase().contains("command not found")
                    && line.len() < 50
                    && !found_names.contains(line) {
                    found_names.insert(line.to_string());
                    let status = Self::get_container_status_by_name(executor, line).unwrap_or_else(|_| "UNKNOWN".to_string());
                    containers.push(ContainerInfo {
                        name: line.to_string(),
                        status,
                    });
                }
            }
        }
        
        // 3. lxc list (LXC 2.x+/LXD) - essaie toujours, même si on a déjà trouvé des containers
        // Vérifier d'abord si la commande existe pour éviter les messages d'erreur
        let cmd_check_lxc = "command -v lxc >/dev/null 2>&1 && echo 'exists' || echo 'not found'";
        let has_lxc_cmd = executor.run_shell(cmd_check_lxc, false)
            .map(|o| o.stdout.contains("exists"))
            .unwrap_or(false);
        
        if has_lxc_cmd {
            let cmd2 = "lxc list --format csv -c n,s 2>/dev/null";
            if let Ok(output) = executor.run_shell(cmd2, false) {
                // Vérifier que la sortie ne contient pas d'erreurs dans stderr
                // (on a redirigé stderr vers /dev/null, donc si stdout contient des erreurs, c'est suspect)
                let stdout_lower = output.stdout.to_lowercase();
                if !stdout_lower.contains("error") 
                    && !stdout_lower.contains("bash:")
                    && !stdout_lower.contains("commande")
                    && !stdout_lower.contains("inconnue")
                    && !stdout_lower.contains("command not found") {
                    
                    for line in output.stdout.lines() {
                        let line = line.trim();
                        // Ignorer les lignes vides
                        if line.is_empty() {
                            continue;
                        }
                        
                        let parts: Vec<&str> = line.split(',').collect();
                        if parts.len() >= 2 {
                            let name = parts[0].trim().to_string();
                            let status = parts[1].trim().to_string();
                            // Valider que le nom ressemble à un nom de container (pas un message d'erreur)
                            // Un nom de container valide : alphanumérique, tirets, underscores, pas d'espaces, pas de caractères spéciaux
                            let is_valid_name = name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                                && !name.starts_with('-')
                                && !name.ends_with('-');
                            
                            if !name.is_empty() 
                                && name != "NAME" 
                                && !name.to_lowercase().contains("error")
                                && !name.to_lowercase().contains("bash:")
                                && !name.contains("commande")
                                && !name.contains("inconnue")
                                && !name.contains(":")
                                && !name.contains(" ")
                                && name.len() < 50
                                && name.len() > 0
                                && is_valid_name
                                && !found_names.contains(&name) {
                                found_names.insert(name.clone());
                                containers.push(ContainerInfo {
                                    name,
                                    status: status.to_uppercase(),
                                });
                            }
                        } else if parts.len() == 1 {
                            // Format simple avec juste le nom
                            let name = parts[0].trim().to_string();
                            // Valider que c'est un vrai nom de container
                            let is_valid_name = name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                                && !name.starts_with('-')
                                && !name.ends_with('-');
                            
                            if !name.is_empty() 
                                && name != "NAME"
                                && !name.to_lowercase().contains("error")
                                && !name.to_lowercase().contains("bash:")
                                && !name.contains("commande")
                                && !name.contains("inconnue")
                                && !name.contains(":")
                                && !name.contains(" ")
                                && name.len() < 50
                                && name.len() > 0
                                && is_valid_name
                                && !found_names.contains(&name) {
                                found_names.insert(name.clone());
                                let status = Self::get_container_status_by_name(executor, &name).unwrap_or_else(|_| "UNKNOWN".to_string());
                                containers.push(ContainerInfo {
                                    name,
                                    status,
                                });
                            }
                        }
                    }
                }
            }
        }
        
        // 4. Vérifier directement les répertoires avec sudo (IMPORTANT pour RHEL)
        let paths = vec!["/var/lib/lxc", "/var/lib/lxd/containers"];
        for base_path in paths {
            // Utiliser sudo pour lister les répertoires
            let cmd_ls = format!("sudo -n ls -1 {} 2>&1", base_path);
            if let Ok(output) = executor.run_shell(&cmd_ls, true) {
                for line in output.stdout.lines() {
                    let name = line.trim().to_string();
                    // Ignorer les messages d'erreur et les fichiers cachés
                    // Validation stricte du nom
                    let is_valid_name = name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                        && !name.starts_with('-')
                        && !name.ends_with('-')
                        && !name.contains(' ')
                        && !name.contains(':');
                    
                    if !name.is_empty() 
                        && !name.starts_with('.') 
                        && !name.starts_with("sudo:") 
                        && !name.to_lowercase().contains("error")
                        && !name.to_lowercase().contains("permission denied")
                        && !name.to_lowercase().contains("bash:")
                        && !name.to_lowercase().contains("commande")
                        && !name.to_lowercase().contains("inconnue")
                        && !name.to_lowercase().contains("command not found")
                        && name.len() < 50
                        && name.len() > 0
                        && is_valid_name
                        && !found_names.contains(&name) {
                        // Vérifier que c'est bien un container (présence d'un fichier config ou rootfs)
                        let container_path = format!("{}/{}", base_path, name);
                        let config_check = format!("sudo -n test -f {}/config && echo 'yes' || echo 'no'", container_path);
                        let rootfs_check = format!("sudo -n test -d {}/rootfs && echo 'yes' || echo 'no'", container_path);
                        
                        let is_container = if let Ok(config_out) = executor.run_shell(&config_check, true) {
                            config_out.stdout.contains("yes")
                        } else {
                            false
                        } || if let Ok(rootfs_out) = executor.run_shell(&rootfs_check, true) {
                            rootfs_out.stdout.contains("yes")
                        } else {
                            false
                        };
                        
                        if is_container {
                            found_names.insert(name.clone());
                            let status = Self::get_container_status_by_name(executor, &name).unwrap_or_else(|_| "UNKNOWN".to_string());
                            containers.push(ContainerInfo {
                                name,
                                status,
                            });
                        }
                    }
                }
            }
            
            // Essayer aussi sans sudo (pour les containers utilisateur)
            if let Ok(entries) = std::fs::read_dir(base_path) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') && !found_names.contains(&name) {
                        let container_path = std::path::Path::new(base_path).join(&name);
                        let config_path = container_path.join("config");
                        let rootfs_path = container_path.join("rootfs");
                        if config_path.exists() || rootfs_path.exists() {
                            found_names.insert(name.clone());
                            let status = Self::get_container_status_by_name(executor, &name).unwrap_or_else(|_| "UNKNOWN".to_string());
                            containers.push(ContainerInfo {
                                name,
                                status,
                            });
                        }
                    }
                }
            }
        }
        
        // Trier par nom pour un affichage cohérent
        containers.sort_by(|a, b| a.name.cmp(&b.name));
        
        Ok(containers)
    }
    
    /// Obtient le statut d'un container par son nom
    fn get_container_status_by_name(executor: &CommandExecutor, name: &str) -> Result<String, ExecError> {
        // Essayer lxc-info avec sudo (prioritaire car création se fait avec sudo)
        let cmd1_sudo = format!("sudo -n lxc-info -n {} -s 2>/dev/null | grep 'State:' | awk '{{print $2}}' || echo 'UNKNOWN'", name);
        if let Ok(output) = executor.run_shell(&cmd1_sudo, true) {
            let status = output.stdout.trim().to_string();
            if status != "UNKNOWN" && !status.is_empty() && !status.to_lowercase().contains("error") {
                return Ok(status);
            }
        }
        
        // Essayer lxc-info sans sudo
        let cmd1 = format!("lxc-info -n {} -s 2>/dev/null | grep 'State:' | awk '{{print $2}}' || echo 'UNKNOWN'", name);
        if let Ok(output) = executor.run_shell(&cmd1, false) {
            let status = output.stdout.trim().to_string();
            if status != "UNKNOWN" && !status.is_empty() && !status.to_lowercase().contains("error") {
                return Ok(status);
            }
        }
        
        // Essayer lxc list
        let cmd2 = format!("lxc list {} --format csv -c s 2>/dev/null | head -1 || echo 'UNKNOWN'", name);
        if let Ok(output) = executor.run_shell(&cmd2, false) {
            let status = output.stdout.trim().to_string();
            if status != "UNKNOWN" && !status.is_empty() && !status.to_lowercase().contains("error") {
                return Ok(status.to_uppercase());
            }
        }
        
        // Essayer lxc-attach avec sudo
        let cmd3_sudo = format!("sudo -n lxc-attach -n {} -- echo 'running' 2>/dev/null && echo 'RUNNING' || echo 'STOPPED'", name);
        if let Ok(output) = executor.run_shell(&cmd3_sudo, true) {
            if output.stdout.contains("RUNNING") {
                return Ok("RUNNING".to_string());
            }
            if output.stdout.contains("STOPPED") {
                return Ok("STOPPED".to_string());
            }
        }
        
        // Essayer lxc-attach sans sudo
        let cmd3 = format!("lxc-attach -n {} -- echo 'running' 2>/dev/null && echo 'RUNNING' || echo 'STOPPED'", name);
        if let Ok(output) = executor.run_shell(&cmd3, false) {
            if output.stdout.contains("RUNNING") {
                return Ok("RUNNING".to_string());
            }
            if output.stdout.contains("STOPPED") {
                return Ok("STOPPED".to_string());
            }
        }
        
        Ok("UNKNOWN".to_string())
    }
    
    /// Démarre un container par son nom
    /// Trouve le chemin du fichier de configuration d'un container par son nom
    fn find_container_config_path_by_name(executor: &CommandExecutor, name: &str) -> Option<String> {
        let possible_paths = vec![
            format!("/var/lib/lxc/{}/config", name),
            format!("/var/lib/lxd/containers/{}/config", name),
            format!("{}/.local/share/lxc/{}/config", std::env::var("HOME").unwrap_or_default(), name),
        ];
        
        for path in &possible_paths {
            let cmd_check = format!("test -f {} && echo 'found' || echo 'not found'", path);
            if let Ok(output) = executor.run_shell(&cmd_check, true) {
                if output.stdout.contains("found") {
                    return Some(path.clone());
                }
            }
        }
        
        None
    }

    pub fn start_container_by_name(executor: &CommandExecutor, name: &str) -> Result<CommandOutput, ExecError> {
        // Trouver le chemin du fichier de configuration
        let config_path = Self::find_container_config_path_by_name(executor, name);
        let cmd = if let Some(config) = &config_path {
            format!("lxc-start -f {} -n {}", config, name)
        } else {
            // Utiliser -P pour spécifier le répertoire racine des containers
            format!("lxc-start -P /var/lib/lxc -n {}", name)
        };
        
        let mut result = executor.run_shell(&cmd, true);
        
        // Si échec et qu'on n'a pas de config, vérifier que le fichier config existe vraiment
        if let Ok(ref output) = result {
            if output.exit_code != Some(0) && config_path.is_none() {
                // Vérifier si le fichier config existe vraiment
                let config_check = format!("test -f /var/lib/lxc/{}/config && echo 'exists' || echo 'missing'", name);
                if let Ok(check_output) = executor.run_shell(&config_check, true) {
                    if check_output.stdout.contains("exists") {
                        // Le fichier existe, utiliser -f explicitement
                        let cmd_alt = format!("lxc-start -f /var/lib/lxc/{}/config -n {}", name, name);
                        result = executor.run_shell(&cmd_alt, true);
                    }
                }
            }
        }
        
        result
    }
    
    /// Arrête un container par son nom
    pub fn stop_container_by_name(executor: &CommandExecutor, name: &str) -> Result<CommandOutput, ExecError> {
        // Trouver le chemin du fichier de configuration
        let config_path = Self::find_container_config_path_by_name(executor, name);
        let cmd = if let Some(config) = &config_path {
            format!("lxc-stop -f {} -n {}", config, name)
        } else {
            // Utiliser -P pour spécifier le répertoire racine des containers
            format!("lxc-stop -P /var/lib/lxc -n {}", name)
        };
        
        let mut result = executor.run_shell(&cmd, true);
        
        // Si échec et qu'on n'a pas de config, vérifier que le fichier config existe vraiment
        if let Ok(ref output) = result {
            if output.exit_code != Some(0) && config_path.is_none() {
                // Vérifier si le fichier config existe vraiment
                let config_check = format!("test -f /var/lib/lxc/{}/config && echo 'exists' || echo 'missing'", name);
                if let Ok(check_output) = executor.run_shell(&config_check, true) {
                    if check_output.stdout.contains("exists") {
                        // Le fichier existe, utiliser -f explicitement
                        let cmd_alt = format!("lxc-stop -f /var/lib/lxc/{}/config -n {}", name, name);
                        result = executor.run_shell(&cmd_alt, true);
                    }
                }
            }
        }
        
        result
    }
    
    /// Supprime un container par son nom
    pub fn destroy_container_by_name(executor: &CommandExecutor, name: &str) -> Result<CommandOutput, ExecError> {
        // Essayer d'abord d'arrêter le container s'il est en cours d'exécution
        let _ = Self::stop_container_by_name(executor, name);
        std::thread::sleep(std::time::Duration::from_secs(1));
        
        // Utiliser -f pour forcer la suppression même si le container est en cours d'exécution
        let cmd = format!("lxc-destroy -f -n {} 2>&1", name);
        executor.run_shell(&cmd, true)
    }
    
    /// Obtient des informations détaillées sur un container
    pub fn get_container_info(executor: &CommandExecutor, name: &str) -> Result<ContainerDetails, ExecError> {
        let status = Self::get_container_status_by_name(executor, name)?;
        
        // Obtenir l'IP si le container est en cours d'exécution
        let ip = if status == "RUNNING" {
            let ip_cmd = format!("lxc-info -n {} -i 2>/dev/null | grep 'IP:' | awk '{{print $2}}' || echo ''", name);
            executor.run_shell(&ip_cmd, false)
                .map(|o| o.stdout.trim().to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };
        
        // Obtenir des informations supplémentaires
        let arch_cmd = format!("lxc-info -n {} -c lxc.arch 2>/dev/null | grep 'lxc.arch' | awk '{{print $2}}' || echo ''", name);
        let arch = executor.run_shell(&arch_cmd, false)
            .map(|o| o.stdout.trim().to_string())
            .unwrap_or_default();
        
        Ok(ContainerDetails {
            name: name.to_string(),
            status,
            ip,
            arch: if arch.is_empty() { "unknown".to_string() } else { arch },
        })
    }
}

#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ContainerDetails {
    pub name: String,
    pub status: String,
    pub ip: String,
    pub arch: String,
}

#[derive(Debug, Clone)]
pub struct ContainerVerification {
    pub exists: bool,
    pub detectable_by_ls: bool,
    pub detectable_by_list: bool,
    pub detectable_by_filesystem: bool,
    pub can_get_status: bool,
    pub can_attach: bool,
    pub is_running: bool,
    pub errors: Vec<String>,
}

impl LXCDeployment {
    pub fn get_container_status(&self, executor: &CommandExecutor) -> Result<String, ExecError> {
        // Essayer plusieurs méthodes pour obtenir le statut du container
        // 1. lxc-info (LXC 1.x)
        let cmd1 = format!("lxc-info -n {} -s 2>/dev/null | grep 'State:' | awk '{{print $2}}' || echo 'UNKNOWN'", self.container_name);
        let output1 = executor.run_shell(&cmd1, false);
        
        if let Ok(ref out) = output1 {
            let status = out.stdout.trim().to_string();
            if status != "UNKNOWN" && !status.is_empty() {
                return Ok(status);
            }
        }
        
        // 2. lxc list (LXC 2.x+)
        let cmd2 = format!("lxc list {} --format csv -c s 2>/dev/null | head -1 || echo 'UNKNOWN'", self.container_name);
        let output2 = executor.run_shell(&cmd2, false);
        
        if let Ok(ref out) = output2 {
            let status = out.stdout.trim().to_string();
            if status != "UNKNOWN" && !status.is_empty() && status != "" {
                // Convertir le format CSV en statut simple
                let status_upper = status.to_uppercase();
                if status_upper.contains("RUNNING") {
                    return Ok("RUNNING".to_string());
                } else if status_upper.contains("STOPPED") {
                    return Ok("STOPPED".to_string());
                } else if status_upper.contains("FROZEN") {
                    return Ok("FROZEN".to_string());
                }
            }
        }
        
        // 3. Vérifier directement avec lxc-attach
        let cmd3 = format!("lxc-attach -n {} -- echo 'running' 2>/dev/null || echo 'UNKNOWN'", self.container_name);
        let output3 = executor.run_shell(&cmd3, true);
        
        if let Ok(ref out) = output3 {
            if out.exit_code == Some(0) {
                return Ok("RUNNING".to_string());
            }
        }
        
        // Par défaut, retourner UNKNOWN
        Ok("UNKNOWN".to_string())
    }

    pub fn install_rmdb_in_container(&self, executor: &CommandExecutor, rmdb_source_path: &str) -> Result<CommandOutput, ExecError> {
        self.log_info("Début de l'installation de RMDB dans le container");
        
        // Vérifier que le container existe (avec executor pour utiliser sudo)
        if !self.check_container_exists_with_executor(executor) {
            let msg = format!("Le container {} n'existe pas", self.container_name);
            self.log_error(&msg);
            return Err(ExecError::Failed(msg));
        }
        self.log_info("Container existe");

        // Vérifier le statut du container et le démarrer si nécessaire
        let status = self.get_container_status(executor).unwrap_or_else(|_| "UNKNOWN".to_string());
        self.log_info(&format!("Statut du container: {}", status));
        
        if status != "RUNNING" {
            self.log_warn(&format!("Le container n'est pas en cours d'exécution (statut: {}). Démarrage...", status));
            match self.start_container(executor) {
                Ok(_) => {
                    self.log_info("Container démarré");
                    // Attendre un peu que le container soit prêt
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
                Err(e) => {
                    let msg = format!("Impossible de démarrer le container: {}", e);
                    self.log_error(&msg);
                    return Err(ExecError::Failed(msg));
                }
            }
        }
        
        // Vérifier à nouveau que le container est accessible avec lxc-attach
        let test_cmd = format!("lxc-attach -n {} -- echo 'test'", self.container_name);
        match executor.run_shell(&test_cmd, true) {
            Ok(output) => {
                if output.exit_code != Some(0) {
                    let msg = format!("Le container {} n'est pas accessible via lxc-attach", self.container_name);
                    self.log_error(&msg);
                    return Err(ExecError::Failed(msg));
                }
                self.log_info("Container accessible via lxc-attach");
            }
            Err(e) => {
                let msg = format!("Impossible d'accéder au container via lxc-attach: {}", e);
                self.log_error(&msg);
                return Err(ExecError::Failed(msg));
            }
        }

        // Vérifier que le répertoire source existe
        let source_path = std::path::Path::new(rmdb_source_path);
        if !source_path.exists() {
            let msg = format!("Le répertoire source RMDB n'existe pas: {}", rmdb_source_path);
            self.log_error(&msg);
            return Err(ExecError::Failed(msg));
        }
        self.log_info(&format!("Répertoire source trouvé: {}", rmdb_source_path));

        let rmdb_dir = "/root/rmdb";
        self.log_step(1, 8, "Copie des fichiers RMDB dans le container");

        // Étape 1: Copier les fichiers RMDB dans le container
        let copy_cmd = format!(
            "cd {} && tar -czf - --exclude='.git' --exclude='*.log' --exclude='*.db' --exclude='target' --exclude='ultraManagerTUI/target' . | lxc-attach -n {} -- tar -xzf - -C {}",
            rmdb_source_path, self.container_name, rmdb_dir
        );
        
        // Créer le répertoire dans le container
        let mkdir_cmd = format!("lxc-attach -n {} -- mkdir -p {}", self.container_name, rmdb_dir);
        executor.run_shell(&mkdir_cmd, true)?;

        // Copier les fichiers
        executor.run_shell(&copy_cmd, true)?;

        // Étape 2: Installer les dépendances
        self.log_step(2, 8, "Installation des dépendances système");
        let install_deps_cmd = format!(
            "lxc-attach -n {} -- sh -c 'apk update && apk add -q go git make gcc musl-dev nbd-server'",
            self.container_name
        );
        self.log_command(&install_deps_cmd);
        let deps_result = executor.run_shell(&install_deps_cmd, true);
        if let Err(ref e) = deps_result {
            self.log_error(&format!("Échec de l'installation des dépendances: {}", e));
            return deps_result.map(|_| CommandOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: Some(0),
            });
        }
        if let Ok(ref output) = deps_result {
            self.log_command_output(output);
        }
        self.log_info("Dépendances installées");
        deps_result?;

        // Étape 3: Télécharger les dépendances Go
        self.log_step(3, 8, "Téléchargement des dépendances Go");
        let go_mod_cmd = format!(
            "lxc-attach -n {} -- sh -c 'cd {} && go mod download'",
            self.container_name, rmdb_dir
        );
        self.log_command(&go_mod_cmd);
        let go_mod_result = executor.run_shell(&go_mod_cmd, true);
        if let Err(ref e) = go_mod_result {
            self.log_error(&format!("Échec du téléchargement des dépendances Go: {}", e));
            return go_mod_result.map(|_| CommandOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: Some(0),
            });
        }
        if let Ok(ref output) = go_mod_result {
            self.log_command_output(output);
        }
        self.log_info("Dépendances Go téléchargées");
        go_mod_result?;

        // Étape 4: Compiler RMDB
        self.log_step(4, 8, "Compilation de RMDB");
        let build_cmd = format!(
            "lxc-attach -n {} -- sh -c 'cd {} && CGO_ENABLED=0 go build -trimpath -ldflags \"-s -w\" -o /usr/local/bin/rmdbd ./cmd/rmdbd && chmod +x /usr/local/bin/rmdbd'",
            self.container_name, rmdb_dir
        );
        self.log_command(&build_cmd);
        let build_result = executor.run_shell(&build_cmd, true);
        if let Err(ref e) = build_result {
            self.log_error(&format!("Échec de la compilation: {}", e));
            return build_result.map(|_| CommandOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: Some(0),
            });
        }
        if let Ok(ref output) = build_result {
            self.log_command_output(output);
            // Vérifier que le binaire existe
            let check_binary_cmd = format!("lxc-attach -n {} -- test -f /usr/local/bin/rmdbd", self.container_name);
            let check_result = executor.run_shell(&check_binary_cmd, false);
            if let Ok(ref check_output) = check_result {
                if check_output.exit_code == Some(0) {
                    self.log_info("Binaire rmdbd créé avec succès");
                } else {
                    self.log_error("Le binaire rmdbd n'a pas été créé");
                }
            }
        }
        self.log_info("RMDB compilé");
        build_result?;

        // Étape 5: Créer la configuration
        self.log_step(5, 8, "Création de la configuration");
        let config_cmd = format!(
            "lxc-attach -n {} -- sh -c 'mkdir -p /etc/rmdbd && cp {}/configs/rmdbd.example.json /etc/rmdbd/config.json 2>/dev/null || true'",
            self.container_name, rmdb_dir
        );
        self.log_command(&config_cmd);
        let config_result = executor.run_shell(&config_cmd, true);
        if let Ok(ref output) = config_result {
            self.log_command_output(output);
            // Vérifier que le fichier de config existe
            let check_config_cmd = format!("lxc-attach -n {} -- test -f /etc/rmdbd/config.json", self.container_name);
            let check_result = executor.run_shell(&check_config_cmd, false);
            if let Ok(ref check_output) = check_result {
                if check_output.exit_code == Some(0) {
                    self.log_info("Fichier de configuration créé");
                } else {
                    self.log_warn("Le fichier de configuration n'a pas été créé");
                }
            }
        }
        config_result?;

        // Étape 6: Créer le service OpenRC
        self.log_step(6, 8, "Création du service OpenRC");
        let service_cmd = format!(
            "lxc-attach -n {} -- sh -c 'cat > /etc/init.d/rmdbd <<\\'SERVICEEOF\\'
#!/sbin/openrc-run
command=\"/usr/local/bin/rmdbd\"
command_args=\"-config /etc/rmdbd/config.json\"
pidfile=\"/var/run/rmdbd.pid\"
command_background=true

depend() {{
    need net
    after firewall
}}
SERVICEEOF
chmod +x /etc/init.d/rmdbd && rc-update add rmdbd default 2>/dev/null || true'",
            self.container_name
        );
        self.log_command(&service_cmd);
        let service_result = executor.run_shell(&service_cmd, true);
        if let Ok(ref output) = service_result {
            self.log_command_output(output);
            // Vérifier que le service existe
            let check_service_cmd = format!("lxc-attach -n {} -- test -f /etc/init.d/rmdbd", self.container_name);
            let check_result = executor.run_shell(&check_service_cmd, false);
            if let Ok(ref check_output) = check_result {
                if check_output.exit_code == Some(0) {
                    self.log_info("Service OpenRC créé");
                } else {
                    self.log_warn("Le service OpenRC n'a pas été créé");
                }
            }
        }
        service_result?;

        // Étape 7: Créer les répertoires nécessaires
        self.log_step(7, 8, "Création des répertoires de données");
        let dirs_cmd = format!(
            "lxc-attach -n {} -- sh -c 'mkdir -p /var/lib/rmdb/{{www,images,vms,overlays,backups,audits,checksums,metrics,tftpboot}} /var/log && chown -R root:root /var/lib/rmdb'",
            self.container_name
        );
        self.log_command(&dirs_cmd);
        let dirs_result = executor.run_shell(&dirs_cmd, true);
        if let Ok(ref output) = dirs_result {
            self.log_command_output(output);
        }
        dirs_result?;

        // Étape 8: Télécharger les fichiers iPXE de base (optionnel, ne pas échouer si ça rate)
        self.log_step(8, 8, "Téléchargement des fichiers iPXE");
        let ipxe_cmd = format!(
            "lxc-attach -n {} -- sh -c 'cd /var/lib/rmdb/www && (command -v wget >/dev/null 2>&1 || apk add -q wget) && wget -q -O /var/lib/rmdb/www/undionly.kpxe https://boot.ipxe.org/undionly.kpxe 2>/dev/null || true && wget -q -O /var/lib/rmdb/www/ipxe.efi https://boot.ipxe.org/ipxe.efi 2>/dev/null || true && cp /var/lib/rmdb/www/undionly.kpxe /var/lib/rmdb/tftpboot/ 2>/dev/null || true && cp /var/lib/rmdb/www/ipxe.efi /var/lib/rmdb/tftpboot/ 2>/dev/null || true'",
            self.container_name
        );
        self.log_command(&ipxe_cmd);
        let ipxe_result = executor.run_shell(&ipxe_cmd, true);
        if let Ok(ref output) = ipxe_result {
            self.log_command_output(output);
        }
        // Ne pas échouer si le téléchargement échoue
        if ipxe_result.is_err() {
            self.log_warn("Échec du téléchargement des fichiers iPXE (non bloquant)");
        } else {
            self.log_info("Fichiers iPXE téléchargés");
        }

        self.log_info("Installation de RMDB terminée avec succès");
        Ok(CommandOutput {
            stdout: format!("RMDB installé avec succès dans le container {}", self.container_name),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }

    fn log_step(&self, step: u32, total: u32, message: &str) {
        if let Some(ref logger) = self.logger {
            logger.step(step, total, message);
        }
    }

    fn setup_lxc_config_for_rhel(&self, _executor: &CommandExecutor) -> Result<(), ExecError> {
        // Créer le répertoire de configuration LXC si nécessaire
        let config_path = self.distribution.lxc_config_path();
        let config_dir = Path::new(&config_path).parent().unwrap_or(Path::new("/tmp"));
        
        self.log_info(&format!("Configuration LXC pour RHEL: {}", config_path));
        
        // Créer le répertoire
        if !config_dir.exists() {
            self.log_info(&format!("Création du répertoire: {}", config_dir.display()));
            fs::create_dir_all(config_dir).map_err(|e| {
                ExecError::Failed(format!("Impossible de créer {}: {}", config_dir.display(), e))
            })?;
        }

        // Créer le fichier de configuration par défaut si nécessaire
        if !Path::new(&config_path).exists() {
            self.log_info("Création de la configuration LXC par défaut");
            
            // Configuration par défaut pour RHEL/CentOS
            // Sur RHEL, LXC nécessite souvent des mappings UID ou l'exécution en root
            let default_config = if self.distribution.needs_root_for_lxc() {
                // Configuration pour root (recommandé sur RHEL)
                format!(
                    "# Configuration LXC par défaut pour RMDB\n\
                     lxc.include = /usr/share/lxc/config/common.conf\n\
                     lxc.include = /usr/share/lxc/config/userns.conf\n\
                     lxc.arch = x86_64\n\
                     lxc.net.0.type = veth\n\
                     lxc.net.0.link = lxcbr0\n\
                     lxc.net.0.flags = up\n\
                     lxc.net.0.ipv4.address = auto\n\
                     lxc.net.0.ipv4.gateway = auto\n\
                     lxc.rootfs.path = dir:{}\n\
                     lxc.utsname = ${{container_name}}\n",
                    self.distribution.lxc_container_path()
                )
            } else {
                // Configuration standard
                format!(
                    "# Configuration LXC par défaut pour RMDB\n\
                     lxc.include = /usr/share/lxc/config/common.conf\n\
                     lxc.arch = x86_64\n\
                     lxc.net.0.type = veth\n\
                     lxc.net.0.link = lxcbr0\n\
                     lxc.net.0.flags = up\n\
                     lxc.net.0.ipv4.address = auto\n\
                     lxc.net.0.ipv4.gateway = auto\n\
                     lxc.rootfs.path = dir:{}\n\
                     lxc.utsname = ${{container_name}}\n",
                    self.distribution.lxc_container_path()
                )
            };

            fs::write(&config_path, default_config).map_err(|e| {
                ExecError::Failed(format!("Impossible d'écrire {}: {}", config_path, e))
            })?;
            
            self.log_info(&format!("Configuration créée: {}", config_path));
        } else {
            self.log_info(&format!("Configuration existante: {}", config_path));
        }

        Ok(())
    }
}



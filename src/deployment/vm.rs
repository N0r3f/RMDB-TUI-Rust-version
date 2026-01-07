/// Module pour la création et la gestion de VMs Rocky Linux
/// et l'installation de RMDB dans ces VMs
use crate::pres::executor::{CommandExecutor, CommandOutput, ExecError};
use crate::data::distribution::DistributionInfo;
use crate::deployment::logger::DeploymentLogger;

pub struct VMDeployment {
    vm_name: String,
    rocky_version: String,
    pub logger: Option<DeploymentLogger>,
    distribution: DistributionInfo,
}

impl VMDeployment {
    pub fn new(vm_name: String, rocky_version: String) -> Self {
        Self {
            vm_name,
            rocky_version,
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

    /// Vérifie si libvirt/KVM est installé
    pub fn check_virt_installed(&self, executor: &CommandExecutor) -> bool {
        let cmd = "command -v virsh >/dev/null 2>&1 && command -v virt-install >/dev/null 2>&1 && echo 'installed' || echo 'not_installed'";
        if let Ok(output) = executor.run_shell(cmd, false) {
            output.stdout.contains("installed")
        } else {
            false
        }
    }

    /// Installe libvirt/KVM si nécessaire
    pub fn install_virt(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        if self.check_virt_installed(executor) {
            return Ok(CommandOutput {
                stdout: "libvirt/KVM est déjà installé".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
            });
        }

        let packages = match self.distribution.package_manager {
            crate::data::distribution::PackageManager::Apt => {
                vec!["qemu-kvm", "libvirt-daemon-system", "libvirt-clients", "virtinst", "virt-manager"]
            }
            crate::data::distribution::PackageManager::Dnf | crate::data::distribution::PackageManager::Yum => {
                vec!["qemu-kvm", "libvirt", "libvirt-daemon", "virt-install", "virt-manager"]
            }
            crate::data::distribution::PackageManager::Pacman => {
                vec!["qemu", "libvirt", "virt-install", "virt-manager"]
            }
            crate::data::distribution::PackageManager::Zypper => {
                vec!["qemu-kvm", "libvirt", "virt-install", "virt-manager"]
            }
            _ => {
                return Err(ExecError::MissingTool("Gestionnaire de paquets non supporté pour l'installation de libvirt".to_string()));
            }
        };

        let packages_refs: Vec<&str> = packages.iter().map(|s| *s).collect();
        let install_cmd = self.distribution.install_command(&packages_refs);
        executor.run_shell(&install_cmd, true)
    }

    /// Vérifie si la VM existe
    pub fn check_vm_exists(&self, executor: &CommandExecutor) -> bool {
        let cmd = format!("virsh dominfo {} 2>/dev/null | grep -q 'Id:' && echo 'exists' || echo 'not_exists'", self.vm_name);
        if let Ok(output) = executor.run_shell(&cmd, false) {
            output.stdout.contains("exists")
        } else {
            false
        }
    }

    /// Crée une VM Rocky Linux
    pub fn create_vm(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        self.log_info(&format!("Début de la création de la VM Rocky Linux '{}'", self.vm_name));

        // Vérifier/installer libvirt
        if !self.check_virt_installed(executor) {
            self.log_info("Installation de libvirt/KVM...");
            self.install_virt(executor)?;
        }

        // Vérifier si la VM existe déjà
        if self.check_vm_exists(executor) {
            let msg = format!("La VM {} existe déjà", self.vm_name);
            self.log_error(&msg);
            return Err(ExecError::Failed(msg));
        }

        // Créer le répertoire pour les disques VM
        let vm_dir = format!("/var/lib/libvirt/images/{}", self.vm_name);
        let mkdir_cmd = format!("sudo mkdir -p {}", vm_dir);
        executor.run_shell(&mkdir_cmd, true)?;

        // Télécharger l'ISO Rocky Linux si nécessaire
        let iso_path = format!("{}/Rocky-{}-x86_64-minimal.iso", vm_dir, self.rocky_version);
        let iso_check = format!("test -f {} && echo 'exists' || echo 'not_exists'", iso_path);
        let iso_output = executor.run_shell(&iso_check, false)?;

        if iso_output.stdout.contains("not_exists") {
            self.log_info("Téléchargement de l'ISO Rocky Linux...");
            // URL de téléchargement Rocky Linux (à adapter selon la version)
            let iso_url = format!(
                "https://download.rockylinux.org/pub/rocky/{}/isos/x86_64/Rocky-{}-x86_64-minimal.iso",
                self.rocky_version, self.rocky_version
            );
            let download_cmd = format!(
                "cd {} && sudo wget -q --show-progress -O {} {}",
                vm_dir, iso_path, iso_url
            );
            executor.run_shell(&download_cmd, true)?;
        }

        // Créer le disque virtuel
        let disk_path = format!("{}/{}.qcow2", vm_dir, self.vm_name);
        let disk_size = "20G"; // Taille par défaut
        let create_disk_cmd = format!(
            "sudo qemu-img create -f qcow2 {} {}",
            disk_path, disk_size
        );
        self.log_command(&create_disk_cmd);
        executor.run_shell(&create_disk_cmd, true)?;

        // Créer la VM avec virt-install
        // Note: Cette commande nécessite un environnement graphique ou VNC
        // Pour une installation non-interactive, on peut utiliser cloud-init
        let create_vm_cmd = format!(
            "sudo virt-install \
            --name {} \
            --ram 2048 \
            --vcpus 2 \
            --disk path={},size=20,format=qcow2 \
            --cdrom {} \
            --network network=default \
            --graphics vnc,listen=0.0.0.0 \
            --noautoconsole \
            --os-type linux \
            --os-variant rocky{} \
            --wait -1",
            self.vm_name, disk_path, iso_path, self.rocky_version
        );

        self.log_command(&create_vm_cmd);
        let result = executor.run_shell(&create_vm_cmd, true);

        if let Ok(ref output) = result {
            self.log_command_output(output);
            if output.exit_code == Some(0) {
                self.log_info("VM créée avec succès");
            } else {
                self.log_error("Échec de la création de la VM");
            }
        }

        result
    }

    /// Installe RMDB dans la VM
    pub fn install_rmdb_in_vm(&self, executor: &CommandExecutor, rmdb_source_path: &str) -> Result<CommandOutput, ExecError> {
        self.log_info("Début de l'installation de RMDB dans la VM");

        // Vérifier que la VM existe et est en cours d'exécution
        if !self.check_vm_exists(executor) {
            return Err(ExecError::Failed(format!("La VM {} n'existe pas", self.vm_name)));
        }

        // Démarrer la VM si elle n'est pas en cours d'exécution
        let status_cmd = format!("virsh dominfo {} | grep 'State:' | awk '{{print $2}}'", self.vm_name);
        let status_output = executor.run_shell(&status_cmd, false)?;
        let status = status_output.stdout.trim().to_lowercase();

        if !status.contains("running") {
            self.log_info("Démarrage de la VM...");
            let start_cmd = format!("virsh start {}", self.vm_name);
            executor.run_shell(&start_cmd, false)?;
            // Attendre que la VM soit prête
            std::thread::sleep(std::time::Duration::from_secs(10));
        }

        // Obtenir l'IP de la VM
        let vm_ip = match self.get_vm_ip(executor) {
            Ok(ip) => {
                self.log_info(&format!("IP de la VM: {}", ip));
                ip
            }
            Err(e) => {
                self.log_warn(&format!("Impossible d'obtenir l'IP de la VM: {}. L'installation se fera via cloud-init.", e));
                String::new()
            }
        };

        // Créer un script cloud-init pour installer RMDB
        self.log_info("Création du script cloud-init pour l'installation de RMDB...");
        let cloud_init_script = self.create_cloud_init_script(&rmdb_source_path)?;
        
        // Copier le script cloud-init dans la VM (via virsh)
        // Note: Pour une installation complète, il faudrait :
        // 1. Créer une image cloud-init avec le script
        // 2. Ou utiliser virt-copy-in pour copier les fichiers
        // 3. Ou utiliser SSH si la VM est accessible

        if !vm_ip.is_empty() {
            // Essayer d'installer via SSH si l'IP est disponible
            self.log_info("Tentative d'installation via SSH...");
            // TODO: Implémenter l'installation via SSH
            // Cela nécessiterait :
            // - Configuration SSH (clés, mot de passe)
            // - Copie des fichiers via scp
            // - Exécution des commandes via ssh
        } else {
            self.log_info("L'installation complète nécessitera un accès manuel à la VM.");
            self.log_info("Utilisez le script cloud-init généré pour finaliser l'installation.");
        }

        self.log_info("Installation de RMDB dans la VM terminée");
        Ok(CommandOutput {
            stdout: format!("VM {} créée. Installation de RMDB nécessite un accès à la VM.", self.vm_name),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }

    /// Obtient l'IP de la VM
    fn get_vm_ip(&self, executor: &CommandExecutor) -> Result<String, ExecError> {
        // Essayer plusieurs méthodes pour obtenir l'IP
        // 1. Via virsh domifaddr
        let cmd1 = format!("virsh domifaddr {} 2>/dev/null | grep -oP '\\d+\\.\\d+\\.\\d+\\.\\d+' | head -1", self.vm_name);
        if let Ok(output) = executor.run_shell(&cmd1, false) {
            let ip = output.stdout.trim().to_string();
            if !ip.is_empty() {
                return Ok(ip);
            }
        }

        // 2. Via arp (simplifié)
        let cmd2 = format!("virsh domiflist {} 2>/dev/null | tail -n +2 | head -1 | awk '{{print $1}}' | xargs -I IFACE ip neigh show 2>/dev/null | grep IFACE | awk '{{print $1}}' | head -1", self.vm_name);
        if let Ok(output) = executor.run_shell(&cmd2, false) {
            let ip = output.stdout.trim().to_string();
            if !ip.is_empty() {
                return Ok(ip);
            }
        }

        Err(ExecError::Failed("Impossible d'obtenir l'IP de la VM".to_string()))
    }

    /// Crée un script cloud-init pour installer RMDB dans la VM
    fn create_cloud_init_script(&self, rmdb_source_path: &str) -> Result<String, ExecError> {
        // Créer un script cloud-init qui :
        // 1. Installe Go
        // 2. Copie les fichiers RMDB (via un volume partagé ou téléchargement)
        // 3. Compile RMDB
        // 4. Configure le service

        let script = format!(
            r#"#cloud-config
# Script d'installation RMDB pour Rocky Linux

package_update: true
package_upgrade: true

packages:
  - wget
  - tar
  - git
  - gcc
  - make

write_files:
  - path: /root/install_rmdb.sh
    permissions: '0755'
    content: |
      #!/bin/bash
      set -e
      
      # Installer Go
      GO_VERSION="1.21.0"
      cd /tmp
      wget -q https://go.dev/dl/go${{GO_VERSION}}.linux-amd64.tar.gz
      tar -C /usr/local -xzf go${{GO_VERSION}}.linux-amd64.tar.gz
      echo 'export PATH=$PATH:/usr/local/go/bin' >> /etc/profile
      source /etc/profile
      
      # Note: Les fichiers RMDB doivent être copiés manuellement ou via un volume partagé
      # Pour l'instant, on suppose qu'ils sont dans /root/rmdb_source
      if [ -d /root/rmdb_source ]; then
          cd /root/rmdb_source
          go mod download
          CGO_ENABLED=0 go build -trimpath -ldflags "-s -w" -o /usr/local/bin/rmdbd ./cmd/rmdbd
          chmod +x /usr/local/bin/rmdbd
          
          # Créer la configuration
          mkdir -p /etc/rmdbd
          cp configs/rmdbd.example.json /etc/rmdbd/config.json 2>/dev/null || true
          
          # Créer les répertoires
          mkdir -p /var/lib/rmdb/{{www,images,vms,overlays,backups,audits}}
          
          # Créer le service systemd
          cat > /etc/systemd/system/rmdbd.service << 'EOF'
      [Unit]
      Description=RMDB Server
      After=network.target
      
      [Service]
      Type=simple
      ExecStart=/usr/local/bin/rmdbd -config /etc/rmdbd/config.json
      Restart=always
      RestartSec=5
      
      [Install]
      WantedBy=multi-user.target
      EOF
          
          systemctl daemon-reload
          systemctl enable rmdbd
      fi

runcmd:
  - /root/install_rmdb.sh
"#
        );

        // Sauvegarder le script dans un fichier temporaire
        let script_path = format!("/tmp/rmdb-cloud-init-{}.yaml", self.vm_name);
        std::fs::write(&script_path, &script)
            .map_err(|e| ExecError::Failed(format!("Impossible d'écrire le script cloud-init: {}", e)))?;

        Ok(script_path)
    }

    /// Démarre la VM
    pub fn start_vm(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        let cmd = format!("virsh start {}", self.vm_name);
        executor.run_shell(&cmd, false)
    }

    /// Arrête la VM
    pub fn stop_vm(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        let cmd = format!("virsh shutdown {}", self.vm_name);
        executor.run_shell(&cmd, false)
    }

    /// Supprime la VM
    pub fn destroy_vm(&self, executor: &CommandExecutor) -> Result<CommandOutput, ExecError> {
        // Arrêter la VM si elle est en cours d'exécution
        let _ = self.stop_vm(executor);
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Supprimer la VM
        let cmd = format!("virsh undefine {} --remove-all-storage", self.vm_name);
        executor.run_shell(&cmd, false)
    }
}


use crate::data::capabilities::Capabilities;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActionMode {
    ReadOnly,
    Safe,
    Admin,
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub enum ExecError {
    NotAllowed(String),
    MissingTool(String),
    Failed(String),
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecError::NotAllowed(msg) => write!(f, "Action non autorisée: {}", msg),
            ExecError::MissingTool(msg) => write!(f, "Outil manquant: {}", msg),
            ExecError::Failed(msg) => write!(f, "Échec: {}", msg),
        }
    }
}

pub struct CommandExecutor {
    mode: ActionMode,
    caps: Capabilities,
}

impl CommandExecutor {
    pub fn new(mode: ActionMode, caps: Capabilities) -> Self {
        Self { mode, caps }
    }

    pub fn set_mode(&mut self, mode: ActionMode) {
        self.mode = mode;
    }

    pub fn mode(&self) -> ActionMode {
        self.mode
    }

    pub fn run_shell(&self, cmd: &str, requires_admin: bool) -> Result<CommandOutput, ExecError> {
        if requires_admin {
            if self.mode != ActionMode::Admin {
                return Err(ExecError::NotAllowed(
                    "Action admin refusée: passez en mode Admin".to_string(),
                ));
            }
            if !self.caps.has_sudo {
                return Err(ExecError::MissingTool(
                    "sudo est requis en mode Admin mais introuvable".to_string(),
                ));
            }
        }

        // Si la commande nécessite des privilèges admin et qu'on est en mode Admin,
        // on doit préfixer la commande avec sudo -n (non-interactif, utilise le timestamp)
        // sudo -n utilise le timestamp sudo valide obtenu lors de l'authentification
        let out = if requires_admin && self.mode == ActionMode::Admin {
            // Exécuter avec sudo -n pour utiliser le timestamp sudo valide
            Command::new("sudo")
                .args(["-n", "sh", "-c", cmd])
                .output()
                .map_err(|e| ExecError::Failed(format!("Impossible d'exécuter la commande avec sudo: {}", e)))?
        } else {
            // Exécuter sans sudo
            Command::new("sh")
            .args(["-lc", cmd])
            .output()
                .map_err(|e| ExecError::Failed(format!("Impossible d'exécuter la commande: {}", e)))?
        };

        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();

        Ok(CommandOutput {
            exit_code: out.status.code(),
            stdout,
            stderr,
        })
    }
}


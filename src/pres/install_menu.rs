/// Menu d'installation de RMDB
/// Permet de choisir le type d'installation (host, container, VM)
use crate::deployment::installer::InstallationMode;

#[derive(Clone)]
pub struct InstallMenuItem {
    pub id: usize,
    pub label: &'static str,
    pub action: InstallMenuAction,
}

#[derive(Clone)]
pub enum InstallMenuAction {
    SelectInstallationMode,
    InstallOnHost,
    InstallInContainer,
    InstallInVM,
    ConfigureInstallation,
    Back,
}

pub fn get_install_menu() -> Vec<InstallMenuItem> {
    vec![
        InstallMenuItem {
            id: 0,
            label: "Sélectionner le mode d'utilisation",
            action: InstallMenuAction::SelectInstallationMode,
        },
        InstallMenuItem {
            id: 1,
            label: "Installer RMDB sur le système hôte",
            action: InstallMenuAction::InstallOnHost,
        },
        InstallMenuItem {
            id: 2,
            label: "Installer RMDB dans un container Alpine Linux",
            action: InstallMenuAction::InstallInContainer,
        },
        InstallMenuItem {
            id: 3,
            label: "Installer RMDB dans une VM Rocky Linux",
            action: InstallMenuAction::InstallInVM,
        },
        InstallMenuItem {
            id: 4,
            label: "Configurer l'installation",
            action: InstallMenuAction::ConfigureInstallation,
        },
        InstallMenuItem {
            id: 5,
            label: "Retour",
            action: InstallMenuAction::Back,
        },
    ]
}

/// Menu de sélection du mode d'utilisation
pub fn get_mode_selection_menu() -> Vec<(InstallationMode, &'static str)> {
    vec![
        (InstallationMode::DesktopGUI, "Client graphique desktop (Mode normal)"),
        (InstallationMode::WebServer, "Via le navigateur web (Mode serveur)"),
        (InstallationMode::TerminalTUI, "En terminal TUI (Mode avancé)"),
    ]
}


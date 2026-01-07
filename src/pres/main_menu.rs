#[derive(Clone)]
pub struct MainMenuItem {
    pub id: usize,
    pub label: &'static str,
    pub category: MainMenuCategory,
    pub action: MainMenuAction,
}

#[derive(Clone)]
pub enum MainMenuCategory {
    Services,
    IPXE,
    Clients,
    VMs,
    Configuration,
    Monitoring,
    System,
    Containers,
    Host,
}

#[derive(Clone)]
pub enum MainMenuAction {
    // Actions principales (thématiques)
    ServicesTheme,
    IPXETheme,
    ClientsTheme,
    VMsTheme,
    ConfigurationTheme,
    MonitoringTheme,
    SystemTheme,
    // Actions Services
    ServiceDHCP,
    ServiceDNS,
    ServiceTFTP,
    ServiceHTTP,
    ServiceStatus,
    ServiceStart,
    ServiceStop,
    ServiceRestart,
    // Actions IPXE
    IPXEMenu,
    IPXEEntries,
    IPXEGenerate,
    IPXEConfig,
    // Actions Clients
    ClientsLeases,
    ClientsConnected,
    ClientsHistory,
    // Actions VMs
    VMsList,
    VMsCreate,
    VMsManage,
    VMsOverlays,
    // Actions Configuration
    ConfigView,
    ConfigEdit,
    ConfigNetwork,
    ConfigSecurity,
    // Actions Monitoring
    MonitoringLogs,
    MonitoringMetrics,
    MonitoringHealth,
    MonitoringDashboard,
    // Actions Système
    SystemInfo,
    SystemServices,
    SystemProcesses,
    // Actions Déploiement
    DeployLXC,
    DeployStatus,
    // Actions Gestion Container LXC (spécifique RMDB)
    LXCManage,
    LXCStart,
    LXCStop,
    LXCRestart,
    LXCLogs,
    LXCShell,
    LXCStats,
    LXCRmdbStart,
    LXCRmdbStop,
    LXCRmdbRestart,
    LXCRmdbLogs,
    LXCConfig,
    LXCDestroy,
    // Actions Gestion Générale Containers LXC
    ContainersTheme,
    ContainersList,
    ContainersStart,
    ContainersStop,
    ContainersRestart,
    ContainersAdd,
    ContainersDestroy,
    ContainersReinstall,
    // Actions RMDB sur Système Hôte
    HostTheme,
    HostInstall,
    HostStatus,
    HostStart,
    HostStop,
    HostRestart,
    HostEnable,
    HostDisable,
    HostUninstall,
    // Actions Installation
    InstallMenu,
    InstallOnHost,
    InstallInContainer,
    InstallInVM,
    Quit,
}

pub fn get_main_menu() -> Vec<MainMenuItem> {
    vec![
        MainMenuItem {
            id: 0,
            label: "Services",
            category: MainMenuCategory::Services,
            action: MainMenuAction::ServicesTheme,
        },
        MainMenuItem {
            id: 1,
            label: "IPXE",
            category: MainMenuCategory::IPXE,
            action: MainMenuAction::IPXETheme,
        },
        MainMenuItem {
            id: 2,
            label: "Clients",
            category: MainMenuCategory::Clients,
            action: MainMenuAction::ClientsTheme,
        },
        MainMenuItem {
            id: 3,
            label: "VMs",
            category: MainMenuCategory::VMs,
            action: MainMenuAction::VMsTheme,
        },
        MainMenuItem {
            id: 4,
            label: "Configuration",
            category: MainMenuCategory::Configuration,
            action: MainMenuAction::ConfigurationTheme,
        },
        MainMenuItem {
            id: 5,
            label: "Monitoring",
            category: MainMenuCategory::Monitoring,
            action: MainMenuAction::MonitoringTheme,
        },
        MainMenuItem {
            id: 6,
            label: "Système",
            category: MainMenuCategory::System,
            action: MainMenuAction::SystemTheme,
        },
        MainMenuItem {
            id: 7,
            label: "Containers LXC",
            category: MainMenuCategory::Containers,
            action: MainMenuAction::ContainersTheme,
        },
        MainMenuItem {
            id: 8,
            label: "RMDB Hôte",
            category: MainMenuCategory::Host,
            action: MainMenuAction::HostTheme,
        },
        MainMenuItem {
            id: 9,
            label: "Installation RMDB",
            category: MainMenuCategory::System,
            action: MainMenuAction::InstallMenu,
        },
        MainMenuItem {
            id: 10,
            label: "Quitter",
            category: MainMenuCategory::System,
            action: MainMenuAction::Quit,
        },
    ]
}

pub fn get_services_submenu() -> Vec<MainMenuItem> {
    vec![
        MainMenuItem {
            id: 0,
            label: "Statut des Services",
            category: MainMenuCategory::Services,
            action: MainMenuAction::ServiceStatus,
        },
        MainMenuItem {
            id: 1,
            label: "Service DHCP",
            category: MainMenuCategory::Services,
            action: MainMenuAction::ServiceDHCP,
        },
        MainMenuItem {
            id: 2,
            label: "Service DNS",
            category: MainMenuCategory::Services,
            action: MainMenuAction::ServiceDNS,
        },
        MainMenuItem {
            id: 3,
            label: "Service TFTP",
            category: MainMenuCategory::Services,
            action: MainMenuAction::ServiceTFTP,
        },
        MainMenuItem {
            id: 4,
            label: "Service HTTP/HTTPS",
            category: MainMenuCategory::Services,
            action: MainMenuAction::ServiceHTTP,
        },
        MainMenuItem {
            id: 5,
            label: "Démarrer Services",
            category: MainMenuCategory::Services,
            action: MainMenuAction::ServiceStart,
        },
        MainMenuItem {
            id: 6,
            label: "Arrêter Services",
            category: MainMenuCategory::Services,
            action: MainMenuAction::ServiceStop,
        },
        MainMenuItem {
            id: 7,
            label: "Redémarrer Services",
            category: MainMenuCategory::Services,
            action: MainMenuAction::ServiceRestart,
        },
        MainMenuItem {
            id: 8,
            label: "Retour",
            category: MainMenuCategory::Services,
            action: MainMenuAction::ServicesTheme,
        },
    ]
}

pub fn get_ipxe_submenu() -> Vec<MainMenuItem> {
    vec![
        MainMenuItem {
            id: 0,
            label: "Menu iPXE",
            category: MainMenuCategory::IPXE,
            action: MainMenuAction::IPXEMenu,
        },
        MainMenuItem {
            id: 1,
            label: "Entrées de Menu",
            category: MainMenuCategory::IPXE,
            action: MainMenuAction::IPXEEntries,
        },
        MainMenuItem {
            id: 2,
            label: "Générer Menu",
            category: MainMenuCategory::IPXE,
            action: MainMenuAction::IPXEGenerate,
        },
        MainMenuItem {
            id: 3,
            label: "Configuration iPXE",
            category: MainMenuCategory::IPXE,
            action: MainMenuAction::IPXEConfig,
        },
        MainMenuItem {
            id: 4,
            label: "Retour",
            category: MainMenuCategory::IPXE,
            action: MainMenuAction::IPXETheme,
        },
    ]
}

pub fn get_clients_submenu() -> Vec<MainMenuItem> {
    vec![
        MainMenuItem {
            id: 0,
            label: "Leases DHCP",
            category: MainMenuCategory::Clients,
            action: MainMenuAction::ClientsLeases,
        },
        MainMenuItem {
            id: 1,
            label: "Clients Connectés",
            category: MainMenuCategory::Clients,
            action: MainMenuAction::ClientsConnected,
        },
        MainMenuItem {
            id: 2,
            label: "Historique",
            category: MainMenuCategory::Clients,
            action: MainMenuAction::ClientsHistory,
        },
        MainMenuItem {
            id: 3,
            label: "Retour",
            category: MainMenuCategory::Clients,
            action: MainMenuAction::ClientsTheme,
        },
    ]
}

pub fn get_vms_submenu() -> Vec<MainMenuItem> {
    vec![
        MainMenuItem {
            id: 0,
            label: "Liste des VMs",
            category: MainMenuCategory::VMs,
            action: MainMenuAction::VMsList,
        },
        MainMenuItem {
            id: 1,
            label: "Créer VM",
            category: MainMenuCategory::VMs,
            action: MainMenuAction::VMsCreate,
        },
        MainMenuItem {
            id: 2,
            label: "Gérer VM",
            category: MainMenuCategory::VMs,
            action: MainMenuAction::VMsManage,
        },
        MainMenuItem {
            id: 3,
            label: "Overlays",
            category: MainMenuCategory::VMs,
            action: MainMenuAction::VMsOverlays,
        },
        MainMenuItem {
            id: 4,
            label: "Retour",
            category: MainMenuCategory::VMs,
            action: MainMenuAction::VMsTheme,
        },
    ]
}

pub fn get_configuration_submenu() -> Vec<MainMenuItem> {
    vec![
        MainMenuItem {
            id: 0,
            label: "Voir Configuration",
            category: MainMenuCategory::Configuration,
            action: MainMenuAction::ConfigView,
        },
        MainMenuItem {
            id: 1,
            label: "Éditer Configuration",
            category: MainMenuCategory::Configuration,
            action: MainMenuAction::ConfigEdit,
        },
        MainMenuItem {
            id: 2,
            label: "Configuration Réseau",
            category: MainMenuCategory::Configuration,
            action: MainMenuAction::ConfigNetwork,
        },
        MainMenuItem {
            id: 3,
            label: "Configuration Sécurité",
            category: MainMenuCategory::Configuration,
            action: MainMenuAction::ConfigSecurity,
        },
        MainMenuItem {
            id: 4,
            label: "Retour",
            category: MainMenuCategory::Configuration,
            action: MainMenuAction::ConfigurationTheme,
        },
    ]
}

pub fn get_monitoring_submenu() -> Vec<MainMenuItem> {
    vec![
        MainMenuItem {
            id: 0,
            label: "Journaux",
            category: MainMenuCategory::Monitoring,
            action: MainMenuAction::MonitoringLogs,
        },
        MainMenuItem {
            id: 1,
            label: "Métriques",
            category: MainMenuCategory::Monitoring,
            action: MainMenuAction::MonitoringMetrics,
        },
        MainMenuItem {
            id: 2,
            label: "Santé du Système",
            category: MainMenuCategory::Monitoring,
            action: MainMenuAction::MonitoringHealth,
        },
        MainMenuItem {
            id: 3,
            label: "Dashboard",
            category: MainMenuCategory::Monitoring,
            action: MainMenuAction::MonitoringDashboard,
        },
        MainMenuItem {
            id: 4,
            label: "Retour",
            category: MainMenuCategory::Monitoring,
            action: MainMenuAction::MonitoringTheme,
        },
    ]
}

pub fn get_system_submenu() -> Vec<MainMenuItem> {
    vec![
        MainMenuItem {
            id: 0,
            label: "Informations Système",
            category: MainMenuCategory::System,
            action: MainMenuAction::SystemInfo,
        },
        MainMenuItem {
            id: 1,
            label: "Services Système",
            category: MainMenuCategory::System,
            action: MainMenuAction::SystemServices,
        },
        MainMenuItem {
            id: 2,
            label: "Processus",
            category: MainMenuCategory::System,
            action: MainMenuAction::SystemProcesses,
        },
        MainMenuItem {
            id: 3,
            label: "Retour",
            category: MainMenuCategory::System,
            action: MainMenuAction::SystemTheme,
        },
    ]
}

pub fn get_lxc_manage_submenu() -> Vec<MainMenuItem> {
    vec![
        MainMenuItem {
            id: 0,
            label: "Statut du Container",
            category: MainMenuCategory::System,
            action: MainMenuAction::DeployStatus,
        },
        MainMenuItem {
            id: 1,
            label: "Démarrer Container",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCStart,
        },
        MainMenuItem {
            id: 2,
            label: "Arrêter Container",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCStop,
        },
        MainMenuItem {
            id: 3,
            label: "Redémarrer Container",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCRestart,
        },
        MainMenuItem {
            id: 4,
            label: "Logs du Container",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCLogs,
        },
        MainMenuItem {
            id: 5,
            label: "Accès Shell",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCShell,
        },
        MainMenuItem {
            id: 6,
            label: "Statistiques",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCStats,
        },
        MainMenuItem {
            id: 7,
            label: "Configuration",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCConfig,
        },
        MainMenuItem {
            id: 8,
            label: "--- Gestion RMDB ---",
            category: MainMenuCategory::System,
            action: MainMenuAction::SystemInfo, // Action placeholder
        },
        MainMenuItem {
            id: 9,
            label: "Démarrer RMDB",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCRmdbStart,
        },
        MainMenuItem {
            id: 10,
            label: "Arrêter RMDB",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCRmdbStop,
        },
        MainMenuItem {
            id: 11,
            label: "Redémarrer RMDB",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCRmdbRestart,
        },
        MainMenuItem {
            id: 12,
            label: "Logs RMDB",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCRmdbLogs,
        },
        MainMenuItem {
            id: 13,
            label: "Supprimer Container",
            category: MainMenuCategory::System,
            action: MainMenuAction::LXCDestroy,
        },
        MainMenuItem {
            id: 14,
            label: "Retour",
            category: MainMenuCategory::System,
            action: MainMenuAction::SystemTheme,
        },
    ]
}

pub fn get_containers_submenu() -> Vec<MainMenuItem> {
    vec![
        MainMenuItem {
            id: 0,
            label: "Lister",
            category: MainMenuCategory::Containers,
            action: MainMenuAction::ContainersList,
        },
        MainMenuItem {
            id: 1,
            label: "Démarrer",
            category: MainMenuCategory::Containers,
            action: MainMenuAction::ContainersStart,
        },
        MainMenuItem {
            id: 2,
            label: "Redémarrer",
            category: MainMenuCategory::Containers,
            action: MainMenuAction::ContainersRestart,
        },
        MainMenuItem {
            id: 3,
            label: "Stopper",
            category: MainMenuCategory::Containers,
            action: MainMenuAction::ContainersStop,
        },
        MainMenuItem {
            id: 4,
            label: "Ajouter",
            category: MainMenuCategory::Containers,
            action: MainMenuAction::ContainersAdd,
        },
        MainMenuItem {
            id: 5,
            label: "Supprimer",
            category: MainMenuCategory::Containers,
            action: MainMenuAction::ContainersDestroy,
        },
        MainMenuItem {
            id: 6,
            label: "Réinstaller",
            category: MainMenuCategory::Containers,
            action: MainMenuAction::ContainersReinstall,
        },
        MainMenuItem {
            id: 7,
            label: "Retour",
            category: MainMenuCategory::Containers,
            action: MainMenuAction::ContainersTheme,
        },
    ]
}

pub fn get_host_submenu() -> Vec<MainMenuItem> {
    vec![
        MainMenuItem {
            id: 0,
            label: "Installer RMDB",
            category: MainMenuCategory::Host,
            action: MainMenuAction::HostInstall,
        },
        MainMenuItem {
            id: 1,
            label: "Statut",
            category: MainMenuCategory::Host,
            action: MainMenuAction::HostStatus,
        },
        MainMenuItem {
            id: 2,
            label: "Démarrer",
            category: MainMenuCategory::Host,
            action: MainMenuAction::HostStart,
        },
        MainMenuItem {
            id: 3,
            label: "Arrêter",
            category: MainMenuCategory::Host,
            action: MainMenuAction::HostStop,
        },
        MainMenuItem {
            id: 4,
            label: "Redémarrer",
            category: MainMenuCategory::Host,
            action: MainMenuAction::HostRestart,
        },
        MainMenuItem {
            id: 5,
            label: "Activer au démarrage",
            category: MainMenuCategory::Host,
            action: MainMenuAction::HostEnable,
        },
        MainMenuItem {
            id: 6,
            label: "Désactiver au démarrage",
            category: MainMenuCategory::Host,
            action: MainMenuAction::HostDisable,
        },
        MainMenuItem {
            id: 7,
            label: "Désinstaller",
            category: MainMenuCategory::Host,
            action: MainMenuAction::HostUninstall,
        },
        MainMenuItem {
            id: 8,
            label: "Retour",
            category: MainMenuCategory::Host,
            action: MainMenuAction::HostTheme,
        },
    ]
}


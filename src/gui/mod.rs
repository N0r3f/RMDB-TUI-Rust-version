/// Module pour l'installateur graphique RMDB
/// Supporte GTK (gtk-rs) pour une interface graphique moderne

#[cfg(feature = "gui-gtk")]
pub mod gtk_installer;

#[cfg(not(feature = "gui-gtk"))]
pub mod tui_fallback;

pub mod desktop_client;

/// Type d'interface utilisateur
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UIType {
    /// Interface graphique GTK
    GTK,
    /// Interface TUI (fallback)
    TUI,
}

/// Détecte le type d'interface à utiliser
pub fn detect_ui_type() -> UIType {
    // Vérifier si on est dans un environnement graphique
    if std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok() {
        #[cfg(feature = "gui-gtk")]
        {
            // Vérifier si GTK est disponible
            if check_gtk_available() {
                return UIType::GTK;
            }
        }
    }
    
    UIType::TUI
}

#[cfg(feature = "gui-gtk")]
fn check_gtk_available() -> bool {
    // Vérifier si les bibliothèques GTK sont disponibles
    use std::process::Command;
    Command::new("pkg-config")
        .args(&["--exists", "gtk+-3.0"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(not(feature = "gui-gtk"))]
fn check_gtk_available() -> bool {
    false
}


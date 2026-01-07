/// Fallback vers l'interface TUI si l'interface graphique n'est pas disponible

/// Lance l'interface TUI
pub fn run_tui() {
    // Utiliser l'interface TUI existante
    use crate::pres::main_app::MainApp;
    let mut app = MainApp::new();
    app.run();
}


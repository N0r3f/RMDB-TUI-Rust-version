use rmdb::pres::main_app::MainApp;
use rmdb::gui::desktop_client::DesktopClient;
use rmdb::deployment::installer::InstallationMode;

fn main() {
    // Vérifier si on doit lancer le GUI
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 {
        let first_arg = &args[1];
        if first_arg == "--gui" || first_arg == "-g" || first_arg == "gui" {
            // Lancer le GUI
            #[cfg(feature = "gui-gtk")]
            {
                // Modifier temporairement les arguments pour que GTK ne voie pas --gui
                // On va créer un nouveau vecteur d'arguments sans --gui
                let mut filtered_args: Vec<String> = vec![args[0].clone()];
                for arg in args.iter().skip(1) {
                    if arg != "--gui" && arg != "-g" && arg != "gui" {
                        filtered_args.push(arg.clone());
                    }
                }
                
                // Remplacer temporairement les arguments d'environnement
                // Note: On ne peut pas modifier std::env::args() directement,
                // mais GTK va parser les arguments originaux. On va simplement lancer
                // et ignorer le warning de GTK sur --gui
                let client = DesktopClient::new(InstallationMode::DesktopGUI);
                client.run();
                return;
            }
            
            #[cfg(not(feature = "gui-gtk"))]
            {
                eprintln!("GUI non disponible : compilé sans la feature gui-gtk");
                eprintln!("Recompilez avec : cargo build --release --features gui-gtk");
                std::process::exit(1);
            }
        } else {
            // Si ce n'est pas --gui, afficher un message d'aide
            eprintln!("Option inconnue: {}", first_arg);
            eprintln!("Usage: {} [--gui|-g|gui]", args[0]);
            eprintln!("  --gui, -g, gui  : Lancer l'interface graphique");
            eprintln!("  (sans option)   : Lancer l'interface terminal (TUI)");
            std::process::exit(1);
        }
    }
    
    // Lancer le TUI par défaut (pas d'arguments)
    let mut app = MainApp::new();
    app.run();
}


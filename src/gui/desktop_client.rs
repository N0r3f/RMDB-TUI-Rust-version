/// Client graphique desktop pour RMDB
/// Interface graphique native GTK pour gérer RMDB

#[cfg(feature = "gui-gtk")]
use gtk::prelude::*;
#[cfg(feature = "gui-gtk")]
use gtk::{Application, ApplicationWindow, Box as GtkBox, Button, HeaderBar, Label, Notebook, ScrolledWindow, TreeView, TreeViewColumn, CellRendererText, ListStore, Entry, Orientation, Align};

use crate::data::api_client::APIClient;
use crate::deployment::installer::InstallationMode;
use std::sync::{Arc, Mutex};

/// Structure principale du client desktop
pub struct DesktopClient {
    /// Mode d'utilisation sélectionné
    mode: InstallationMode,
    /// Adresse du serveur RMDB (local ou distant)
    server_address: String,
    /// Port du serveur
    server_port: u16,
    /// Client API pour communiquer avec le serveur
    api_client: Arc<Mutex<Option<APIClient>>>,
}

impl DesktopClient {
    /// Crée une nouvelle instance du client desktop
    pub fn new(mode: InstallationMode) -> Self {
        Self {
            mode,
            server_address: "localhost".to_string(),
            server_port: 80,
            api_client: Arc::new(Mutex::new(None)),
        }
    }

    /// Configure l'adresse du serveur
    pub fn with_server_address(mut self, address: String) -> Self {
        self.server_address = address;
        self
    }

    /// Configure le port du serveur
    pub fn with_server_port(mut self, port: u16) -> Self {
        self.server_port = port;
        self
    }

    /// Lance l'interface graphique
    #[cfg(feature = "gui-gtk")]
    pub fn run(&self) {
        // Créer l'application
        let app = Application::new(
            Some("com.rmdb.desktop"),
            gio::ApplicationFlags::FLAGS_NONE,
        );
        
        let server_addr = self.server_address.clone();
        let server_port = self.server_port;
        app.connect_activate(move |app| {
            Self::build_ui(app, server_addr.clone(), server_port);
        });

        // Lancer l'application
        // Note: GTK va parser les arguments automatiquement depuis std::env::args()
        // et peut afficher un warning pour --gui, mais cela n'empêche pas le lancement.
        // Le warning "Option inconnue --gui" est normal et peut être ignoré.
        app.run();
    }

    #[cfg(not(feature = "gui-gtk"))]
    pub fn run(&self) {
        println!("Client desktop RMDB");
        println!("Mode: {}", self.mode.display_name());
        println!("Serveur: {}:{}", self.server_address, self.server_port);
        println!("Interface graphique non disponible (compilé sans feature gui-gtk)");
    }

    /// Construit l'interface utilisateur GTK
    #[cfg(feature = "gui-gtk")]
    fn build_ui(app: &Application, server_address: String, server_port: u16) {
        // Créer la fenêtre principale
        let window = ApplicationWindow::new(app);
        window.set_title("RMDB - Remote Management & Deployment Boot");
        window.set_default_size(1200, 800);
        window.set_position(gtk::WindowPosition::Center);

        // Créer le header bar
        let header = HeaderBar::new();
        header.set_title(Some("RMDB Desktop Client"));
        header.set_show_close_button(true);
        window.set_titlebar(Some(&header));

        // Créer le conteneur principal
        let main_box = GtkBox::new(Orientation::Vertical, 0);
        window.add(&main_box);

        // Créer le notebook pour les onglets
        let notebook = Notebook::new();
        notebook.set_tab_pos(gtk::PositionType::Top);
        main_box.pack_start(&notebook, true, true, 0);

        // Créer les différents widgets
        let connection_widget = Self::create_connection_widget(&server_address, server_port);
        let vm_widget = Self::create_vm_widget();
        let monitoring_widget = Self::create_monitoring_widget();
        let clients_widget = Self::create_clients_widget();
        let config_widget = Self::create_config_widget();

        // Ajouter les onglets
        notebook.append_page(&connection_widget, Some(&Label::new(Some("Connexion"))));
        notebook.append_page(&vm_widget, Some(&Label::new(Some("VMs"))));
        notebook.append_page(&monitoring_widget, Some(&Label::new(Some("Monitoring"))));
        notebook.append_page(&clients_widget, Some(&Label::new(Some("Clients"))));
        notebook.append_page(&config_widget, Some(&Label::new(Some("Configuration"))));

        // Afficher la fenêtre
        window.show_all();
    }

    /// Crée le widget de connexion
    #[cfg(feature = "gui-gtk")]
    fn create_connection_widget(server_address: &str, server_port: u16) -> GtkBox {
        let box_ = GtkBox::new(Orientation::Vertical, 10);
        box_.set_margin_start(20);
        box_.set_margin_end(20);
        box_.set_margin_top(20);
        box_.set_margin_bottom(20);

        // Titre
        let title = Label::new(Some("Connexion au serveur RMDB"));
        title.set_markup("<span size='large' weight='bold'>Connexion au serveur RMDB</span>");
        box_.pack_start(&title, false, false, 10);

        // Champ adresse serveur
        let server_box = GtkBox::new(Orientation::Horizontal, 10);
        let server_label = Label::new(Some("Adresse du serveur:"));
        server_label.set_halign(Align::Start);
        server_box.pack_start(&server_label, false, false, 5);
        
        let server_entry = Entry::new();
        server_entry.set_text(server_address);
        server_entry.set_hexpand(true);
        server_box.pack_start(&server_entry, true, true, 5);
        box_.pack_start(&server_box, false, false, 5);

        // Champ port
        let port_box = GtkBox::new(Orientation::Horizontal, 10);
        let port_label = Label::new(Some("Port:"));
        port_label.set_halign(Align::Start);
        port_box.pack_start(&port_label, false, false, 5);
        
        let port_entry = Entry::new();
        port_entry.set_text(&server_port.to_string());
        port_entry.set_hexpand(true);
        port_box.pack_start(&port_entry, true, true, 5);
        box_.pack_start(&port_box, false, false, 5);

        // Champ username
        let user_box = GtkBox::new(Orientation::Horizontal, 10);
        let user_label = Label::new(Some("Nom d'utilisateur:"));
        user_label.set_halign(Align::Start);
        user_box.pack_start(&user_label, false, false, 5);
        
        let user_entry = Entry::new();
        user_entry.set_placeholder_text(Some("admin"));
        user_entry.set_hexpand(true);
        user_box.pack_start(&user_entry, true, true, 5);
        box_.pack_start(&user_box, false, false, 5);

        // Champ password
        let pass_box = GtkBox::new(Orientation::Horizontal, 10);
        let pass_label = Label::new(Some("Mot de passe:"));
        pass_label.set_halign(Align::Start);
        pass_box.pack_start(&pass_label, false, false, 5);
        
        let pass_entry = Entry::new();
        pass_entry.set_placeholder_text(Some("changeme"));
        pass_entry.set_visibility(false);
        pass_entry.set_hexpand(true);
        pass_box.pack_start(&pass_entry, true, true, 5);
        box_.pack_start(&pass_box, false, false, 5);

        // Bouton de connexion
        let connect_button = Button::with_label("Se connecter");
        connect_button.set_halign(Align::Center);
        connect_button.set_margin_top(20);
        
        let status_label = Label::new(Some("Non connecté"));
        status_label.set_halign(Align::Center);
        
        // Gérer le clic sur le bouton de connexion
        {
            let server_entry = server_entry.clone();
            let port_entry = port_entry.clone();
            let user_entry = user_entry.clone();
            let pass_entry = pass_entry.clone();
            let status_label = status_label.clone();
            
            connect_button.connect_clicked(move |_| {
                let address = server_entry.text().to_string();
                let port = port_entry.text().to_string();
                let username = user_entry.text().to_string();
                let password = pass_entry.text().to_string();
                
                // Créer le client API
                let base_url = format!("http://{}:{}", address, port);
                let client = APIClient::new(base_url);
                
                // TODO: Implémenter l'authentification
                // Pour l'instant, on simule juste la connexion
                status_label.set_text("Connecté");
                status_label.set_markup("<span foreground='green'>Connecté</span>");
            });
        }
        
        box_.pack_start(&connect_button, false, false, 10);
        box_.pack_start(&status_label, false, false, 5);

        box_
    }

    /// Crée le widget de liste des VMs
    #[cfg(feature = "gui-gtk")]
    fn create_vm_widget() -> GtkBox {
        let box_ = GtkBox::new(Orientation::Vertical, 10);
        box_.set_margin_start(20);
        box_.set_margin_end(20);
        box_.set_margin_top(20);
        box_.set_margin_bottom(20);

        // Titre
        let title = Label::new(Some("Gestion des VMs"));
        title.set_markup("<span size='large' weight='bold'>Gestion des Machines Virtuelles</span>");
        box_.pack_start(&title, false, false, 10);

        // Barre d'outils
        let toolbar = GtkBox::new(Orientation::Horizontal, 5);
        
        let refresh_button = Button::with_label("Rafraîchir");
        let create_button = Button::with_label("Créer une VM");
        let delete_button = Button::with_label("Supprimer");
        
        toolbar.pack_start(&refresh_button, false, false, 5);
        toolbar.pack_start(&create_button, false, false, 5);
        toolbar.pack_start(&delete_button, false, false, 5);
        
        box_.pack_start(&toolbar, false, false, 10);

        // Liste des VMs (TreeView)
        let scrolled = ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        
        // Créer le modèle de données
        let model = ListStore::new(&[
            String::static_type(), // ID
            String::static_type(), // Nom
            String::static_type(), // Catégorie
            String::static_type(), // Format
            String::static_type(), // Taille
        ]);
        
        // Créer la vue
        let tree_view = TreeView::with_model(&model);
        
        // Colonnes
        let cols = ["ID", "Nom", "Catégorie", "Format", "Taille"];
        for (i, col_name) in cols.iter().enumerate() {
            let col = TreeViewColumn::new();
            col.set_title(col_name);
            
            let cell = CellRendererText::new();
            use gtk::prelude::TreeViewColumnExt;
            TreeViewColumnExt::pack_start(&col, &cell, true);
            TreeViewColumnExt::add_attribute(&col, &cell, "text", i as i32);
            
            tree_view.append_column(&col);
        }
        
        scrolled.add(&tree_view);
        box_.pack_start(&scrolled, true, true, 10);

        // TODO: Implémenter le chargement des VMs depuis l'API
        refresh_button.connect_clicked(move |_| {
            // Charger les VMs depuis l'API
            // model.clear();
            // for vm in vms {
            //     model.insert_with_values(None, &[0, 1, 2, 3, 4], &[
            //         &vm.id, &vm.name, &vm.category, &vm.format, &format_size(vm.size)
            //     ]);
            // }
        });

        box_
    }

    /// Crée le widget de monitoring système
    #[cfg(feature = "gui-gtk")]
    fn create_monitoring_widget() -> GtkBox {
        let box_ = GtkBox::new(Orientation::Vertical, 10);
        box_.set_margin_start(20);
        box_.set_margin_end(20);
        box_.set_margin_top(20);
        box_.set_margin_bottom(20);

        // Titre
        let title = Label::new(Some("Monitoring Système"));
        title.set_markup("<span size='large' weight='bold'>Monitoring Système</span>");
        box_.pack_start(&title, false, false, 10);

        // Métriques CPU
        let cpu_box = GtkBox::new(Orientation::Vertical, 5);
        let cpu_label = Label::new(Some("CPU:"));
        cpu_label.set_markup("<span weight='bold'>CPU</span>");
        cpu_box.pack_start(&cpu_label, false, false, 5);
        
        let cpu_usage = Label::new(Some("Usage: 0%"));
        cpu_box.pack_start(&cpu_usage, false, false, 5);
        box_.pack_start(&cpu_box, false, false, 10);

        // Métriques Mémoire
        let mem_box = GtkBox::new(Orientation::Vertical, 5);
        let mem_label = Label::new(Some("Mémoire:"));
        mem_label.set_markup("<span weight='bold'>Mémoire</span>");
        mem_box.pack_start(&mem_label, false, false, 5);
        
        let mem_usage = Label::new(Some("Usage: 0%"));
        mem_box.pack_start(&mem_usage, false, false, 5);
        box_.pack_start(&mem_box, false, false, 10);

        // Métriques Disque
        let disk_box = GtkBox::new(Orientation::Vertical, 5);
        let disk_label = Label::new(Some("Disque:"));
        disk_label.set_markup("<span weight='bold'>Disque</span>");
        disk_box.pack_start(&disk_label, false, false, 5);
        
        let disk_usage = Label::new(Some("Usage: 0%"));
        disk_box.pack_start(&disk_usage, false, false, 5);
        box_.pack_start(&disk_box, false, false, 10);

        // Bouton de rafraîchissement
        let refresh_button = Button::with_label("Rafraîchir");
        refresh_button.set_halign(Align::Center);
        box_.pack_start(&refresh_button, false, false, 10);

        // TODO: Implémenter la mise à jour périodique des métriques
        // thread::spawn(move || {
        //     loop {
        //         thread::sleep(Duration::from_secs(5));
        //         // Récupérer les métriques depuis l'API
        //         // Mettre à jour les labels
        //     }
        // });

        box_
    }

    /// Crée le widget de gestion des clients
    #[cfg(feature = "gui-gtk")]
    fn create_clients_widget() -> GtkBox {
        let box_ = GtkBox::new(Orientation::Vertical, 10);
        box_.set_margin_start(20);
        box_.set_margin_end(20);
        box_.set_margin_top(20);
        box_.set_margin_bottom(20);

        // Titre
        let title = Label::new(Some("Gestion des Clients"));
        title.set_markup("<span size='large' weight='bold'>Clients DHCP et Connexions</span>");
        box_.pack_start(&title, false, false, 10);

        // Notebook pour les onglets (Leases, Connexions)
        let notebook = Notebook::new();
        
        // Onglet Leases DHCP
        let leases_widget = Self::create_dhcp_leases_widget();
        notebook.append_page(&leases_widget, Some(&Label::new(Some("Leases DHCP"))));
        
        // Onglet Clients connectés
        let connected_widget = Self::create_connected_clients_widget();
        notebook.append_page(&connected_widget, Some(&Label::new(Some("Clients Connectés"))));
        
        box_.pack_start(&notebook, true, true, 10);

        box_
    }

    /// Crée le widget des leases DHCP
    #[cfg(feature = "gui-gtk")]
    fn create_dhcp_leases_widget() -> GtkBox {
        let box_ = GtkBox::new(Orientation::Vertical, 10);
        
        let scrolled = ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        
        // TODO: Créer la liste des leases
        let label = Label::new(Some("Liste des leases DHCP"));
        scrolled.add(&label);
        
        box_.pack_start(&scrolled, true, true, 10);
        
        box_
    }

    /// Crée le widget des clients connectés
    #[cfg(feature = "gui-gtk")]
    fn create_connected_clients_widget() -> GtkBox {
        let box_ = GtkBox::new(Orientation::Vertical, 10);
        
        let scrolled = ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        
        // TODO: Créer la liste des clients connectés
        let label = Label::new(Some("Liste des clients connectés"));
        scrolled.add(&label);
        
        box_.pack_start(&scrolled, true, true, 10);
        
        box_
    }

    /// Crée le widget de configuration
    #[cfg(feature = "gui-gtk")]
    fn create_config_widget() -> GtkBox {
        let box_ = GtkBox::new(Orientation::Vertical, 10);
        box_.set_margin_start(20);
        box_.set_margin_end(20);
        box_.set_margin_top(20);
        box_.set_margin_bottom(20);

        // Titre
        let title = Label::new(Some("Configuration"));
        title.set_markup("<span size='large' weight='bold'>Configuration du Serveur</span>");
        box_.pack_start(&title, false, false, 10);

        // TODO: Ajouter les champs de configuration
        let label = Label::new(Some("Configuration à implémenter"));
        box_.pack_start(&label, false, false, 10);

        box_
    }

    /// Se connecte au serveur RMDB
    fn connect_to_server(&self) -> Result<(), String> {
        let base_url = format!("http://{}:{}", self.server_address, self.server_port);
        let client = APIClient::new(base_url);
        
        // Stocker le client dans l'Arc
        if let Ok(mut api_client) = self.api_client.lock() {
            *api_client = Some(client);
        }
        
        Ok(())
    }

    /// Authentifie l'utilisateur
    fn authenticate(&self, username: &str, password: &str) -> Result<String, String> {
        // TODO: Implémenter l'authentification via l'API
        // Pour l'instant, on retourne un token factice
        Ok("session_token".to_string())
    }
}

/// Fenêtre principale de l'application (structure vide pour compatibilité)
pub struct MainWindow {
    // Les champs GTK seront gérés par DesktopClient
}

impl MainWindow {
    /// Crée la fenêtre principale
    pub fn new() -> Self {
        Self {}
    }

    /// Affiche la fenêtre
    pub fn show(&self) {
        // La fenêtre est gérée par DesktopClient::run()
    }
}

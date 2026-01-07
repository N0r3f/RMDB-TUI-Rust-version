use crate::pres::ui::{UI, Color};
use crate::pres::input::{InputReader, Key};
use crate::pres::terminal::RawModeGuard;
use crate::pres::sudo::SudoKeepAliveGuard;
use crate::pres::executor::{CommandExecutor, ActionMode as ExecActionMode, ExecError};
use crate::pres::main_menu::{
    get_main_menu, get_services_submenu, get_ipxe_submenu, get_clients_submenu,
    get_vms_submenu, get_configuration_submenu, get_monitoring_submenu, get_system_submenu,
    get_containers_submenu, get_host_submenu,
    MainMenuAction, MainMenuItem
};
use crate::pres::install_menu::get_mode_selection_menu;
use crate::deployment::installer::{RMDBInstaller, InstallationConfig, InstallationType, InstallationMode};
use crate::data::capabilities::Capabilities;
use crate::data::distribution::DistributionInfo;
use crate::data::api_client::{APIClient, VM, APIError, DHCPLease, ConnectedClient, SystemMetrics, IPXEEntry, VMOverlay, RepairResult, RepairProblem, TestResult, SecurityMetrics};
use crate::deployment::lxc::LXCDeployment;
use crate::deployment::host::HostDeployment;
use crate::deployment::logger::DeploymentLogger;
use std::time::Duration;
use std::io::{self, Write};

fn yesno(v: bool) -> &'static str {
    if v { "oui" } else { "non" }
}



enum MenuState {
    Main,
    SubMenu(String, Vec<MainMenuItem>),
}

pub struct MainApp {
    ui: UI,
    input_reader: InputReader,
    _raw_mode: RawModeGuard,
    sudo_keepalive: Option<SudoKeepAliveGuard>,
    selected_menu: usize,
    menu_items: Vec<&'static str>,
    menu_offset: usize,
    #[allow(dead_code)]
    last_selected_menu: usize,
    needs_full_redraw: bool,
    capabilities: Capabilities,
    action_mode: ExecActionMode,
    executor: CommandExecutor,
    menu_state: MenuState,
    #[allow(dead_code)]
    current_submenu: Option<Vec<MainMenuItem>>,
    distribution: DistributionInfo,
}

impl MainApp {
    const MIN_WIDTH: u16 = 80;
    const MIN_HEIGHT: u16 = 24;

    pub fn new() -> Self {
        let menu = get_main_menu();
        let labels: Vec<&'static str> = menu.iter().map(|m| m.label).collect();
        let capabilities = Capabilities::detect();
        let capabilities_for_executor = capabilities.clone();
        let distribution = DistributionInfo::detect();
        
        Self {
            ui: UI::new(),
            input_reader: InputReader::new(),
            _raw_mode: RawModeGuard::enable(),
            sudo_keepalive: None,
            selected_menu: 0,
            menu_items: labels,
            menu_offset: 0,
            last_selected_menu: usize::MAX,
            needs_full_redraw: true,
            capabilities,
            action_mode: ExecActionMode::Safe,
            executor: CommandExecutor::new(ExecActionMode::Safe, capabilities_for_executor),
            menu_state: MenuState::Main,
            current_submenu: None,
            distribution,
        }
    }

    pub fn run(&mut self) {
        if !self.boot_sequence() {
            return;
        }

        self.ui.update_terminal_size();
        self.render_full();
        
        loop {
            self.ui.update_terminal_size();
            if self.ui.terminal.width() < Self::MIN_WIDTH || self.ui.terminal.height() < Self::MIN_HEIGHT {
                self.show_terminal_size_warning();
                let _ = self.input_reader.read_key();
                self.needs_full_redraw = true;
                continue;
            }

            match self.input_reader.read_key() {
                Ok(Key::Quit) => {
                    match &self.menu_state {
                        MenuState::SubMenu(_, _) => {
                            self.return_to_main_menu();
                            self.render_full();
                        }
                        MenuState::Main => break,
                    }
                }
                Ok(Key::Backspace) => {
                    // Retour en arrière avec Backspace
                    match &self.menu_state {
                        MenuState::SubMenu(_, _) => {
                            self.return_to_main_menu();
                            self.render_full();
                        }
                        MenuState::Main => {
                            // Si on est déjà au menu principal, Backspace ne fait rien
                        }
                    }
                }
                Ok(Key::Up) => {
                    if self.selected_menu > 0 {
                        self.selected_menu -= 1;
                        self.update_menu_offset();
                        self.render_menu_only();
                    } else {
                        self.selected_menu = self.menu_items.len() - 1;
                        self.update_menu_offset();
                        self.render_menu_only();
                    }
                }
                Ok(Key::Down) => {
                    if self.selected_menu < self.menu_items.len() - 1 {
                        self.selected_menu += 1;
                        self.update_menu_offset();
                        self.render_menu_only();
                    } else {
                        self.selected_menu = 0;
                        self.update_menu_offset();
                        self.render_menu_only();
                    }
                }
                Ok(Key::Enter) => {
                    if !self.execute_menu() {
                        break;
                    }
                    self.needs_full_redraw = true;
                    self.render_full();
                }
                _ => {}
            }
        }
        
        self.ui.clear_screen();
        self.ui.show_cursor();
        self.ui.set_color(Color::Reset);
    }

    fn boot_sequence(&mut self) -> bool {
        self.ui.clear_screen();
        self.ui.show_cursor();
        self.ui.draw_header("RMDB - Initialisation");
        let (box_x, box_y, _, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 6;

        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "Détection des outils disponibles…");
        y += 1;
        self.ui.set_color(Color::Reset);

        let caps = &self.capabilities;
        self.ui.set_color(Color::Warning);
        self.ui.draw_text(box_x + 2, y, "Outils:");
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(
            box_x + 9,
            y,
            &format!(
                "systemctl={} rc-service={} sudo={} rmdbd={}",
                yesno(caps.has_systemctl),
                yesno(caps.has_rc_service),
                yesno(caps.has_sudo),
                yesno(caps.has_rmdbd)
            ),
        );
        y += 2;

        self.ui.set_color(Color::Warning);
        self.ui.draw_text(box_x + 2, y, "Mode:");
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 8, y, "1=Lecture seule  2=Safe  3=Admin");
        y += 1;
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "Choix (1/2/3) puis Entrée:");
        y += 2;

        if y + 2 >= box_y + box_h - 2 {
            return true;
        }

        // Choix par défaut : Mode Admin (3)
        let mut selected_mode = ExecActionMode::Admin;
        let mut choice = None;
        
        // Afficher l'instruction initiale
        self.ui.set_color(Color::Info);
        let mode_text = match selected_mode {
            ExecActionMode::ReadOnly => "▶ Mode sélectionné: 1 - Lecture seule",
            ExecActionMode::Safe => "▶ Mode sélectionné: 2 - Safe",
            ExecActionMode::Admin => "▶ Mode sélectionné: 3 - Admin (par défaut)",
        };
        self.ui.draw_text(box_x + 2, y, mode_text);
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 1, "Appuyez sur Entrée pour valider, ou 1/2/3 pour changer");
        io::stdout().flush().unwrap();
        
        while choice.is_none() {
            match self.input_reader.read_key() {
                Ok(Key::Char('1')) => {
                    selected_mode = ExecActionMode::ReadOnly;
                    // Mettre à jour l'affichage
                    self.ui.set_color(Color::Info);
                    let mode_text = "▶ Mode sélectionné: 1 - Lecture seule";
                    self.ui.draw_text(box_x + 2, y, mode_text);
                    self.ui.set_color(Color::Reset);
                }
                Ok(Key::Char('2')) => {
                    selected_mode = ExecActionMode::Safe;
                    // Mettre à jour l'affichage
                    self.ui.set_color(Color::Info);
                    let mode_text = "▶ Mode sélectionné: 2 - Safe";
                    self.ui.draw_text(box_x + 2, y, mode_text);
                    self.ui.set_color(Color::Reset);
                }
                Ok(Key::Char('3')) => {
                    selected_mode = ExecActionMode::Admin;
                    // Mettre à jour l'affichage
                    self.ui.set_color(Color::Info);
                    let mode_text = "▶ Mode sélectionné: 3 - Admin";
                    self.ui.draw_text(box_x + 2, y, mode_text);
                    self.ui.set_color(Color::Reset);
                }
                Ok(Key::Enter) => {
                    choice = Some(selected_mode);
                }
                Ok(Key::Quit) => return false,
                _ => {}
            }
        }
        
        // Nettoyer les lignes d'affichage
        self.ui.clear_line(y);
        self.ui.clear_line(y + 1);
        
        self.action_mode = choice.unwrap();
        self.executor.set_mode(self.action_mode);

        if self.action_mode == ExecActionMode::Admin {
            if !self.capabilities.has_sudo {
                self.show_error_message("Sudo absent", "Le mode Admin nécessite `sudo`, introuvable sur ce système.");
                return false;
            }
            
            // Afficher l'écran de saisie du mot de passe
            if !self.show_sudo_password_prompt("Authentification sudo", "Le mode Admin nécessite des privilèges administrateur.", "Veuillez saisir votre mot de passe sudo ci-dessous:") {
                return false;
            }

            self.sudo_keepalive = Some(SudoKeepAliveGuard::start(Duration::from_secs(60)));
        }

        self.ui.hide_cursor();
        true
    }

    fn ensure_admin(&mut self) -> bool {
        if self.action_mode != ExecActionMode::Admin {
            self.show_error_message("Mode insuffisant", "Cette action requiert le mode Admin.");
            return false;
        }
        let needs_reauth = self.sudo_keepalive.as_ref()
            .map(|ka| ka.needs_reauth())
            .unwrap_or(false);
        
        if needs_reauth {
            if !self.show_sudo_password_prompt("Ré-authentification sudo", "Votre session sudo a expiré.", "Veuillez saisir votre mot de passe sudo pour continuer:") {
                return false;
            }
            if let Some(ka) = self.sudo_keepalive.as_ref() {
                ka.clear_reauth_flag();
            }
        }
        true
    }

    fn show_sudo_password_prompt(&mut self, title: &str, message: &str, instruction: &str) -> bool {
        // Boucle de réessai jusqu'à ce que l'authentification réussisse ou que l'utilisateur annule
        loop {
            // Afficher l'interface de saisie
            self.ui.clear_screen();
            self.ui.show_cursor();
            self.ui.draw_header(title);
            
            let (box_x, box_y, box_w, _box_h) = self.ui.get_box_dimensions();
            let center_x = box_x + box_w / 2;
            let mut y = box_y + 5;
        
        // Message principal
        self.ui.set_color(Color::Info);
        let msg_lines: Vec<&str> = message.split('\n').collect();
        for line in &msg_lines {
            let line_len = line.len() as u16;
            let x = center_x.saturating_sub(line_len / 2);
            self.ui.draw_text(x, y, line);
            y += 1;
        }
        y += 2;
        
        // Instruction
        self.ui.set_color(Color::Fg);
        let inst_lines: Vec<&str> = instruction.split('\n').collect();
        for line in &inst_lines {
            let line_len = line.len() as u16;
            let x = center_x.saturating_sub(line_len / 2);
            self.ui.draw_text(x, y, line);
            y += 1;
        }
        y += 2;
        
        // Zone de saisie (indication visuelle)
        self.ui.set_color(Color::Warning);
        let prompt_text = "┌─────────────────────────────────────────┐";
        let prompt_x = center_x.saturating_sub(prompt_text.len() as u16 / 2);
        self.ui.draw_text(prompt_x, y, prompt_text);
        y += 1;
        
        // Ligne avec le prompt personnalisé - on affiche notre propre prompt
        let prompt_label = "[sudo] Mot de passe: ";
        let prompt_line = format!("│ {}│", prompt_label);
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(prompt_x, y, &prompt_line);
        
        // Positionner le curseur à l'intérieur du rectangle, après le prompt
        let password_input_x = prompt_x + 2 + prompt_label.len() as u16;
        let password_input_y = y;
        y += 1;
        
        let prompt_bottom = "└─────────────────────────────────────────┘";
        self.ui.set_color(Color::Warning);
        self.ui.draw_text(prompt_x, y, prompt_bottom);
        
        // Positionner le curseur à l'intérieur du rectangle pour la saisie
        self.ui.set_color(Color::Reset);
        self.ui.set_cursor(password_input_x, password_input_y);
        io::stdout().flush().unwrap();
        
        // Lire le mot de passe caractère par caractère (sans écho, mode raw déjà activé)
        let mut password = String::new();
        loop {
            match self.input_reader.read_key() {
                Ok(Key::Enter) => break,
                Ok(Key::Backspace) => {
                    if !password.is_empty() {
                        password.pop();
                        // Effacer le dernier astérisque
                        self.ui.set_cursor(password_input_x + password.len() as u16, password_input_y);
                        print!(" ");
                        self.ui.set_cursor(password_input_x + password.len() as u16, password_input_y);
                        io::stdout().flush().unwrap();
                    }
                }
                Ok(Key::Char(c)) => {
                    password.push(c);
                    // Afficher un astérisque pour chaque caractère
                    print!("*");
                    io::stdout().flush().unwrap();
                }
                Ok(Key::Quit) => {
                    return false;
                }
                _ => {}
            }
        }
        
        // Vérifier que le mot de passe n'est pas vide
        if password.is_empty() {
            self.ui.clear_screen();
            self.ui.draw_header(title);
            let (_bx, by, _, _) = self.ui.get_box_dimensions();
            let mut y_err = by + 6;
            self.ui.set_color(Color::Error);
            let error_msg = "✗ Mot de passe vide";
            let error_x = center_x.saturating_sub(error_msg.len() as u16 / 2);
            self.ui.draw_text(error_x, y_err, error_msg);
            y_err += 2;
            self.ui.set_color(Color::Fg);
            let error_detail = "Veuillez saisir un mot de passe.";
            let detail_x = center_x.saturating_sub(error_detail.len() as u16 / 2);
            self.ui.draw_text(detail_x, y_err, error_detail);
            y_err += 2;
            self.ui.set_color(Color::Info);
            let retry_msg = "Appuyez sur Entrée pour réessayer, ou Q pour annuler";
            let retry_x = center_x.saturating_sub(retry_msg.len() as u16 / 2);
            self.ui.draw_text(retry_x, y_err, retry_msg);
            self.ui.set_color(Color::Reset);
            io::stdout().flush().unwrap();
            match self.input_reader.read_key() {
                Ok(Key::Quit) => return false,
                _ => continue,
            }
        }
        
        // Utiliser sudo -S pour envoyer le mot de passe via stdin
        use std::process::{Command, Stdio};
        use std::io::Write;
        
        // Créer la commande avec stdin piped et stderr capturé pour détecter les erreurs
        let mut cmd = Command::new("sudo");
        cmd.args(["-S", "-v"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped()) // Capturer stdout
            .stderr(Stdio::piped()); // Capturer stderr pour détecter les erreurs
        
        // Démarrer le processus
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(_) => {
                // Erreur lors du démarrage du processus
                self.ui.clear_screen();
                self.ui.draw_header(title);
                let (_bx, by, _, _) = self.ui.get_box_dimensions();
                let mut y_err = by + 6;
                self.ui.set_color(Color::Error);
                let error_msg = "✗ Erreur lors du démarrage de sudo";
                let error_x = center_x.saturating_sub(error_msg.len() as u16 / 2);
                self.ui.draw_text(error_x, y_err, error_msg);
                y_err += 2;
                self.ui.set_color(Color::Info);
                let retry_msg = "Appuyez sur Entrée pour réessayer, ou Q pour annuler";
                let retry_x = center_x.saturating_sub(retry_msg.len() as u16 / 2);
                self.ui.draw_text(retry_x, y_err, retry_msg);
                self.ui.set_color(Color::Reset);
                io::stdout().flush().unwrap();
                match self.input_reader.read_key() {
                    Ok(Key::Quit) => return false,
                    _ => continue,
                }
            }
        };
        
        // Écrire le mot de passe dans stdin
        let write_result = if let Some(mut stdin) = child.stdin.take() {
            writeln!(stdin, "{}", password).is_ok()
        } else {
            false
        };
        
        // Si l'écriture a échoué, c'est une erreur
        if !write_result {
            let _ = child.wait(); // Nettoyer le processus
            self.ui.clear_screen();
            self.ui.draw_header(title);
            let (_bx, by, _, _) = self.ui.get_box_dimensions();
            let mut y_err = by + 6;
            self.ui.set_color(Color::Error);
            let error_msg = "✗ Erreur lors de l'envoi du mot de passe";
            let error_x = center_x.saturating_sub(error_msg.len() as u16 / 2);
            self.ui.draw_text(error_x, y_err, error_msg);
            y_err += 2;
            self.ui.set_color(Color::Info);
            let retry_msg = "Appuyez sur Entrée pour réessayer, ou Q pour annuler";
            let retry_x = center_x.saturating_sub(retry_msg.len() as u16 / 2);
            self.ui.draw_text(retry_x, y_err, retry_msg);
            self.ui.set_color(Color::Reset);
            io::stdout().flush().unwrap();
            match self.input_reader.read_key() {
                Ok(Key::Quit) => return false,
                _ => continue,
            }
        }
        
        // Attendre la fin de l'exécution et capturer stdout et stderr
        // IMPORTANT : On utilise wait() puis on récupère stderr séparément si nécessaire
        let status = match child.wait() {
            Ok(s) => s,
            Err(_) => {
                // Erreur lors de l'attente du processus
                self.ui.clear_screen();
                self.ui.draw_header(title);
                let (_bx, by, _, _) = self.ui.get_box_dimensions();
                let mut y_err = by + 6;
                self.ui.set_color(Color::Error);
                let error_msg = "✗ Erreur lors de la validation";
                let error_x = center_x.saturating_sub(error_msg.len() as u16 / 2);
                self.ui.draw_text(error_x, y_err, error_msg);
                y_err += 2;
                self.ui.set_color(Color::Info);
                let retry_msg = "Appuyez sur Entrée pour réessayer, ou Q pour annuler";
                let retry_x = center_x.saturating_sub(retry_msg.len() as u16 / 2);
                self.ui.draw_text(retry_x, y_err, retry_msg);
                self.ui.set_color(Color::Reset);
                io::stdout().flush().unwrap();
                match self.input_reader.read_key() {
                    Ok(Key::Quit) => return false,
                    _ => continue,
                }
            }
        };
        
        // Pour capturer stderr, on doit utiliser output() directement
        // Mais on a déjà utilisé spawn(), donc on doit refaire la commande pour capturer stderr
        // En fait, on peut simplement vérifier le code de sortie, qui est le critère principal
        // Si le code de sortie est 0, l'authentification a réussi
        // Si le code de sortie n'est pas 0, l'authentification a échoué
        
        // Vérifier le code de sortie explicitement (0 = succès, autre = échec)
        // CRITIQUE : Le code de sortie est le critère ABSOLU
        // - Si exit_code == 0 : le mot de passe est correct et sudo a validé le timestamp
        // - Si exit_code != 0 : le mot de passe est incorrect ou une erreur s'est produite
        let exit_code = status.code().unwrap_or(-1);
        
        // CRITIQUE : Si le code de sortie n'est pas EXACTEMENT 0, l'authentification a ÉCHOUÉ
        // Un mot de passe incorrect retourne toujours un code non-zéro (généralement 1)
        // On NE PEUT PAS continuer si exit_code != 0
        // C'est le test PRINCIPAL et le plus fiable
        if exit_code != 0 {
            // Afficher un message d'erreur explicite
            self.ui.clear_screen();
            self.ui.draw_header(title);
            let (_bx, by, _, _) = self.ui.get_box_dimensions();
            let mut y_err = by + 6;
            self.ui.set_color(Color::Error);
            let error_msg = "✗ Mot de passe incorrect";
            let error_x = center_x.saturating_sub(error_msg.len() as u16 / 2);
            self.ui.draw_text(error_x, y_err, error_msg);
            y_err += 2;
            self.ui.set_color(Color::Fg);
            let error_detail = "Le mot de passe root que vous avez saisi est incorrect.";
            let detail_x = center_x.saturating_sub(error_detail.len() as u16 / 2);
            self.ui.draw_text(detail_x, y_err, error_detail);
            y_err += 1;
            self.ui.set_color(Color::Warning);
            let error_note = "Le mode Admin nécessite le mot de passe root.";
            let note_x = center_x.saturating_sub(error_note.len() as u16 / 2);
            self.ui.draw_text(note_x, y_err, error_note);
            y_err += 2;
            self.ui.set_color(Color::Info);
            let retry_msg = "Appuyez sur Entrée pour réessayer, ou Q pour annuler";
            let retry_x = center_x.saturating_sub(retry_msg.len() as u16 / 2);
            self.ui.draw_text(retry_x, y_err, retry_msg);
            self.ui.set_color(Color::Reset);
            io::stdout().flush().unwrap();
            
            // Attendre la touche de l'utilisateur
            // IMPORTANT : On continue la boucle pour réessayer, on ne retourne PAS true
            match self.input_reader.read_key() {
                Ok(Key::Quit) => return false, // L'utilisateur annule
                Ok(Key::Enter) => continue, // Réessayer
                _ => continue, // Par défaut, réessayer
            }
        }
        
        // Test supplémentaire CRITIQUE : vérifier que sudo fonctionne vraiment avec -n (non-interactif)
        // Si le timestamp est valide, sudo -n -v devrait réussir sans demander de mot de passe
        // C'est le test le plus fiable pour vérifier que l'authentification a vraiment réussi
        // On attend un peu pour que le timestamp soit bien écrit
        std::thread::sleep(Duration::from_millis(200));
        
        // Test avec sudo -n -v pour vérifier que le timestamp est valide
        let verify_result = Command::new("sudo")
            .args(["-n", "-v"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped()) // Capturer stderr pour voir les erreurs
            .status();
        
        let verify_success = match verify_result {
            Ok(status) => {
                // Le code de sortie doit être EXACTEMENT 0
                // Si sudo -n -v réussit, cela signifie que le timestamp est valide
                // et que l'authentification précédente a vraiment fonctionné
                let verify_code = status.code().unwrap_or(-1);
                verify_code == 0
            }
            Err(_) => false,
        };
        
        // Si la vérification échoue, l'authentification n'est PAS valide
        // On ne peut PAS continuer sans une authentification valide
        // Ce test est CRITIQUE : même si sudo -S -v a retourné 0, on doit vérifier que le timestamp est valide
        if !verify_success {
            // Le test de vérification a échoué, l'authentification n'est pas valide
            // Cela signifie que le mot de passe était incorrect ou que le timestamp n'est pas valide
            self.ui.clear_screen();
            self.ui.draw_header(title);
            let (_bx, by, _, _) = self.ui.get_box_dimensions();
            let mut y_err = by + 6;
            self.ui.set_color(Color::Error);
            let error_msg = "✗ Échec de la vérification";
            let error_x = center_x.saturating_sub(error_msg.len() as u16 / 2);
            self.ui.draw_text(error_x, y_err, error_msg);
            y_err += 2;
            self.ui.set_color(Color::Fg);
            let error_detail = "L'authentification n'a pas pu être vérifiée.";
            let detail_x = center_x.saturating_sub(error_detail.len() as u16 / 2);
            self.ui.draw_text(detail_x, y_err, error_detail);
            y_err += 1;
            self.ui.set_color(Color::Warning);
            let error_note = "Le mot de passe root est requis pour le mode Admin.";
            let note_x = center_x.saturating_sub(error_note.len() as u16 / 2);
            self.ui.draw_text(note_x, y_err, error_note);
            y_err += 2;
            self.ui.set_color(Color::Info);
            let retry_msg = "Appuyez sur Entrée pour réessayer, ou Q pour annuler";
            let retry_x = center_x.saturating_sub(retry_msg.len() as u16 / 2);
            self.ui.draw_text(retry_x, y_err, retry_msg);
            self.ui.set_color(Color::Reset);
            io::stdout().flush().unwrap();
            
            // Attendre la touche de l'utilisateur
            // IMPORTANT : On continue la boucle pour réessayer, on ne retourne PAS true
            match self.input_reader.read_key() {
                Ok(Key::Quit) => return false, // L'utilisateur annule
                Ok(Key::Enter) => continue, // Réessayer
                _ => continue, // Par défaut, réessayer
            }
        }
        
        // Authentification réussie ! Les deux tests ont réussi :
        // 1. sudo -S -v a retourné 0
        // 2. sudo -n -v a réussi (timestamp valide)
        self.ui.clear_screen();
        self.ui.draw_header(title);
        let (_bx, by, _, _) = self.ui.get_box_dimensions();
        let yy = by + 6;
        self.ui.set_color(Color::Success);
        let success_msg = "✓ Authentification réussie";
        let success_x = center_x.saturating_sub(success_msg.len() as u16 / 2);
        self.ui.draw_text(success_x, yy, success_msg);
        self.ui.set_color(Color::Reset);
        io::stdout().flush().unwrap();
        std::thread::sleep(Duration::from_millis(800));
        self.ui.hide_cursor();
        return true; // Sortir de la boucle avec succès - SEULEMENT si les deux tests ont réussi
        }
    }

    fn render_full(&mut self) {
        if !self.needs_full_redraw {
            return;
        }
        self.ui.clear_screen();
        self.ui.draw_header("RMDB - Serveur de Boot Réseau");
        self.render_menu();
        self.render_status();
        self.needs_full_redraw = false;
    }

    fn render_menu_only(&mut self) {
        self.render_menu();
    }

    fn render_menu(&mut self) {
        let (box_x, box_y, box_w, box_h) = self.ui.get_box_dimensions();
        let menu_x = box_x + 2;
        let menu_y = box_y + 5;
        let menu_height = box_h - 10;
        let max_visible = self.ui.get_max_visible_items();

        let items: Vec<(usize, &str)> = match &self.menu_state {
            MenuState::Main => {
                self.menu_items.iter().enumerate().map(|(i, s)| (i, *s)).collect()
            }
            MenuState::SubMenu(_, submenu) => {
                submenu.iter().map(|m| m.label).enumerate().map(|(i, s)| (i, s)).collect()
            }
        };

        let visible_items = items.len().min(max_visible);
        let start_idx = self.menu_offset;
        let end_idx = (start_idx + visible_items).min(items.len());

        for (i, (idx, label)) in items[start_idx..end_idx].iter().enumerate() {
            let y = menu_y + i as u16;
            let selected = *idx == self.selected_menu;
            self.ui.draw_button(menu_x, y, label, selected);
        }

        if items.len() > max_visible {
            let scrollbar_x = box_w - 3;
            self.ui.draw_scrollbar(
                scrollbar_x,
                menu_y,
                menu_height,
                items.len(),
                visible_items,
                self.menu_offset,
            );
        }
    }

    fn render_status(&mut self) {
        let (_, _, _box_w, box_h) = self.ui.get_box_dimensions();
        let status_y = box_h - 3;
        let mode_str = match self.action_mode {
            ExecActionMode::ReadOnly => "Lecture seule",
            ExecActionMode::Safe => "Safe",
            ExecActionMode::Admin => "Admin",
        };
        let back_hint = match &self.menu_state {
            MenuState::SubMenu(_, _) => " | Backspace/Q: Retour",
            MenuState::Main => "",
        };
        let status_msg = format!("Mode: {} | Flèches: Navigation | Entrée: Sélectionner{} | Q: Quitter", mode_str, back_hint);
        self.ui.draw_status_bar(status_y, &status_msg);
    }

    fn update_menu_offset(&mut self) {
        let max_visible = self.ui.get_max_visible_items();
        if self.selected_menu < self.menu_offset {
            self.menu_offset = self.selected_menu;
        } else if self.selected_menu >= self.menu_offset + max_visible {
            self.menu_offset = self.selected_menu - max_visible + 1;
        }
    }

    fn execute_menu(&mut self) -> bool {
        let menu = match &self.menu_state {
            MenuState::Main => get_main_menu(),
            MenuState::SubMenu(_, submenu) => submenu.clone(),
        };

        if self.selected_menu >= menu.len() {
            return true;
        }

        let item = &menu[self.selected_menu];
        
        // Vérifier si on est dans un sous-menu et si l'action est un retour
        let is_return_action = match &self.menu_state {
            MenuState::SubMenu(category, _) => {
                match (&item.action, category.as_str()) {
                    (MainMenuAction::ServicesTheme, "Services") => true,
                    (MainMenuAction::IPXETheme, "IPXE") => true,
                    (MainMenuAction::ClientsTheme, "Clients") => true,
                    (MainMenuAction::VMsTheme, "VMs") => true,
                    (MainMenuAction::ConfigurationTheme, "Configuration") => true,
                    (MainMenuAction::MonitoringTheme, "Monitoring") => true,
                    (MainMenuAction::SystemTheme, "Système") => true,
                    // LXCManage n'est plus un sous-menu, c'est une action directe
                    (MainMenuAction::ContainersTheme, "Containers LXC") => true,
                    (MainMenuAction::HostTheme, "RMDB Hôte") => true,
                    _ => false,
                }
            }
            MenuState::Main => false,
        };
        
        // Si c'est une action de retour, revenir au menu principal
        if is_return_action {
            self.return_to_main_menu();
            return true;
        }
        
        match &item.action {
            MainMenuAction::Quit => return false,
            MainMenuAction::ServicesTheme => {
                self.menu_state = MenuState::SubMenu("Services".to_string(), get_services_submenu());
                self.selected_menu = 0;
                self.menu_offset = 0;
                self.menu_items = get_services_submenu().iter().map(|m| m.label).collect();
            }
            MainMenuAction::IPXETheme => {
                self.menu_state = MenuState::SubMenu("IPXE".to_string(), get_ipxe_submenu());
                self.selected_menu = 0;
                self.menu_offset = 0;
                self.menu_items = get_ipxe_submenu().iter().map(|m| m.label).collect();
            }
            MainMenuAction::ClientsTheme => {
                self.menu_state = MenuState::SubMenu("Clients".to_string(), get_clients_submenu());
                self.selected_menu = 0;
                self.menu_offset = 0;
                self.menu_items = get_clients_submenu().iter().map(|m| m.label).collect();
            }
            MainMenuAction::VMsTheme => {
                self.menu_state = MenuState::SubMenu("VMs".to_string(), get_vms_submenu());
                self.selected_menu = 0;
                self.menu_offset = 0;
                self.menu_items = get_vms_submenu().iter().map(|m| m.label).collect();
            }
            MainMenuAction::ConfigurationTheme => {
                self.menu_state = MenuState::SubMenu("Configuration".to_string(), get_configuration_submenu());
                self.selected_menu = 0;
                self.menu_offset = 0;
                self.menu_items = get_configuration_submenu().iter().map(|m| m.label).collect();
            }
            MainMenuAction::MonitoringTheme => {
                self.menu_state = MenuState::SubMenu("Monitoring".to_string(), get_monitoring_submenu());
                self.selected_menu = 0;
                self.menu_offset = 0;
                self.menu_items = get_monitoring_submenu().iter().map(|m| m.label).collect();
            }
            MainMenuAction::SystemTheme => {
                self.menu_state = MenuState::SubMenu("Système".to_string(), get_system_submenu());
                self.selected_menu = 0;
                self.menu_offset = 0;
                self.menu_items = get_system_submenu().iter().map(|m| m.label).collect();
            }
            // LXCManage n'est plus utilisé, remplacé par DeployStatus
            MainMenuAction::ContainersTheme => {
                self.menu_state = MenuState::SubMenu("Containers LXC".to_string(), get_containers_submenu());
                self.selected_menu = 0;
                self.menu_offset = 0;
                self.menu_items = get_containers_submenu().iter().map(|m| m.label).collect();
            }
            MainMenuAction::HostTheme => {
                self.menu_state = MenuState::SubMenu("RMDB Hôte".to_string(), get_host_submenu());
                self.selected_menu = 0;
                self.menu_offset = 0;
                self.menu_items = get_host_submenu().iter().map(|m| m.label).collect();
            }
            _ => {
                self.handle_action(&item.action);
            }
        }
        true
    }

    fn handle_action(&mut self, action: &MainMenuAction) {
        match action {
            MainMenuAction::ServiceStatus => {
                self.show_service_status();
            }
            MainMenuAction::ServiceDHCP => {
                self.show_service_info("DHCP");
            }
            MainMenuAction::ServiceDNS => {
                self.show_service_info("DNS");
            }
            MainMenuAction::ServiceTFTP => {
                self.show_service_info("TFTP");
            }
            MainMenuAction::ServiceHTTP => {
                self.show_service_info("HTTP/HTTPS");
            }
            MainMenuAction::ServiceStart => {
                if self.ensure_admin() {
                    self.start_services();
                }
            }
            MainMenuAction::ServiceStop => {
                if self.ensure_admin() {
                    self.stop_services();
                }
            }
            MainMenuAction::ServiceRestart => {
                if self.ensure_admin() {
                    self.restart_services();
                }
            }
            MainMenuAction::IPXEMenu => {
                self.show_ipxe_menu();
            }
            MainMenuAction::IPXEEntries => {
                self.show_ipxe_entries();
            }
            MainMenuAction::IPXEGenerate => {
                self.generate_ipxe_menu();
            }
            MainMenuAction::ClientsLeases => {
                self.show_dhcp_leases();
            }
            MainMenuAction::ClientsConnected => {
                self.show_connected_clients();
            }
            MainMenuAction::VMsList => {
                self.show_vms_list();
            }
            MainMenuAction::VMsCreate => {
                self.show_vm_create();
            }
            MainMenuAction::VMsManage => {
                self.show_vms_manage();
            }
            MainMenuAction::VMsOverlays => {
                self.show_vm_overlays();
            }
            MainMenuAction::ConfigView => {
                self.show_config();
            }
            MainMenuAction::MonitoringLogs => {
                self.show_logs();
            }
            MainMenuAction::MonitoringHealth => {
                self.show_health();
            }
            MainMenuAction::SystemInfo => {
                self.show_system_info();
            }
            MainMenuAction::DeployLXC => {
                if self.ensure_admin() {
                    self.deploy_lxc_container();
                }
            }
            MainMenuAction::DeployStatus => {
                self.show_deployment_status();
            }
            // LXCManage n'est plus utilisé, remplacé par DeployStatus
            MainMenuAction::LXCStart => {
                if self.ensure_admin() {
                    self.lxc_start_container();
                }
            }
            MainMenuAction::LXCStop => {
                if self.ensure_admin() {
                    self.lxc_stop_container();
                }
            }
            MainMenuAction::LXCRestart => {
                if self.ensure_admin() {
                    self.lxc_restart_container();
                }
            }
            MainMenuAction::LXCLogs => {
                self.lxc_show_logs();
            }
            MainMenuAction::LXCShell => {
                self.lxc_access_shell();
            }
            MainMenuAction::LXCStats => {
                self.lxc_show_stats();
            }
            MainMenuAction::LXCConfig => {
                self.lxc_show_config();
            }
            MainMenuAction::LXCRmdbStart => {
                if self.ensure_admin() {
                    self.lxc_rmdb_start();
                }
            }
            MainMenuAction::LXCRmdbStop => {
                if self.ensure_admin() {
                    self.lxc_rmdb_stop();
                }
            }
            MainMenuAction::LXCRmdbRestart => {
                if self.ensure_admin() {
                    self.lxc_rmdb_restart();
                }
            }
            MainMenuAction::LXCRmdbLogs => {
                self.lxc_rmdb_logs();
            }
            MainMenuAction::LXCDestroy => {
                if self.ensure_admin() {
                    self.lxc_destroy_container();
                }
            }
            MainMenuAction::ContainersList => {
                self.show_containers_list();
            }
            MainMenuAction::ContainersStart => {
                if self.ensure_admin() {
                    self.containers_start();
                }
            }
            MainMenuAction::ContainersStop => {
                if self.ensure_admin() {
                    self.containers_stop();
                }
            }
            MainMenuAction::ContainersRestart => {
                if self.ensure_admin() {
                    self.containers_restart();
                }
            }
            MainMenuAction::ContainersAdd => {
                if self.ensure_admin() {
                    self.containers_add();
                }
            }
            MainMenuAction::ContainersDestroy => {
                if self.ensure_admin() {
                    self.containers_destroy();
                }
            }
            MainMenuAction::ContainersReinstall => {
                if self.ensure_admin() {
                    self.containers_reinstall();
                }
            }
            MainMenuAction::HostInstall => {
                if self.ensure_admin() {
                    self.host_install();
                }
            }
            MainMenuAction::HostStatus => {
                self.host_status();
            }
            MainMenuAction::HostStart => {
                if self.ensure_admin() {
                    self.host_start();
                }
            }
            MainMenuAction::HostStop => {
                if self.ensure_admin() {
                    self.host_stop();
                }
            }
            MainMenuAction::HostRestart => {
                if self.ensure_admin() {
                    self.host_restart();
                }
            }
            MainMenuAction::HostEnable => {
                if self.ensure_admin() {
                    self.host_enable();
                }
            }
            MainMenuAction::HostDisable => {
                if self.ensure_admin() {
                    self.host_disable();
                }
            }
            MainMenuAction::HostUninstall => {
                if self.ensure_admin() {
                    self.host_uninstall();
                }
            }
            MainMenuAction::InstallMenu => {
                self.show_install_menu();
            }
            MainMenuAction::InstallOnHost => {
                if self.ensure_admin() {
                    self.install_on_host();
                }
            }
            MainMenuAction::InstallInContainer => {
                if self.ensure_admin() {
                    self.install_in_container();
                }
            }
            MainMenuAction::InstallInVM => {
                if self.ensure_admin() {
                    self.install_in_vm();
                }
            }
            // ContainersInfo n'est plus dans le menu simplifié
            _ => {
                self.show_message("Fonctionnalité", "Cette fonctionnalité sera implémentée prochainement.");
            }
        }
    }

    fn show_service_status(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Statut des Services RMDB");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let services = vec!["rmdbd", "dhcp", "dns", "tftp", "http"];
        for service in services {
            let cmd = if self.capabilities.has_systemctl {
                format!("systemctl is-active {} 2>/dev/null || echo inactive", service)
            } else if self.capabilities.has_rc_service {
                format!("rc-service {} status 2>/dev/null | grep -q started && echo active || echo inactive", service)
            } else {
                format!("pgrep -f {} >/dev/null && echo active || echo inactive", service)
            };

            let output = self.executor.run_shell(&cmd, false);
            let status = output.map(|o| o.stdout.trim().to_string()).unwrap_or_else(|_| "inconnu".to_string());
            
            self.ui.set_color(if status == "active" { Color::Success } else { Color::Error });
            self.ui.draw_text(box_x + 2, y, &format!("{}: {}", service, status));
            y += 1;
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn show_service_info(&mut self, service_name: &str) {
        self.show_message("Service", &format!("Informations sur le service {}", service_name));
    }

    fn start_services(&mut self) {
        let cmd = if self.capabilities.has_systemctl {
            "sudo systemctl start rmdbd"
        } else if self.capabilities.has_rc_service {
            "sudo rc-service rmdbd start"
        } else {
            "sudo /usr/local/bin/rmdbd -config /etc/rmdbd/config.json &"
        };

        let output = self.executor.run_shell(cmd, true);
        if output.is_ok() {
            self.show_message("Succès", "Services démarrés avec succès.");
        } else {
            self.show_error_message("Erreur", "Impossible de démarrer les services.");
        }
    }

    fn stop_services(&mut self) {
        let cmd = if self.capabilities.has_systemctl {
            "sudo systemctl stop rmdbd"
        } else if self.capabilities.has_rc_service {
            "sudo rc-service rmdbd stop"
        } else {
            "sudo pkill -f rmdbd"
        };

        let output = self.executor.run_shell(cmd, true);
        if output.is_ok() {
            self.show_message("Succès", "Services arrêtés avec succès.");
        } else {
            self.show_error_message("Erreur", "Impossible d'arrêter les services.");
        }
    }

    fn restart_services(&mut self) {
        let cmd = if self.capabilities.has_systemctl {
            "sudo systemctl restart rmdbd"
        } else if self.capabilities.has_rc_service {
            "sudo rc-service rmdbd restart"
        } else {
            "sudo pkill -f rmdbd && sleep 1 && sudo /usr/local/bin/rmdbd -config /etc/rmdbd/config.json &"
        };

        let output = self.executor.run_shell(cmd, true);
        if output.is_ok() {
            self.show_message("Succès", "Services redémarrés avec succès.");
        } else {
            self.show_error_message("Erreur", "Impossible de redémarrer les services.");
        }
    }

    /// Affiche le menu iPXE généré
    fn show_ipxe_menu(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Menu iPXE Généré");

        let (box_x, box_y, _box_w, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 2;

        let api_client = APIClient::new("http://localhost:8080".to_string());

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Chargement du menu iPXE...");
        y += 1;

        match api_client.get_ipxe_menu() {
            Ok(menu) => {
                self.ui.set_color(Color::Fg);
                // Afficher le menu (limité à la taille de l'écran)
                let max_lines = (box_h as usize).saturating_sub(8);
                for (i, line) in menu.lines().take(max_lines).enumerate() {
                    self.ui.draw_text(box_x + 2, y + i as u16, line);
                }

                if menu.lines().count() > max_lines {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 2, box_h - 3, &format!("... et {} ligne(s) supplémentaire(s)", menu.lines().count() - max_lines));
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur: {}", e));
                self.ui.set_color(Color::Fg);
                self.ui.draw_text(box_x + 4, y + 1, "Assurez-vous que le serveur RMDB est démarré");
            }
        }

        y = box_h - 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    /// Affiche les entrées iPXE
    fn show_ipxe_entries(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Entrées de Menu iPXE");

        let (box_x, box_y, _box_w, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 2;

        let api_client = APIClient::new("http://localhost:8080".to_string());

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Chargement des entrées...");
        y += 1;

        match api_client.get_ipxe_entries() {
            Ok(entries) => {
                if entries.is_empty() {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 2, y, "Aucune entrée iPXE trouvée.");
                    y += 2;
                } else {
                    self.ui.set_color(Color::Fg);
                    self.ui.draw_text(box_x + 2, y, &format!("Total: {} entrée(s)", entries.len()));
                    y += 2;

                    let max_items = (box_h as usize).saturating_sub(8).min(entries.len());
                    for (i, entry) in entries.iter().take(max_items).enumerate() {
                        self.ui.set_color(Color::Fg);
                        self.ui.draw_text(box_x + 2, y, &format!("{}. {}", i + 1, entry.name));
                        y += 1;
                        self.ui.set_color(Color::Info);
                        if let Some(ref desc) = entry.description {
                            self.ui.draw_text(box_x + 4, y, &format!("Description: {}", desc));
                            y += 1;
                        }
                        self.ui.draw_text(box_x + 4, y, &format!("Type: {} | Activé: {}", entry.menu_type, if entry.enabled { "Oui" } else { "Non" }));
                        y += 1;
                        if let Some(ref target) = entry.boot_target {
                            self.ui.draw_text(box_x + 4, y, &format!("Cible de boot: {}", target));
                            y += 1;
                        }
                        y += 1;
                    }

                    if entries.len() > max_items {
                        self.ui.set_color(Color::Warning);
                        self.ui.draw_text(box_x + 2, y, &format!("... et {} entrée(s) supplémentaire(s)", entries.len() - max_items));
                        y += 1;
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    /// Génère le menu iPXE
    fn generate_ipxe_menu(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Génération du Menu iPXE");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let api_client = APIClient::new("http://localhost:8080".to_string());

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Génération du menu iPXE en cours...");
        y += 2;

        match api_client.generate_ipxe_menu() {
            Ok(result) => {
                self.ui.set_color(Color::Success);
                self.ui.draw_text(box_x + 2, y, "✓ Menu iPXE généré avec succès !");
                y += 1;
                self.ui.set_color(Color::Fg);
                // Afficher un aperçu si disponible
                if !result.is_empty() {
                    self.ui.draw_text(box_x + 2, y + 1, "Aperçu:");
                    y += 2;
                    for line in result.lines().take(5) {
                        self.ui.draw_text(box_x + 4, y, line);
                        y += 1;
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur lors de la génération: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn show_dhcp_leases(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Leases DHCP");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let cmd = "cat /var/lib/dhcp/dhcpd.leases 2>/dev/null | head -20 || echo 'Aucun lease trouvé'";
        let output = self.executor.run_shell(cmd, false);
        let content = output.map(|o| o.stdout).unwrap_or_else(|_| "Erreur lors de la lecture des leases".to_string());

        for line in content.lines().take(15) {
            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 2, y, line);
            y += 1;
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn show_connected_clients(&mut self) {
        self.show_message("Clients Connectés", "Liste des clients connectés");
    }

    /// Affiche la configuration RMDB (version améliorée avec API)
    fn show_config(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Configuration RMDB");

        let (box_x, box_y, _box_w, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 2;

        let api_client = APIClient::new("http://localhost:8080".to_string());

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Chargement de la configuration...");
        y += 1;

        match api_client.get_config() {
            Ok(config) => {
                // Afficher la configuration formatée
                let config_str = serde_json::to_string_pretty(&config)
                    .unwrap_or_else(|_| "Erreur de formatage".to_string());
                
                self.ui.set_color(Color::Fg);
                let max_lines = (box_h as usize).saturating_sub(8);
                for (i, line) in config_str.lines().take(max_lines).enumerate() {
                    self.ui.draw_text(box_x + 2, y + i as u16, line);
                }

                if config_str.lines().count() > max_lines {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 2, box_h - 3, &format!("... et {} ligne(s) supplémentaire(s)", config_str.lines().count() - max_lines));
                }
            }
            Err(e) => {
                // Fallback vers méthode locale
                self.ui.set_color(Color::Warning);
                self.ui.draw_text(box_x + 2, y, "API non disponible, utilisation de la méthode locale...");
                y += 2;
                self.show_config_local();
                return;
            }
        }

        y = box_h - 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    /// Affiche la configuration (méthode locale de fallback)
    fn show_config_local(&mut self) {
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let config_path = "/etc/rmdbd/config.json";
        let cmd = format!("cat {} 2>/dev/null | head -30 || echo 'Fichier de configuration non trouvé'", config_path);
        let output = self.executor.run_shell(&cmd, false);
        let content = output.map(|o| o.stdout).unwrap_or_else(|_| "Erreur lors de la lecture de la configuration".to_string());

        for line in content.lines().take(25) {
            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 2, y, line);
            y += 1;
        }
    }

    /// Édite la configuration
    fn edit_config(&mut self) {
        self.show_message("Configuration", "Édition de configuration - À implémenter (nécessite éditeur de texte)");
    }

    /// Affiche la configuration réseau
    fn show_network_config(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Configuration Réseau");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 3;

        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "Services réseau:");
        y += 2;

        let services = vec![
            ("DHCP", "Port 67"),
            ("DNS", "Port 53"),
            ("TFTP", "Port 69"),
            ("HTTP", "Port 8080"),
            ("NBD", "Port 10809"),
        ];

        for (service, port) in services {
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 4, y, &format!("{}: {}", service, port));
            y += 1;
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    /// Affiche la configuration de sécurité
    fn show_security_config(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Configuration Sécurité");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 2;

        let api_client = APIClient::new("http://localhost:8080".to_string());

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Chargement des métriques de sécurité...");
        y += 1;

        match api_client.get_security_metrics() {
            Ok(metrics) => {
                self.ui.set_color(Color::Fg);
                self.ui.draw_text(box_x + 2, y, "Métriques de sécurité:");
                y += 2;

                self.ui.set_color(Color::Info);
                self.ui.draw_text(box_x + 4, y, &format!("Menaces détectées: {}", metrics.threats_detected));
                y += 1;
                self.ui.draw_text(box_x + 4, y, &format!("Menaces actives: {}", metrics.active_threats));
                y += 1;
                self.ui.draw_text(box_x + 4, y, &format!("IPs bloquées: {}", metrics.blocked_ips));
                y += 1;
                self.ui.draw_text(box_x + 4, y, &format!("Tentatives de connexion échouées: {}", metrics.failed_logins));
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn show_logs(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Journaux RMDB");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let log_path = "/var/log/rmdbd.log";
        let cmd = format!("tail -20 {} 2>/dev/null || echo 'Fichier de log non trouvé'", log_path);
        let output = self.executor.run_shell(&cmd, false);
        let content = output.map(|o| o.stdout).unwrap_or_else(|_| "Erreur lors de la lecture des logs".to_string());

        for line in content.lines().take(20) {
            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 2, y, line);
            y += 1;
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn show_health(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Santé du Système");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 3;

        let api_client = APIClient::new("http://localhost:8080".to_string());

        // Vérifier les problèmes de réparation
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Vérification des problèmes système...");
        y += 1;

        match api_client.get_repair_problems() {
            Ok(problems) => {
                if problems.is_empty() {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "✓ Aucun problème détecté. Système en bonne santé.");
                    y += 2;
                } else {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 2, y, &format!("⚠ {} problème(s) détecté(s):", problems.len()));
                    y += 2;

                    for problem in problems.iter().take(5) {
                        self.ui.set_color(Color::Fg);
                        self.ui.draw_text(box_x + 4, y, &format!("- [{}] {}", problem.severity, problem.description));
                        y += 1;
                        self.ui.set_color(Color::Info);
                        self.ui.draw_text(box_x + 6, y, &format!("Catégorie: {}", problem.category));
                        y += 1;
                    }

                    if problems.len() > 5 {
                        self.ui.set_color(Color::Warning);
                        self.ui.draw_text(box_x + 2, y, &format!("... et {} problème(s) supplémentaire(s)", problems.len() - 5));
                        y += 1;
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur lors de la vérification: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn show_system_info(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Informations Système");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let commands = vec![
            ("Hostname", "hostname"),
            ("Uptime", "uptime"),
            ("CPU", "nproc"),
            ("Memory", "free -h | head -2"),
        ];

        for (label, cmd) in commands {
            let output = self.executor.run_shell(cmd, false);
            let value = output.map(|o| o.stdout.trim().to_string()).unwrap_or_else(|_| "N/A".to_string());
            self.ui.set_color(Color::Warning);
            self.ui.draw_text(box_x + 2, y, &format!("{}: ", label));
            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 12, y, &value);
            y += 1;
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn return_to_main_menu(&mut self) {
        let menu = get_main_menu();
        self.menu_items = menu.iter().map(|m| m.label).collect();
        self.menu_state = MenuState::Main;
        self.selected_menu = 0;
        self.menu_offset = 0;
    }

    fn show_message(&mut self, title: &str, message: &str) {
        self.ui.clear_screen();
        self.ui.draw_header(title);
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, box_y + 5, message);
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, box_y + 7, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn show_error_message(&mut self, title: &str, message: &str) {
        self.ui.clear_screen();
        self.ui.draw_header(title);
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        self.ui.set_color(Color::Error);
        self.ui.draw_text(box_x + 2, box_y + 5, message);
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, box_y + 7, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn show_terminal_size_warning(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Taille du Terminal");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        self.ui.set_color(Color::Warning);
        self.ui.draw_text(box_x + 2, box_y + 5, &format!("Terminal trop petit: {}x{}", self.ui.terminal.width(), self.ui.terminal.height()));
        self.ui.draw_text(box_x + 2, box_y + 6, &format!("Minimum requis: {}x{}", Self::MIN_WIDTH, Self::MIN_HEIGHT));
        self.ui.set_color(Color::Reset);
    }

    fn ask_yes_no(&mut self, title: &str, question: &str) -> bool {
        self.ui.clear_screen();
        self.ui.draw_header(title);
        let (box_x, box_y, _box_w, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;
        
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, question);
        y += 2;
        
        let mut selected = 0; // 0 = Oui, 1 = Non
        let options = vec!["Oui", "Non"];
        
        loop {
            // Effacer la zone des options
            for i in 0..options.len() {
                self.ui.clear_line(y + i as u16);
            }
            
            // Afficher les options
            for (i, option) in options.iter().enumerate() {
                let selected_char = if i == selected { "▶" } else { " " };
                self.ui.set_color(if i == selected { Color::Selection } else { Color::Fg });
                self.ui.draw_text(box_x + 4, y + i as u16, &format!("{} {}", selected_char, option));
            }
            self.ui.set_color(Color::Reset);
            
            // Instructions
            self.ui.clear_line(box_h - 2);
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, box_h - 2, "Flèches: Sélectionner | Entrée: Valider | Q: Annuler");
            self.ui.set_color(Color::Reset);
            
            match self.input_reader.read_key() {
                Ok(Key::Up) => {
                    if selected > 0 {
                        selected -= 1;
                    } else {
                        selected = options.len() - 1;
                    }
                }
                Ok(Key::Down) => {
                    if selected < options.len() - 1 {
                        selected += 1;
                    } else {
                        selected = 0;
                    }
                }
                Ok(Key::Enter) => {
                    return selected == 0; // true si "Oui", false si "Non"
                }
                Ok(Key::Quit) => {
                    return false;
                }
                _ => {}
            }
        }
    }

    fn install_lxc_templates(&mut self) -> bool {
        self.ui.clear_screen();
        self.ui.draw_header("Installation des Templates LXC");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;
        
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, &format!("Distribution détectée: {}", self.distribution.distro));
        if let Some(ref version) = self.distribution.version {
            self.ui.draw_text(box_x + 2, y + 1, &format!("Version: {}", version));
            y += 1;
        }
        y += 2;
        
        // Préparer les commandes d'installation
        let packages = vec!["lxc-templates"];
        let update_cmd = format!("sudo {}", self.distribution.update_command());
        let install_cmd = format!("sudo {}", self.distribution.install_command(&packages.iter().map(|s| *s).collect::<Vec<_>>()));
        
        // Mettre à jour les dépôts d'abord
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Mise à jour des dépôts...");
        y += 1;
        let _ = self.executor.run_shell(&update_cmd, true);
        y += 1;
        
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Installation des templates LXC en cours...");
        y += 1;
        self.ui.set_color(Color::Reset);
        
        // Installer les templates
        let output = self.executor.run_shell(&install_cmd, true);
        
        if output.is_ok() && output.as_ref().unwrap().exit_code == Some(0) {
            self.ui.set_color(Color::Success);
            self.ui.draw_text(box_x + 2, y, "Templates LXC installés avec succès!");
            y += 2;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return true;
        } else {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Échec de l'installation des templates LXC.");
            if let Ok(err) = output {
                if !err.stderr.is_empty() {
                    let error_msg = err.stderr.lines().take(3).collect::<Vec<_>>().join("\n");
                    self.ui.draw_text(box_x + 2, y + 1, &error_msg);
                }
            }
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return false;
        }
    }

    fn install_lxc(&mut self) -> bool {
        self.ui.clear_screen();
        self.ui.draw_header("Installation de LXC");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;
        
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, &format!("Distribution détectée: {}", self.distribution.distro));
        if let Some(ref version) = self.distribution.version {
            self.ui.draw_text(box_x + 2, y + 1, &format!("Version: {}", version));
            y += 1;
        }
        y += 2;
        
        // Préparer les commandes d'installation
        let packages = vec!["lxc", "lxc-templates"];
        let update_cmd = format!("sudo {}", self.distribution.update_command());
        let install_cmd = format!("sudo {}", self.distribution.install_command(&packages.iter().map(|s| *s).collect::<Vec<_>>()));
        
        // Mettre à jour les dépôts d'abord
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Mise à jour des dépôts...");
        y += 1;
        let _ = self.executor.run_shell(&update_cmd, true);
        y += 1;
        
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Installation de LXC en cours...");
        y += 1;
        self.ui.set_color(Color::Reset);
        
        // Installer LXC
        let output = self.executor.run_shell(&install_cmd, true);
        
        if output.is_ok() && output.as_ref().unwrap().exit_code == Some(0) {
            self.ui.set_color(Color::Success);
            self.ui.draw_text(box_x + 2, y, "LXC installé avec succès!");
            y += 2;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return true;
        } else {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Échec de l'installation de LXC.");
            if let Ok(err) = output {
                if !err.stderr.is_empty() {
                    self.ui.draw_text(box_x + 2, y + 1, &format!("Erreur: {}", err.stderr.lines().next().unwrap_or("Inconnue")));
                }
            }
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return false;
        }
    }

    fn deploy_lxc_container(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Déploiement Container LXC Alpine");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        // Vérifier LXC
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Vérification de LXC...");
        y += 1;

        // Créer le logger de déploiement
        let logger = match DeploymentLogger::new() {
            Ok(l) => {
                l.info("=== Début du déploiement LXC ===");
                l.info("Container: rmdb, Alpine: 3.20");
                Some(l)
            }
            Err(e) => {
                eprintln!("Avertissement: Impossible de créer le logger: {}", e);
                None
            }
        };
        
        let lxc_deploy = if let Some(logger) = logger {
            LXCDeployment::new("rmdb".to_string(), "3.20".to_string())
                .with_logger(logger)
        } else {
            LXCDeployment::new("rmdb".to_string(), "3.20".to_string())
        };
        
        if !lxc_deploy.check_lxc_installed() {
            self.ui.set_color(Color::Warning);
            self.ui.draw_text(box_x + 2, y, "LXC n'est pas installé.");
            y += 2;
            self.ui.set_color(Color::Reset);
            
            // Proposer l'installation
            let install_lxc = self.ask_yes_no(
                "Installation de LXC",
                "LXC n'est pas installé. Voulez-vous l'installer automatiquement ?"
            );
            
            if install_lxc {
                if !self.install_lxc() {
                    return; // Échec de l'installation
                }
                // Vérifier à nouveau après installation
                if !lxc_deploy.check_lxc_installed() {
                    self.show_error_message("Erreur", "LXC n'a pas pu être installé ou détecté.");
                    return;
                }
            } else {
                // L'utilisateur a refusé l'installation
                return;
            }
        }

        self.ui.set_color(Color::Success);
        self.ui.draw_text(box_x + 2, y, "LXC est installé.");
        y += 2;

        // Vérifier les templates LXC
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Vérification des templates LXC...");
        y += 1;
        
        if !lxc_deploy.check_lxc_templates() {
            self.ui.set_color(Color::Warning);
            self.ui.draw_text(box_x + 2, y, "Les templates LXC ne sont pas installés.");
            self.ui.set_color(Color::Reset);
            
            // Proposer l'installation des templates
            let install_templates = self.ask_yes_no(
                "Installation des Templates LXC",
                "Les templates LXC ne sont pas installés. Voulez-vous les installer automatiquement ?"
            );
            
            if install_templates {
                if !self.install_lxc_templates() {
                    return; // Échec de l'installation
                }
                
                // Attendre un peu pour que les fichiers soient écrits
                std::thread::sleep(std::time::Duration::from_secs(1));
                
                // Vérifier à nouveau après installation avec plusieurs tentatives
                let mut templates_ok = false;
                for attempt in 1..=3 {
                    if lxc_deploy.check_lxc_templates() {
                        templates_ok = true;
                        break;
                    }
                    if attempt < 3 {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                }
                
                if !templates_ok {
                    // Afficher un avertissement mais continuer quand même
                    // Parfois les templates sont installés mais la détection échoue
                    // (surtout sur RHEL où les emplacements peuvent différer)
                    self.ui.clear_screen();
                    self.ui.draw_header("Déploiement Container LXC Alpine");
                    y = box_y + 5;
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 2, y, "Les templates LXC ont été installés.");
                    self.ui.draw_text(box_x + 2, y + 1, "La détection automatique a échoué, mais nous continuons.");
                    self.ui.draw_text(box_x + 2, y + 2, "Si la création échoue, les templates peuvent être à un emplacement non standard.");
                    y += 4;
                    std::thread::sleep(std::time::Duration::from_secs(2));
                } else {
                    // Continuer avec la création
                    self.ui.clear_screen();
                    self.ui.draw_header("Déploiement Container LXC Alpine");
                    y = box_y + 5;
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "Templates LXC installés et détectés.");
                    y += 2;
                }
            } else {
                // L'utilisateur a refusé l'installation
                return;
            }
        } else {
            self.ui.set_color(Color::Success);
            self.ui.draw_text(box_x + 2, y, "Templates LXC disponibles.");
            y += 2;
        }

        // Vérifier si le container existe (avec executor pour utiliser sudo)
        if lxc_deploy.check_container_exists_with_executor(&self.executor) {
            self.ui.set_color(Color::Warning);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' existe déjà.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 1, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        // Créer le container
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Création du container Alpine Linux 3.20...");
        y += 1;
        self.ui.set_color(Color::Reset);

        match lxc_deploy.create_container(&self.executor) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "Container créé avec succès!");
                    y += 2;
                    
                    // Vérifier immédiatement que le container existe avec diagnostic détaillé
                    self.ui.set_color(Color::Info);
                    self.ui.draw_text(box_x + 2, y, "Vérification détaillée de l'existence du container...");
                    y += 1;
                    io::stdout().flush().unwrap();
                    
                    // Attendre un peu pour que le système de fichiers soit à jour
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    
                    // Vérification stricte : le container DOIT exister avant de continuer
                    let container_exists = lxc_deploy.check_container_exists_with_executor(&self.executor);
                    if !container_exists {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, y, "✗ ERREUR: Le container n'a pas été créé correctement!");
                        y += 1;
                        self.ui.set_color(Color::Fg);
                        self.ui.draw_text(box_x + 2, y, "La commande lxc-create a réussi mais le container n'est pas détectable.");
                        y += 1;
                        self.ui.draw_text(box_x + 2, y, "Vérifiez les logs système et les permissions LXC.");
                        y += 2;
                        self.ui.set_color(Color::Info);
                        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                        self.ui.set_color(Color::Reset);
                        io::stdout().flush().unwrap();
                        let _ = self.input_reader.read_key();
                        return;
                    }
                    
                    // Diagnostic détaillé
                    self.show_container_diagnostic(&lxc_deploy, box_x, &mut y);
                    
                    // Démarrer le container
                    self.ui.set_color(Color::Info);
                    self.ui.draw_text(box_x + 2, y, "Démarrage du container...");
                    y += 1;
                    
                    match lxc_deploy.start_container(&self.executor) {
                        Ok(_) => {
                            self.ui.set_color(Color::Success);
                            self.ui.draw_text(box_x + 2, y, "Container démarré!");
                            y += 2;
                            
                            // Attendre que le container soit prêt
                            self.ui.set_color(Color::Info);
                            self.ui.draw_text(box_x + 2, y, "Attente que le container soit prêt...");
                            y += 1;
                            std::thread::sleep(std::time::Duration::from_secs(3));
                            
                            // Vérification complète du container
                            self.ui.set_color(Color::Info);
                            self.ui.draw_text(box_x + 2, y, "Vérification complète du container...");
                            y += 1;
                            io::stdout().flush().unwrap();
                            
                            match lxc_deploy.verify_container(&self.executor) {
                                Ok(verification) => {
                                    let mut all_ok = true;
                                    
                                    // Vérifier chaque aspect
                                    if !verification.exists {
                                        self.ui.set_color(Color::Error);
                                        self.ui.draw_text(box_x + 2, y, "✗ Container non détecté par check_container_exists()");
                                        y += 1;
                                        all_ok = false;
                                    }
                                    
                                    if !verification.detectable_by_ls && !verification.detectable_by_list {
                                        self.ui.set_color(Color::Warning);
                                        self.ui.draw_text(box_x + 2, y, "⚠ Container non détecté par lxc-ls ou lxc list");
                                        y += 1;
                                        all_ok = false;
                                    }
                                    
                                    if !verification.detectable_by_filesystem {
                                        self.ui.set_color(Color::Warning);
                                        self.ui.draw_text(box_x + 2, y, "⚠ Container non trouvé dans le système de fichiers");
                                        y += 1;
                                        all_ok = false;
                                    }
                                    
                                    if !verification.can_get_status {
                                        self.ui.set_color(Color::Error);
                                        self.ui.draw_text(box_x + 2, y, "✗ Impossible d'obtenir le statut du container");
                                        y += 1;
                                        all_ok = false;
                                    }
                                    
                                    if !verification.can_attach {
                                        self.ui.set_color(Color::Error);
                                        self.ui.draw_text(box_x + 2, y, "✗ Impossible d'accéder au container via lxc-attach");
                                        y += 1;
                                        all_ok = false;
                                    }
                                    
                                    if !verification.is_running {
                                        self.ui.set_color(Color::Warning);
                                        self.ui.draw_text(box_x + 2, y, "⚠ Container créé mais n'est pas en cours d'exécution");
                                        y += 1;
                                        // Essayer de le démarrer à nouveau
                                        self.ui.set_color(Color::Info);
                                        self.ui.draw_text(box_x + 2, y, "Tentative de démarrage...");
                                        y += 1;
                                        let _ = lxc_deploy.start_container(&self.executor);
                                        std::thread::sleep(std::time::Duration::from_secs(2));
                                    }
                                    
                                    // Afficher les erreurs détaillées si présentes
                                    if !verification.errors.is_empty() {
                                        self.ui.set_color(Color::Warning);
                                        self.ui.draw_text(box_x + 2, y, "Détails des problèmes:");
                                        y += 1;
                                        for error in verification.errors.iter().take(5) {
                                            self.ui.set_color(Color::Fg);
                                            self.ui.draw_text(box_x + 4, y, &format!("- {}", error));
                                            y += 1;
                                        }
                                    }
                                    
                                    if all_ok && verification.is_running && verification.can_attach {
                                        self.ui.set_color(Color::Success);
                                        self.ui.draw_text(box_x + 2, y, "✓ Container vérifié et opérationnel!");
                                        y += 2;
                                    } else {
                                        // Si les vérifications critiques échouent, arrêter le processus
                                        if !verification.exists || !verification.can_attach {
                                            self.ui.set_color(Color::Error);
                                            self.ui.draw_text(box_x + 2, y, "✗ ERREUR CRITIQUE: Le container n'est pas utilisable!");
                                            y += 1;
                                            self.ui.set_color(Color::Fg);
                                            self.ui.draw_text(box_x + 2, y, "Le déploiement ne peut pas continuer.");
                                            y += 1;
                                            self.ui.draw_text(box_x + 2, y, "Veuillez vérifier la configuration LXC et réessayer.");
                                            y += 2;
                                            self.ui.set_color(Color::Info);
                                            self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                                            self.ui.set_color(Color::Reset);
                                            io::stdout().flush().unwrap();
                                            let _ = self.input_reader.read_key();
                                            return;
                                        }
                                        
                                        self.ui.set_color(Color::Warning);
                                        self.ui.draw_text(box_x + 2, y, "⚠ Container créé mais certains tests ont échoué");
                                        y += 1;
                                        self.ui.draw_text(box_x + 2, y, "Le container peut ne pas apparaître dans la liste");
                                        y += 2;
                                    }
                                }
                                Err(e) => {
                                    self.ui.set_color(Color::Error);
                                    self.ui.draw_text(box_x + 2, y, &format!("Erreur lors de la vérification: {}", e));
                                    y += 2;
                                }
                            }
                            
                            // Installer RMDB dans le container
                            // Chercher rmdb_source dans plusieurs emplacements (FHS + développement)
                            let current_dir = std::env::current_dir().unwrap_or_default();
                            
                            let rmdb_source_str = {
                                let mut found_path: Option<String> = None;
                                
                                // 1. Emplacement de développement (dossier courant)
                                let dev_path = current_dir.join("rmdb_source");
                                if dev_path.exists() {
                                    found_path = Some(dev_path.to_string_lossy().to_string());
                                }
                                
                                // 2. Emplacement de développement (répertoire parent)
                                if found_path.is_none() {
                                    let parent_dir = current_dir.parent().unwrap_or(&current_dir);
                                    let parent_rmdb_source = parent_dir.join("rmdb_source");
                                    if parent_rmdb_source.exists() {
                                        found_path = Some(parent_rmdb_source.to_string_lossy().to_string());
                                    }
                                }
                                
                                // 3. Emplacement FHS standard: /usr/local/share/rmdb/rmdb_source
                                if found_path.is_none() {
                                    let fhs_path = std::path::Path::new("/usr/local/share/rmdb/rmdb_source");
                                    if fhs_path.exists() {
                                        found_path = Some(fhs_path.to_string_lossy().to_string());
                                    }
                                }
                                
                                // 4. Emplacement alternatif: /opt/rmdb/rmdb_source
                                if found_path.is_none() {
                                    let opt_path = std::path::Path::new("/opt/rmdb/rmdb_source");
                                    if opt_path.exists() {
                                        found_path = Some(opt_path.to_string_lossy().to_string());
                                    }
                                }
                                
                                match found_path {
                                    Some(path) => path,
                                    None => {
                                        self.ui.set_color(Color::Error);
                                        self.ui.draw_text(box_x + 2, y, "rmdb_source introuvable.");
                                        y += 1;
                                        self.ui.set_color(Color::Info);
                                        self.ui.draw_text(box_x + 2, y, "Emplacements vérifiés:");
                                        y += 1;
                                        self.ui.draw_text(box_x + 4, y, &format!("- {}", current_dir.join("rmdb_source").display()));
                                        y += 1;
                                        self.ui.draw_text(box_x + 4, y, "- /usr/local/share/rmdb/rmdb_source");
                                        y += 1;
                                        self.ui.draw_text(box_x + 4, y, "- /opt/rmdb/rmdb_source");
                                        y += 2;
                                        self.ui.set_color(Color::Reset);
                                        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                                        let _ = self.input_reader.read_key();
                                        return;
                                    }
                                }
                            };
                            
                            self.ui.set_color(Color::Info);
                            self.ui.draw_text(box_x + 2, y, &format!("Installation de RMDB depuis: {}", rmdb_source_str));
                            y += 1;
                            
                            match lxc_deploy.install_rmdb_in_container(&self.executor, &rmdb_source_str) {
                                Ok(_) => {
                                    self.ui.set_color(Color::Success);
                                    self.ui.draw_text(box_x + 2, y, "RMDB installé avec succès!");
                                    y += 2;
                                    
                                    // Vérification finale : s'assurer que le container apparaît dans la liste
                                    self.ui.set_color(Color::Info);
                                    self.ui.draw_text(box_x + 2, y, "Vérification finale de la détection du container...");
                                    y += 1;
                                    io::stdout().flush().unwrap();
                                    
                                    // Attendre un peu pour que le système soit à jour
                                    std::thread::sleep(std::time::Duration::from_secs(1));
                                    
                                    match LXCDeployment::list_all_containers(&self.executor) {
                                        Ok(containers) => {
                                            let found = containers.iter().any(|c| c.name == "rmdb");
                                            if found {
                                                self.ui.set_color(Color::Success);
                                                self.ui.draw_text(box_x + 2, y, "✓ Container 'rmdb' détecté dans la liste des containers");
                                                y += 2;
                                            } else {
                                                self.ui.set_color(Color::Error);
                                                self.ui.draw_text(box_x + 2, y, "✗ ERREUR: Container 'rmdb' non trouvé dans la liste!");
                                                y += 1;
                                                self.ui.set_color(Color::Fg);
                                                self.ui.draw_text(box_x + 2, y, &format!("Containers trouvés: {}", containers.len()));
                                                y += 1;
                                                
                                                // Afficher les containers trouvés pour debug
                                                if !containers.is_empty() {
                                                    self.ui.set_color(Color::Info);
                                                    self.ui.draw_text(box_x + 2, y, "Containers détectés:");
                                                    y += 1;
                                                    for container in containers.iter().take(5) {
                                                        self.ui.set_color(Color::Fg);
                                                        self.ui.draw_text(box_x + 4, y, &format!("- {} ({})", container.name, container.status));
                                                        y += 1;
                                                    }
                                                } else {
                                                    self.ui.set_color(Color::Warning);
                                                    self.ui.draw_text(box_x + 2, y, "Aucun container détecté par list_all_containers()");
                                                    y += 1;
                                                }
                                                y += 1;
                                                self.ui.set_color(Color::Warning);
                                                self.ui.draw_text(box_x + 2, y, "Le container a été créé mais n'est pas détectable.");
                                                y += 1;
                                                self.ui.draw_text(box_x + 2, y, "Vérifiez les permissions LXC et la configuration système.");
                                                y += 2;
                                            }
                                        }
                                        Err(e) => {
                                            self.ui.set_color(Color::Warning);
                                            self.ui.draw_text(box_x + 2, y, &format!("⚠ Impossible de lister les containers: {}", e));
                                            y += 2;
                                        }
                                    }
                                    
                                    // Afficher le chemin du log si disponible
                                    if let Some(ref logger) = lxc_deploy.logger {
                                        self.ui.set_color(Color::Info);
                                        self.ui.draw_text(box_x + 2, y, &format!("Logs disponibles dans: {}", logger.log_path().display()));
                                        y += 1;
                                    }
                                    
                                    self.ui.set_color(Color::Info);
                                    self.ui.draw_text(box_x + 2, y, "Déploiement terminé!");
                                    y += 1;
                                    self.ui.set_color(Color::Fg);
                                    self.ui.draw_text(box_x + 2, y, "Pour démarrer RMDB:");
                                    y += 1;
                                    self.ui.draw_text(box_x + 4, y, "lxc-attach -n rmdb -- rc-service rmdbd start");
                                    y += 1;
                                }
                                Err(e) => {
                                    self.ui.set_color(Color::Error);
                                    self.ui.draw_text(box_x + 2, y, &format!("Erreur lors de l'installation de RMDB: {}", e));
                                    y += 1;
                                    
                                    // Afficher le chemin du log pour le débogage
                                    if let Some(ref logger) = lxc_deploy.logger {
                                        self.ui.set_color(Color::Warning);
                                        self.ui.draw_text(box_x + 2, y, &format!("Consultez les logs dans: {}", logger.log_path().display()));
                                        y += 1;
                                    }
                                    y += 1;
                                }
                            }
                        }
                        Err(e) => {
                            self.ui.set_color(Color::Error);
                            self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
                            y += 2;
                        }
                    }
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "Erreur lors de la création du container.");
                    y += 1;
                    
                    // Afficher les détails de l'erreur
                    let (_, _, _, box_h) = self.ui.get_box_dimensions();
                    let error_text = format!("{}{}", output.stderr, output.stdout);
                    
                    // Détecter les erreurs spécifiques à RHEL
                    let is_rhel_permission_error = error_text.contains("No uid mapping") || 
                                                   error_text.contains("You must either run as root") ||
                                                   error_text.contains("Error chowning");
                    let is_config_error = error_text.contains("Failed to open file") && 
                                         error_text.contains("default.conf");
                    
                    if is_rhel_permission_error {
                        self.ui.set_color(Color::Warning);
                        self.ui.draw_text(box_x + 2, y, "Erreur de permissions détectée (RHEL/CentOS).");
                        y += 1;
                        self.ui.set_color(Color::Info);
                        self.ui.draw_text(box_x + 2, y, "Sur RHEL, LXC nécessite souvent:");
                        y += 1;
                        self.ui.set_color(Color::Fg);
                        self.ui.draw_text(box_x + 4, y, "1. Exécution en tant que root, OU");
                        y += 1;
                        self.ui.draw_text(box_x + 4, y, "2. Configuration de mappings UID/GID");
                        y += 2;
                        self.ui.set_color(Color::Warning);
                        self.ui.draw_text(box_x + 2, y, "Solution recommandée: Exécutez le TUI avec sudo");
                        y += 2;
                    } else if is_config_error {
                        self.ui.set_color(Color::Warning);
                        self.ui.draw_text(box_x + 2, y, "Configuration LXC manquante.");
                        y += 1;
                        self.ui.set_color(Color::Info);
                        self.ui.draw_text(box_x + 2, y, "La configuration a été créée automatiquement.");
                        y += 1;
                        self.ui.draw_text(box_x + 2, y, "Réessayez la création du container.");
                        y += 2;
                    } else {
                        // Afficher les erreurs standard
                        if !output.stderr.is_empty() {
                            let error_lines: Vec<&str> = output.stderr.lines().collect();
                            for (i, line) in error_lines.iter().take(5).enumerate() {
                                if (y + i as u16) < (box_y + box_h - 5) {
                                    self.ui.set_color(Color::Error);
                                    self.ui.draw_text(box_x + 2, y + i as u16, line);
                                }
                            }
                            y += error_lines.len().min(5) as u16;
                        }
                        
                        if !output.stdout.is_empty() {
                            let output_lines: Vec<&str> = output.stdout.lines().collect();
                            for (i, line) in output_lines.iter().take(3).enumerate() {
                                if (y + i as u16) < (box_y + box_h - 5) {
                                    self.ui.set_color(Color::Warning);
                                    self.ui.draw_text(box_x + 2, y + i as u16, line);
                                }
                            }
                            y += output_lines.len().min(3) as u16;
                        }
                        
                        y += 1;
                        self.ui.set_color(Color::Info);
                        self.ui.draw_text(box_x + 2, y, "Vérifiez que les templates LXC sont installés:");
                        y += 1;
                        self.ui.set_color(Color::Fg);
                        let packages = vec!["lxc-templates"];
                        let install_hint = format!("  sudo {}", self.distribution.install_command(&packages.iter().map(|s| *s).collect::<Vec<_>>()));
                        self.ui.draw_text(box_x + 2, y, &install_hint);
                        y += 1;
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
                y += 2;
                
                // Si c'est un outil manquant, proposer l'installation
                if let ExecError::MissingTool(_) = e {
                    self.ui.set_color(Color::Info);
                    self.ui.draw_text(box_x + 2, y, "Voulez-vous installer les templates LXC ?");
                    y += 2;
                    
                    let install_templates = self.ask_yes_no(
                        "Installation des Templates LXC",
                        "Les templates LXC ne sont pas installés. Voulez-vous les installer automatiquement ?"
                    );
                    
                    if install_templates {
                        if self.install_lxc_templates() {
                            // Réessayer la création
                            self.ui.clear_screen();
                            self.ui.draw_header("Déploiement Container LXC Alpine");
                            y = box_y + 5;
                            self.ui.set_color(Color::Info);
                            self.ui.draw_text(box_x + 2, y, "Nouvelle tentative de création du container...");
                            y += 1;
                            
                            match lxc_deploy.create_container(&self.executor) {
                                Ok(output) => {
                                    if output.exit_code == Some(0) {
                                        self.ui.set_color(Color::Success);
                                        self.ui.draw_text(box_x + 2, y, "Container créé avec succès!");
                                        y += 2;
                                    } else {
                                        self.ui.set_color(Color::Error);
                                        self.ui.draw_text(box_x + 2, y, "Échec de la création après installation des templates.");
                                        y += 1;
                                    }
                                }
                                Err(e2) => {
                                    self.ui.set_color(Color::Error);
                                    self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e2));
                                    y += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 1, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn show_deployment_status(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Statut des Containers LXC");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        // Vérifier LXC
        if !lxc_deploy.check_lxc_installed() {
            self.ui.set_color(Color::Warning);
            self.ui.draw_text(box_x + 2, y, "LXC n'est pas installé.");
            self.ui.set_color(Color::Reset);
            
            // Proposer l'installation
            let install_lxc = self.ask_yes_no(
                "Installation de LXC",
                "LXC n'est pas installé. Voulez-vous l'installer automatiquement ?"
            );
            
            if install_lxc {
                if !self.install_lxc() {
                    return; // Échec de l'installation
                }
                // Vérifier à nouveau après installation
                if !lxc_deploy.check_lxc_installed() {
                    self.show_error_message("Erreur", "LXC n'a pas pu être installé ou détecté.");
                    return;
                }
                // Continuer avec l'affichage du statut
                self.ui.clear_screen();
                self.ui.draw_header("Statut des Containers LXC");
                y = box_y + 5;
            } else {
                // L'utilisateur a refusé l'installation
                return;
            }
        }

        // Lister les containers
        let cmd = "lxc-ls -f 2>/dev/null || echo 'Aucun container trouvé'";
        let output = self.executor.run_shell(cmd, false);
        let content = output.map(|o| o.stdout).unwrap_or_else(|_| "Erreur".to_string());

        for line in content.lines().take(20) {
            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 2, y, line);
            y += 1;
        }

        // Vérifier le statut du container rmdb
        if lxc_deploy.check_container_exists_with_executor(&self.executor) {
            y += 1;
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y, "Container 'rmdb':");
            y += 1;
            
            match lxc_deploy.get_container_status(&self.executor) {
                Ok(status) => {
                    self.ui.set_color(if status == "RUNNING" { Color::Success } else { Color::Warning });
                    self.ui.draw_text(box_x + 4, y, &format!("État: {}", status));
                }
                Err(_) => {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 4, y, "État: inconnu");
                }
            }
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    // ========== Gestion Container LXC ==========

    fn lxc_start_container(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Démarrage Container LXC");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Démarrage du container 'rmdb'...");
        y += 1;

        match lxc_deploy.start_container(&self.executor) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "Container démarré avec succès!");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "Échec du démarrage du container.");
                    if !output.stderr.is_empty() {
                        self.ui.draw_text(box_x + 2, y + 1, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
            }
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn lxc_stop_container(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Arrêt Container LXC");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Arrêt du container 'rmdb'...");
        y += 1;

        match lxc_deploy.stop_container(&self.executor) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "Container arrêté avec succès!");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "Échec de l'arrêt du container.");
                    if !output.stderr.is_empty() {
                        self.ui.draw_text(box_x + 2, y + 1, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
            }
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn lxc_restart_container(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Redémarrage Container LXC");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Redémarrage du container 'rmdb'...");
        y += 1;

        // Arrêter
        match lxc_deploy.stop_container(&self.executor) {
            Ok(_) => {
                std::thread::sleep(std::time::Duration::from_secs(2));
                // Démarrer
                match lxc_deploy.start_container(&self.executor) {
                    Ok(output) => {
                        if output.exit_code == Some(0) {
                            self.ui.set_color(Color::Success);
                            self.ui.draw_text(box_x + 2, y, "Container redémarré avec succès!");
                        } else {
                            self.ui.set_color(Color::Error);
                            self.ui.draw_text(box_x + 2, y, "Échec du redémarrage du container.");
                        }
                    }
                    Err(e) => {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, y, &format!("Erreur au démarrage: {}", e));
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("Erreur à l'arrêt: {}", e));
            }
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn lxc_show_logs(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Logs Container LXC");
        let (box_x, box_y, _, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        let cmd = format!("lxc-info -n rmdb -S 2>/dev/null || echo 'Container non démarré'");
        let output = self.executor.run_shell(&cmd, false);
        let status = output.map(|o| o.stdout.trim().to_string()).unwrap_or_else(|_| "Inconnu".to_string());

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, &format!("Statut: {}", status));
        y += 2;

        // Afficher les dernières lignes des logs système
        let logs_cmd = format!("journalctl -u lxc@rmdb.service -n 50 --no-pager 2>/dev/null || dmesg | grep -i lxc | tail -20 || echo 'Logs non disponibles'");
        let logs_output = self.executor.run_shell(&logs_cmd, false);
        let logs = logs_output.map(|o| o.stdout).unwrap_or_else(|_| "Erreur".to_string());

        self.ui.set_color(Color::Fg);
        for line in logs.lines().take((box_h - y - 5) as usize) {
            if y >= box_h - 5 {
                break;
            }
            self.ui.draw_text(box_x + 2, y, line);
            y += 1;
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, box_h - 2, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn lxc_access_shell(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Accès Shell Container");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Pour accéder au shell du container, utilisez:");
        y += 2;
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "  lxc-attach -n rmdb");
        y += 2;
        self.ui.set_color(Color::Warning);
        self.ui.draw_text(box_x + 2, y, "Ou depuis le TUI, exécutez cette commande:");
        y += 1;
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "  lxc-attach -n rmdb -- sh");
        y += 3;
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Note: L'accès shell interactif nécessite de quitter le TUI.");
        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn lxc_show_stats(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Statistiques Container LXC");
        let (box_x, box_y, _, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        // Informations du container
        let info_cmd = format!("lxc-info -n rmdb 2>/dev/null || echo 'Container non démarré'");
        let info_output = self.executor.run_shell(&info_cmd, false);
        let info = info_output.map(|o| o.stdout).unwrap_or_else(|_| "Erreur".to_string());

        self.ui.set_color(Color::Fg);
        for line in info.lines().take((box_h - y - 5) as usize) {
            if y >= box_h - 5 {
                break;
            }
            self.ui.draw_text(box_x + 2, y, line);
            y += 1;
        }

        y += 1;

        // Statistiques CPU/Mémoire depuis le container
        let stats_cmd = format!("lxc-attach -n rmdb -- sh -c 'top -bn1 | head -5' 2>/dev/null || echo 'Statistiques non disponibles'");
        let stats_output = self.executor.run_shell(&stats_cmd, false);
        let stats = stats_output.map(|o| o.stdout).unwrap_or_else(|_| "Erreur".to_string());

        self.ui.set_color(Color::Warning);
        self.ui.draw_text(box_x + 2, y, "Statistiques du container:");
        y += 1;
        self.ui.set_color(Color::Fg);
        for line in stats.lines().take((box_h - y - 5) as usize) {
            if y >= box_h - 5 {
                break;
            }
            self.ui.draw_text(box_x + 2, y, line);
            y += 1;
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, box_h - 2, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn lxc_show_config(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Configuration Container LXC");
        let (box_x, box_y, _, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        let config_path = "/var/lib/lxc/rmdb/config";
        let config_cmd = format!("cat {} 2>/dev/null || echo 'Configuration non trouvée'", config_path);
        let config_output = self.executor.run_shell(&config_cmd, false);
        let config = config_output.map(|o| o.stdout).unwrap_or_else(|_| "Erreur".to_string());

        self.ui.set_color(Color::Fg);
        for line in config.lines().take((box_h - y - 5) as usize) {
            if y >= box_h - 5 {
                break;
            }
            self.ui.draw_text(box_x + 2, y, line);
            y += 1;
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, box_h - 2, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn lxc_rmdb_start(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Démarrage RMDB dans Container");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Démarrage de RMDB dans le container...");
        y += 1;

        let cmd = "lxc-attach -n rmdb -- rc-service rmdbd start 2>&1 || lxc-attach -n rmdb -- /usr/local/bin/rmdbd -config /etc/rmdbd/config.json &";
        let output = self.executor.run_shell(cmd, true);

        match output {
            Ok(o) => {
                if o.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "RMDB démarré avec succès!");
                } else {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 2, y, "RMDB peut être déjà démarré ou erreur au démarrage.");
                    if !o.stderr.is_empty() {
                        self.ui.draw_text(box_x + 2, y + 1, &o.stderr.lines().next().unwrap_or(""));
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
            }
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn lxc_rmdb_stop(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Arrêt RMDB dans Container");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Arrêt de RMDB dans le container...");
        y += 1;

        let cmd = "lxc-attach -n rmdb -- rc-service rmdbd stop 2>&1 || lxc-attach -n rmdb -- pkill rmdbd";
        let output = self.executor.run_shell(cmd, true);

        match output {
            Ok(o) => {
                if o.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "RMDB arrêté avec succès!");
                } else {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 2, y, "RMDB peut être déjà arrêté.");
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
            }
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn lxc_rmdb_restart(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Redémarrage RMDB dans Container");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Redémarrage de RMDB dans le container...");
        y += 1;

        let cmd = "lxc-attach -n rmdb -- rc-service rmdbd restart 2>&1 || (lxc-attach -n rmdb -- pkill rmdbd && sleep 1 && lxc-attach -n rmdb -- /usr/local/bin/rmdbd -config /etc/rmdbd/config.json &)";
        let output = self.executor.run_shell(cmd, true);

        match output {
            Ok(o) => {
                if o.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "RMDB redémarré avec succès!");
                } else {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 2, y, "Redémarrage effectué (peut avoir échoué).");
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
            }
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn lxc_rmdb_logs(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Logs RMDB dans Container");
        let (box_x, box_y, _, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        let cmd = "lxc-attach -n rmdb -- tail -50 /var/log/rmdbd.log 2>/dev/null || lxc-attach -n rmdb -- journalctl -u rmdbd -n 50 --no-pager 2>/dev/null || echo 'Logs non disponibles'";
        let output = self.executor.run_shell(cmd, false);
        let logs = output.map(|o| o.stdout).unwrap_or_else(|_| "Erreur".to_string());

        self.ui.set_color(Color::Fg);
        for line in logs.lines().take((box_h - y - 5) as usize) {
            if y >= box_h - 5 {
                break;
            }
            self.ui.draw_text(box_x + 2, y, line);
            y += 1;
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, box_h - 2, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn lxc_destroy_container(&mut self) {
        let confirm = self.ask_yes_no(
            "Suppression Container",
            "Êtes-vous sûr de vouloir supprimer le container 'rmdb' ? Cette action est irréversible."
        );

        if !confirm {
            return;
        }

        self.ui.clear_screen();
        self.ui.draw_header("Suppression Container LXC");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let lxc_deploy = LXCDeployment::new("rmdb".to_string(), "3.20".to_string());

        if !lxc_deploy.check_container_exists() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "Le container 'rmdb' n'existe pas.");
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        self.ui.set_color(Color::Warning);
        self.ui.draw_text(box_x + 2, y, "Arrêt du container...");
        y += 1;
        let _ = lxc_deploy.stop_container(&self.executor);

        self.ui.draw_text(box_x + 2, y, "Suppression du container...");
        y += 1;

        match lxc_deploy.destroy_container(&self.executor) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "Container supprimé avec succès!");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "Échec de la suppression du container.");
                    if !output.stderr.is_empty() {
                        self.ui.draw_text(box_x + 2, y + 1, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
            }
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    // Fonctions de gestion générale des containers LXC

    fn show_containers_list(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Liste des Containers LXC");
        let (box_x, box_y, _box_w, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Chargement de la liste des containers...");
        self.ui.set_color(Color::Reset);
        io::stdout().flush().unwrap();

        match LXCDeployment::list_all_containers(&self.executor) {
            Ok(containers) => {
                if containers.is_empty() {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 2, y, "Aucun container LXC trouvé.");
                    y += 2;
                } else {
                    self.ui.clear_line(y);
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, &format!("{} container(s) trouvé(s):", containers.len()));
                    y += 2;

                    // En-tête
                    self.ui.set_color(Color::Info);
                    let header = format!("{:<20} {:<15}", "Nom", "Statut");
                    self.ui.draw_text(box_x + 2, y, &header);
                    y += 1;
                    self.ui.draw_text(box_x + 2, y, &"-".repeat(35));
                    y += 1;

                    // Liste des containers
                    for container in containers {
                        self.ui.set_color(Color::Fg);
                        let status_color = match container.status.as_str() {
                            "RUNNING" => Color::Success,
                            "STOPPED" => Color::Warning,
                            "FROZEN" => Color::Info,
                            _ => Color::Fg,
                        };
                        self.ui.set_color(status_color);
                        let line = format!("{:<20} {:<15}", container.name, container.status);
                        if y < box_y + box_h - 5 {
                            self.ui.draw_text(box_x + 2, y, &line);
                            y += 1;
                        }
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("Erreur lors de la récupération de la liste: {}", e));
                y += 2;
            }
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn select_container(&mut self, title: &str) -> Option<String> {
        self.ui.clear_screen();
        self.ui.draw_header(title);
        let (box_x, box_y, _box_w, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Chargement de la liste des containers...");
        self.ui.set_color(Color::Reset);
        io::stdout().flush().unwrap();

        let containers = match LXCDeployment::list_all_containers(&self.executor) {
            Ok(c) => c,
            Err(e) => {
                self.ui.clear_line(y);
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
                y += 2;
                self.ui.set_color(Color::Reset);
                self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                let _ = self.input_reader.read_key();
                return None;
            }
        };

        if containers.is_empty() {
            self.ui.clear_line(y);
            self.ui.set_color(Color::Warning);
            self.ui.draw_text(box_x + 2, y, "Aucun container LXC trouvé.");
            y += 2;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return None;
        }

        // Afficher la liste avec sélection
        let mut selected = 0;
        loop {
            self.ui.clear_screen();
            self.ui.draw_header(title);
            y = box_y + 5;

            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y, "Sélectionnez un container:");
            y += 2;

            for (i, container) in containers.iter().enumerate() {
                let selected_char = if i == selected { "▶" } else { " " };
                let status_color = match container.status.as_str() {
                    "RUNNING" => Color::Success,
                    "STOPPED" => Color::Warning,
                    "FROZEN" => Color::Info,
                    _ => Color::Fg,
                };
                self.ui.set_color(if i == selected { Color::Selection } else { Color::Fg });
                self.ui.draw_text(box_x + 2, y, &format!("{} {}", selected_char, container.name));
                self.ui.set_color(status_color);
                self.ui.draw_text(box_x + 25, y, &format!("({})", container.status));
                y += 1;
            }

            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, box_h - 2, "Flèches: Sélectionner | Entrée: Valider | Q: Annuler");

            match self.input_reader.read_key() {
                Ok(Key::Up) => {
                    if selected > 0 {
                        selected -= 1;
                    } else {
                        selected = containers.len() - 1;
                    }
                }
                Ok(Key::Down) => {
                    if selected < containers.len() - 1 {
                        selected += 1;
                    } else {
                        selected = 0;
                    }
                }
                Ok(Key::Enter) => {
                    return Some(containers[selected].name.clone());
                }
                Ok(Key::Quit) => {
                    return None;
                }
                _ => {}
            }
        }
    }

    fn containers_start(&mut self) {
        if let Some(container_name) = self.select_container("Démarrer Container") {
            self.ui.clear_screen();
            self.ui.draw_header("Démarrer Container LXC");
            let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
            let mut y = box_y + 5;

            // Vérifier d'abord si le container a une configuration valide
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y, &format!("Vérification de la configuration du container '{}'...", container_name));
            y += 1;
            io::stdout().flush().unwrap();

            let config_check = format!("sudo -n test -f /var/lib/lxc/{}/config && echo 'found' || echo 'not found'", container_name);
            let has_config = self.executor.run_shell(&config_check, true)
                .map(|o| o.stdout.contains("found"))
                .unwrap_or(false);

            if !has_config {
                self.ui.clear_line(y - 1);
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y - 1, "✗ Configuration du container introuvable!");
                y += 1;
                self.ui.set_color(Color::Warning);
                self.ui.draw_text(box_x + 2, y, "Le container semble être corrompu ou incomplet.");
                y += 1;
                self.ui.set_color(Color::Fg);
                self.ui.draw_text(box_x + 2, y, "Solutions possibles:");
                y += 1;
                self.ui.draw_text(box_x + 4, y, "1. Réinstaller le container (menu: Réinstaller)");
                y += 1;
                self.ui.draw_text(box_x + 4, y, "2. Supprimer et recréer le container");
                y += 2;
                self.ui.set_color(Color::Reset);
                self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                let _ = self.input_reader.read_key();
                return;
            }

            self.ui.clear_line(y - 1);
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y - 1, &format!("Démarrage du container '{}'...", container_name));
            y += 2;

            match LXCDeployment::start_container_by_name(&self.executor, &container_name) {
                Ok(output) => {
                    if output.exit_code == Some(0) {
                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 2, y, "Container démarré avec succès!");
                        
                        // Vérifier si RMDB est installé dans le container
                        y += 2;
                        self.ui.set_color(Color::Info);
                        self.ui.draw_text(box_x + 2, y, "Vérification de l'installation de RMDB...");
                        y += 1;
                        io::stdout().flush().unwrap();
                        
                        let rmdb_check = format!("lxc-attach -n {} -- test -f /usr/local/bin/rmdbd && echo 'installed' || echo 'not installed'", container_name);
                        let rmdb_installed = self.executor.run_shell(&rmdb_check, true)
                            .map(|o| o.stdout.contains("installed"))
                            .unwrap_or(false);
                        
                        if !rmdb_installed && container_name == "rmdb" {
                            self.ui.clear_line(y - 1);
                            self.ui.set_color(Color::Warning);
                            self.ui.draw_text(box_x + 2, y - 1, "⚠ RMDB n'est pas installé dans le container.");
                            y += 1;
                            self.ui.set_color(Color::Fg);
                            self.ui.draw_text(box_x + 2, y, "Pour installer RMDB:");
                            y += 1;
                            self.ui.draw_text(box_x + 4, y, "1. Utilisez le menu 'Ajouter' pour créer un nouveau container avec RMDB");
                            y += 1;
                            self.ui.draw_text(box_x + 4, y, "2. Ou réinstallez le container 'rmdb' (menu: Réinstaller)");
                            y += 1;
                            self.ui.set_color(Color::Info);
                            self.ui.draw_text(box_x + 2, y, "Note: L'installation de RMDB nécessite rmdb_source.");
                            y += 1;
                        } else if rmdb_installed {
                            self.ui.clear_line(y - 1);
                            self.ui.set_color(Color::Success);
                            self.ui.draw_text(box_x + 2, y - 1, "✓ RMDB est installé dans le container.");
                        }
                    } else {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, y, "Échec du démarrage du container.");
                        y += 1;
                        
                        // Analyser l'erreur pour donner des conseils
                        let stderr = output.stderr.to_lowercase();
                        if stderr.contains("no container config") || stderr.contains("config specified") {
                            self.ui.set_color(Color::Warning);
                            self.ui.draw_text(box_x + 2, y, "Le container n'a pas de configuration valide.");
                            y += 1;
                            self.ui.set_color(Color::Fg);
                            self.ui.draw_text(box_x + 2, y, "Solutions:");
                            y += 1;
                            self.ui.draw_text(box_x + 4, y, "1. Réinstaller le container (menu: Réinstaller)");
                            y += 1;
                            self.ui.draw_text(box_x + 4, y, "2. Vérifier que le container a été créé correctement");
                            y += 1;
                        } else {
                            if !output.stderr.is_empty() {
                                self.ui.set_color(Color::Fg);
                                let error_line = output.stderr.lines().next().unwrap_or("Erreur inconnue");
                                self.ui.draw_text(box_x + 2, y, error_line);
                                y += 1;
                            }
                        }
                    }
                }
                Err(e) => {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
                }
            }

            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
        }
    }

    fn containers_stop(&mut self) {
        if let Some(container_name) = self.select_container("Stopper Container") {
            self.ui.clear_screen();
            self.ui.draw_header("Arrêter Container LXC");
            let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
            let mut y = box_y + 5;

            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y, &format!("Arrêt du container '{}'...", container_name));
            y += 2;

            match LXCDeployment::stop_container_by_name(&self.executor, &container_name) {
                Ok(output) => {
                    if output.exit_code == Some(0) {
                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 2, y, "Container arrêté avec succès!");
                    } else {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, y, "Échec de l'arrêt du container.");
                        if !output.stderr.is_empty() {
                            self.ui.draw_text(box_x + 2, y + 1, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                        }
                    }
                }
                Err(e) => {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
                }
            }

            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
        }
    }

    fn containers_destroy(&mut self) {
        if let Some(container_name) = self.select_container("Supprimer Container") {
            let confirm = self.ask_yes_no(
                "Suppression Container",
                &format!("Êtes-vous sûr de vouloir supprimer le container '{}' ? Cette action est irréversible.", container_name)
            );

            if !confirm {
                return;
            }

            self.ui.clear_screen();
            self.ui.draw_header("Supprimer Container LXC");
            let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
            let mut y = box_y + 5;

            self.ui.set_color(Color::Warning);
            self.ui.draw_text(box_x + 2, y, &format!("Suppression du container '{}'...", container_name));
            y += 2;

            match LXCDeployment::destroy_container_by_name(&self.executor, &container_name) {
                Ok(output) => {
                    if output.exit_code == Some(0) {
                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 2, y, "Container supprimé avec succès!");
                    } else {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, y, "Échec de la suppression du container.");
                        if !output.stderr.is_empty() {
                            self.ui.draw_text(box_x + 2, y + 1, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                        }
                    }
                }
                Err(e) => {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
                }
            }

            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
        }
    }

    fn containers_restart(&mut self) {
        if let Some(container_name) = self.select_container("Redémarrer Container") {
            self.ui.clear_screen();
            self.ui.draw_header("Redémarrer Container");
            let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
            let mut y = box_y + 5;

            // Arrêter le container
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y, &format!("Arrêt du container '{}'...", container_name));
            y += 1;
            io::stdout().flush().unwrap();

            let _ = LXCDeployment::stop_container_by_name(&self.executor, &container_name);
            std::thread::sleep(std::time::Duration::from_secs(2));

            // Démarrer le container
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y, &format!("Démarrage du container '{}'...", container_name));
            y += 2;
            io::stdout().flush().unwrap();

            match LXCDeployment::start_container_by_name(&self.executor, &container_name) {
                Ok(output) => {
                    if output.exit_code == Some(0) {
                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 2, y, "Container redémarré avec succès!");
                    } else {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, y, "Échec du redémarrage du container.");
                        if !output.stderr.is_empty() {
                            self.ui.draw_text(box_x + 2, y + 1, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                        }
                    }
                }
                Err(e) => {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
                }
            }

            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
        }
    }

    fn containers_add(&mut self) {
        // Utiliser la fonction de déploiement existante mais avec un nom personnalisable
        self.ui.clear_screen();
        self.ui.draw_header("Ajouter Container Alpine Linux");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Création d'un nouveau container Alpine Linux...");
        y += 2;

        // Demander le nom du container
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "Nom du container (ou Entrée pour 'rmdb'):");
        y += 1;
        self.ui.set_color(Color::Reset);
        io::stdout().flush().unwrap();

        let mut container_name = String::new();
        loop {
            match self.input_reader.read_key() {
                Ok(Key::Enter) => {
                    if container_name.trim().is_empty() {
                        container_name = "rmdb".to_string();
                    }
                    break;
                }
                Ok(Key::Backspace) => {
                    if !container_name.is_empty() {
                        container_name.pop();
                        self.ui.set_cursor(box_x + 2 + container_name.len() as u16, y);
                        print!(" ");
                        self.ui.set_cursor(box_x + 2 + container_name.len() as u16, y);
                        io::stdout().flush().unwrap();
                    }
                }
                Ok(Key::Char(c)) => {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        container_name.push(c);
                        print!("{}", c);
                        io::stdout().flush().unwrap();
                    }
                }
                Ok(Key::Quit) => {
                    return;
                }
                _ => {}
            }
        }

        // Vérifier si le container existe déjà en utilisant check_container_exists_with_executor
        // Cette méthode vérifie réellement via LXC, pas seulement via list_all_containers qui peut avoir des placeholders
        let lxc_deploy = LXCDeployment::new(container_name.clone(), "3.20".to_string());
        if lxc_deploy.check_container_exists_with_executor(&self.executor) {
            // Vérifier aussi via lxc-ls pour confirmer
            let cmd_verify = format!("sudo -n lxc-ls -1 2>/dev/null | grep -q '^{}$' && echo 'found' || echo 'not found'", container_name);
            let is_really_managed = self.executor.run_shell(&cmd_verify, true)
                .map(|o| o.stdout.contains("found"))
                .unwrap_or(false);
            
            if is_really_managed {
                self.ui.set_color(Color::Warning);
                self.ui.draw_text(box_x + 2, y + 2, &format!("Le container '{}' existe déjà et est géré par LXC.", container_name));
                self.ui.set_color(Color::Reset);
                self.ui.draw_text(box_x + 2, y + 4, "Appuyez sur une touche pour continuer...");
                let _ = self.input_reader.read_key();
                return;
            } else {
                // Le répertoire existe mais n'est pas géré par LXC - c'est un container fantôme
                self.ui.set_color(Color::Warning);
                self.ui.draw_text(box_x + 2, y + 2, &format!("⚠ Un répertoire existe pour '{}' mais le container n'est pas géré par LXC.", container_name));
                y += 1;
                self.ui.set_color(Color::Info);
                self.ui.draw_text(box_x + 2, y + 2, "Voulez-vous supprimer ce répertoire et créer un nouveau container ?");
                y += 1;
                self.ui.set_color(Color::Reset);
                
                let should_clean = self.ask_yes_no("Nettoyage", "Supprimer le répertoire existant ?");
                if should_clean {
                    let cmd_clean = format!("sudo -n rm -rf /var/lib/lxc/{} 2>&1", container_name);
                    if let Ok(output) = self.executor.run_shell(&cmd_clean, true) {
                        if output.exit_code == Some(0) || output.stdout.is_empty() {
                            self.ui.set_color(Color::Success);
                            self.ui.draw_text(box_x + 2, y + 2, "✓ Répertoire supprimé. Vous pouvez maintenant créer le container.");
                            y += 2;
                            self.ui.set_color(Color::Reset);
                            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
                            let _ = self.input_reader.read_key();
                            // Continuer avec la création
                        } else {
                            self.ui.set_color(Color::Error);
                            self.ui.draw_text(box_x + 2, y + 2, &format!("✗ Erreur lors de la suppression: {}", output.stdout));
                            y += 2;
                            self.ui.set_color(Color::Reset);
                            self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
                            let _ = self.input_reader.read_key();
                            return;
                        }
                    } else {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, y + 2, "✗ Impossible de supprimer le répertoire.");
                        y += 2;
                        self.ui.set_color(Color::Reset);
                        self.ui.draw_text(box_x + 2, y + 2, "Appuyez sur une touche pour continuer...");
                        let _ = self.input_reader.read_key();
                        return;
                    }
                } else {
                    // L'utilisateur a refusé le nettoyage
                    return;
                }
            }
        }

        // Créer le container
        y += 2;
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, &format!("Création du container '{}'...", container_name));
        y += 1;
        io::stdout().flush().unwrap();

        // Utiliser la logique de création existante mais avec le nom personnalisé
        let lxc_deploy = LXCDeployment::new(container_name.clone(), "3.20".to_string());
        match lxc_deploy.create_container(&self.executor) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "Container créé avec succès!");
                    y += 2;

                    // Démarrer le container
                    self.ui.set_color(Color::Info);
                    self.ui.draw_text(box_x + 2, y, "Démarrage du container...");
                    y += 1;
                    let _ = lxc_deploy.start_container(&self.executor);
                    std::thread::sleep(std::time::Duration::from_secs(2));

                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "Container démarré!");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "Erreur lors de la création du container.");
                    if !output.stderr.is_empty() {
                        self.ui.draw_text(box_x + 2, y + 1, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
            }
        }

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn containers_reinstall(&mut self) {
        if let Some(container_name) = self.select_container("Réinstaller Container") {
            let confirm = self.ask_yes_no(
                "Réinstallation Container",
                &format!("Êtes-vous sûr de vouloir réinstaller le container '{}' ?\nCette action va supprimer complètement le container et le recréer.", container_name)
            );

            if !confirm {
                return;
            }

            self.ui.clear_screen();
            self.ui.draw_header("Réinstaller Container");
            let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
            let mut y = box_y + 5;

            // Étape 1: Arrêter le container
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y, &format!("Arrêt du container '{}'...", container_name));
            y += 1;
            io::stdout().flush().unwrap();
            let _ = LXCDeployment::stop_container_by_name(&self.executor, &container_name);
            std::thread::sleep(std::time::Duration::from_secs(2));

            // Étape 2: Supprimer le container
            self.ui.set_color(Color::Warning);
            self.ui.draw_text(box_x + 2, y, &format!("Suppression du container '{}'...", container_name));
            y += 1;
            io::stdout().flush().unwrap();

            match LXCDeployment::destroy_container_by_name(&self.executor, &container_name) {
                Ok(output) => {
                    if output.exit_code == Some(0) || output.stderr.contains("does not exist") || output.stderr.contains("not found") {
                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 2, y, "Container supprimé.");
                    } else {
                        self.ui.set_color(Color::Warning);
                        self.ui.draw_text(box_x + 2, y, "Avertissement: Erreur lors de la suppression.");
                        y += 1;
                        if !output.stderr.is_empty() {
                            self.ui.set_color(Color::Fg);
                            let error_preview = output.stderr.lines().next().unwrap_or("Erreur inconnue");
                            self.ui.draw_text(box_x + 4, y, error_preview);
                            y += 1;
                        }
                    }
                }
                Err(e) => {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 2, y, &format!("Erreur lors de la suppression: {}", e));
                    y += 1;
                }
            }
            
            // Attendre un peu et vérifier que le container a bien été supprimé
            std::thread::sleep(std::time::Duration::from_secs(2));
            
            // Vérification stricte : utiliser check_container_fully_removed qui vérifie uniquement le système de fichiers
            let mut fully_removed = LXCDeployment::check_container_fully_removed(&self.executor, &container_name);
            
            if !fully_removed {
                self.ui.set_color(Color::Warning);
                self.ui.draw_text(box_x + 2, y, "Le répertoire du container existe encore. Suppression forcée...");
                y += 1;
                io::stdout().flush().unwrap();
                
                // Essayer de supprimer directement tous les répertoires possibles
                let paths_to_remove = vec![
                    format!("/var/lib/lxc/{}", container_name),
                    format!("/var/lib/lxd/containers/{}", container_name),
                ];
                
                for path in &paths_to_remove {
                    let force_remove_cmd = format!("sudo -n rm -rf {} 2>&1", path);
                    let _ = self.executor.run_shell(&force_remove_cmd, true);
                }
                
                // Attendre un peu après suppression
                std::thread::sleep(std::time::Duration::from_secs(1));
                
                // Vérifier à nouveau
                fully_removed = LXCDeployment::check_container_fully_removed(&self.executor, &container_name);
                
                if fully_removed {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "Répertoires supprimés avec succès.");
                    y += 1;
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "Impossible de supprimer les répertoires.");
                    y += 1;
                    self.ui.set_color(Color::Fg);
                    self.ui.draw_text(box_x + 2, y, "Veuillez supprimer manuellement:");
                    y += 1;
                    for path in &paths_to_remove {
                        self.ui.draw_text(box_x + 4, y, &format!("sudo rm -rf {}", path));
                        y += 1;
                    }
                    y += 1;
                    self.ui.set_color(Color::Reset);
                    self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                    let _ = self.input_reader.read_key();
                    return;
                }
            }
            
            // Nettoyer les entrées fantômes (si le container est détecté par lxc-ls mais n'existe pas dans le système de fichiers)
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y, "Nettoyage des entrées fantômes...");
            y += 1;
            io::stdout().flush().unwrap();
            let _ = LXCDeployment::cleanup_ghost_container(&self.executor, &container_name);
            std::thread::sleep(std::time::Duration::from_secs(1));
            
            // Vérification finale : s'assurer que le container n'est plus détecté nulle part
            let lxc_deploy_check = LXCDeployment::new(container_name.clone(), "3.20".to_string());
            let still_detected = lxc_deploy_check.check_container_exists_with_executor(&self.executor);
            
            if still_detected {
                self.ui.set_color(Color::Warning);
                self.ui.draw_text(box_x + 2, y, "⚠ Le container est encore détecté par certaines commandes LXC.");
                y += 1;
                self.ui.set_color(Color::Fg);
                self.ui.draw_text(box_x + 2, y, "Tentative de nettoyage supplémentaire...");
                y += 1;
                io::stdout().flush().unwrap();
                
                // Attendre un peu plus et réessayer
                std::thread::sleep(std::time::Duration::from_secs(2));
                
                // Vérifier à nouveau
                let still_detected_after = lxc_deploy_check.check_container_exists_with_executor(&self.executor);
                if still_detected_after {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "✗ Le container est toujours détecté. Réinstallation impossible.");
                    y += 1;
                    self.ui.set_color(Color::Fg);
                    self.ui.draw_text(box_x + 2, y, "Veuillez redémarrer le système ou nettoyer manuellement les caches LXC.");
                    y += 2;
                    self.ui.set_color(Color::Reset);
                    self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                    let _ = self.input_reader.read_key();
                    return;
                }
            }
            
            self.ui.clear_line(y - 1);
            self.ui.set_color(Color::Success);
            self.ui.draw_text(box_x + 2, y - 1, "✓ Container complètement supprimé et nettoyé.");
            y += 1;

            // Étape 3: Vérification finale avant création
            y += 1;
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y, "Vérification finale avant création...");
            y += 1;
            io::stdout().flush().unwrap();
            
            // Vérifier une dernière fois que le container n'existe vraiment plus (système de fichiers uniquement)
            let final_check = LXCDeployment::check_container_fully_removed(&self.executor, &container_name);
            if !final_check {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, "✗ ERREUR: Le container existe encore dans le système de fichiers.");
                y += 1;
                self.ui.set_color(Color::Fg);
                self.ui.draw_text(box_x + 2, y, "Veuillez supprimer manuellement le répertoire:");
                y += 1;
                self.ui.draw_text(box_x + 4, y, &format!("sudo rm -rf /var/lib/lxc/{}", container_name));
                y += 2;
                self.ui.set_color(Color::Reset);
                self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                let _ = self.input_reader.read_key();
                return;
            }
            
            self.ui.clear_line(y - 1);
            self.ui.set_color(Color::Success);
            self.ui.draw_text(box_x + 2, y - 1, "✓ Vérification OK. Le container peut être recréé.");
            y += 2;
            
            // Étape 4: Recréer le container
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y, &format!("Création du container '{}'...", container_name));
            y += 1;
            io::stdout().flush().unwrap();

            let lxc_deploy = LXCDeployment::new(container_name.clone(), "3.20".to_string());
            match lxc_deploy.create_container(&self.executor) {
                Ok(output) => {
                    if output.exit_code == Some(0) {
                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 2, y, "Container créé avec succès!");
                        y += 2;

                        // Étape 5: Démarrer le container
                        self.ui.set_color(Color::Info);
                        self.ui.draw_text(box_x + 2, y, "Démarrage du container...");
                        y += 1;
                        let _ = lxc_deploy.start_container(&self.executor);
                        std::thread::sleep(std::time::Duration::from_secs(2));

                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 2, y, "Container réinstallé et démarré avec succès!");
                    } else {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, y, "Erreur lors de la création du container.");
                        if !output.stderr.is_empty() {
                            self.ui.draw_text(box_x + 2, y + 1, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                        }
                    }
                }
                Err(e) => {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, &format!("Erreur: {}", e));
                }
            }

            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y + 3, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
        }
    }


    fn show_container_diagnostic(&mut self, _lxc_deploy: &LXCDeployment, box_x: u16, y: &mut u16) {
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, *y, "Diagnostic détaillé du container 'rmdb':");
        *y += 2;

        // Test 1: lxc-ls avec sudo
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 4, *y, "1. Test lxc-ls (avec sudo):");
        *y += 1;
        let cmd1 = "sudo -n lxc-ls -1 2>&1";
        match self.executor.run_shell(cmd1, true) {
            Ok(output) => {
                let found = output.stdout.lines().any(|line| line.trim() == "rmdb");
                if found {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 6, *y, "✓ Container trouvé");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 6, *y, "✗ Container non trouvé");
                    if !output.stdout.is_empty() {
                        self.ui.set_color(Color::Fg);
                        let preview = output.stdout.lines().take(3).collect::<Vec<_>>().join(", ");
                        self.ui.draw_text(box_x + 6, *y + 1, &format!("Containers vus: {}", preview));
                        *y += 1;
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 6, *y, &format!("✗ Erreur: {}", e));
            }
        }
        *y += 2;

        // Test 2: lxc-ls sans sudo
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 4, *y, "2. Test lxc-ls (sans sudo):");
        *y += 1;
        let cmd2 = "lxc-ls -1 2>&1";
        match self.executor.run_shell(cmd2, false) {
            Ok(output) => {
                let found = output.stdout.lines().any(|line| line.trim() == "rmdb");
                if found {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 6, *y, "✓ Container trouvé");
                } else {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 6, *y, "⚠ Container non trouvé (normal si permissions requises)");
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Warning);
                self.ui.draw_text(box_x + 6, *y, &format!("⚠ Erreur (attendu): {}", e));
            }
        }
        *y += 2;

        // Test 3: lxc list
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 4, *y, "3. Test lxc list:");
        *y += 1;
        let cmd3 = "lxc list --format csv -c n 2>&1";
        match self.executor.run_shell(cmd3, false) {
            Ok(output) => {
                let found = output.stdout.lines().any(|line| line.trim() == "rmdb" || line.contains("rmdb"));
                if found {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 6, *y, "✓ Container trouvé");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 6, *y, "✗ Container non trouvé");
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Warning);
                self.ui.draw_text(box_x + 6, *y, &format!("⚠ Erreur: {}", e));
            }
        }
        *y += 2;

        // Test 4: Système de fichiers
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 4, *y, "4. Test système de fichiers:");
        *y += 1;
        let paths = vec!["/var/lib/lxc/rmdb", "/var/lib/lxd/containers/rmdb"];
        let mut found_fs = false;
        for path in &paths {
            let cmd4 = format!("test -d {} && echo 'found' || echo 'not found'", path);
            match self.executor.run_shell(&cmd4, true) {
                Ok(output) => {
                    if output.stdout.contains("found") {
                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 6, *y, &format!("✓ Trouvé: {}", path));
                        found_fs = true;
                        *y += 1;
                    }
                }
                Err(_) => {}
            }
        }
        if !found_fs {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 6, *y, "✗ Container non trouvé dans le système de fichiers");
            *y += 1;
        }
        *y += 1;

        // Test 5: list_all_containers
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 4, *y, "5. Test list_all_containers():");
        *y += 1;
        match LXCDeployment::list_all_containers(&self.executor) {
            Ok(containers) => {
                let found = containers.iter().any(|c| c.name == "rmdb");
                if found {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 6, *y, &format!("✓ Container trouvé ({} containers au total)", containers.len()));
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 6, *y, &format!("✗ Container non trouvé ({} autres containers vus)", containers.len()));
                    if !containers.is_empty() {
                        self.ui.set_color(Color::Info);
                        self.ui.draw_text(box_x + 6, *y + 1, "Containers détectés:");
                        *y += 1;
                        for container in containers.iter().take(3) {
                            self.ui.draw_text(box_x + 8, *y, &format!("- {} ({})", container.name, container.status));
                            *y += 1;
                        }
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 6, *y, &format!("✗ Erreur: {}", e));
            }
        }
        *y += 2;

        // Résumé
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, *y, "Résumé:");
        *y += 1;
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 4, *y, "Si le container n'est pas trouvé par list_all_containers()");
        *y += 1;
        self.ui.draw_text(box_x + 4, *y, "mais existe dans le système de fichiers, il y a probablement");
        *y += 1;
        self.ui.draw_text(box_x + 4, *y, "un problème de permissions ou de configuration LXC.");
        *y += 2;
    }

    // ========== Fonctions de gestion RMDB sur le système hôte ==========

    fn host_install(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Installation RMDB sur le système hôte");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        // Vérifier si RMDB est déjà installé
        let host_deploy = HostDeployment::new();
        if host_deploy.check_rmdb_installed(&self.executor) {
            self.ui.set_color(Color::Warning);
            self.ui.draw_text(box_x + 2, y, "RMDB est déjà installé sur le système hôte.");
            y += 2;
            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 2, y, "Voulez-vous le réinstaller ?");
            y += 1;
            let confirm = self.ask_yes_no("Réinstallation", "RMDB est déjà installé. Voulez-vous le réinstaller ?");
            if !confirm {
                y += 2;
                self.ui.set_color(Color::Reset);
                self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                let _ = self.input_reader.read_key();
                return;
            }
        }

        // Trouver le répertoire source RMDB
        let rmdb_source = self.find_rmdb_source();
        if rmdb_source.is_none() {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 2, y, "✗ Erreur: Répertoire source RMDB introuvable.");
            y += 1;
            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 2, y, "Recherché dans:");
            y += 1;
            self.ui.draw_text(box_x + 4, y, "- ./rmdb_source");
            y += 1;
            self.ui.draw_text(box_x + 4, y, "- ../rmdb_source");
            y += 1;
            self.ui.draw_text(box_x + 4, y, "- /usr/local/share/rmdb/rmdb_source");
            y += 1;
            self.ui.draw_text(box_x + 4, y, "- /opt/rmdb/rmdb_source");
            y += 2;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        let rmdb_source_path = rmdb_source.unwrap();
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, &format!("Installation depuis: {}", rmdb_source_path));
        y += 2;

        // Installation
        match host_deploy.install_rmdb(&self.executor, &rmdb_source_path) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "✓ RMDB installé avec succès sur le système hôte!");
                    y += 2;
                    self.ui.set_color(Color::Info);
                    self.ui.draw_text(box_x + 2, y, "Binaire: /usr/local/bin/rmdbd");
                    y += 1;
                    self.ui.draw_text(box_x + 2, y, "Configuration: /etc/rmdbd/config.json");
                    y += 1;
                    self.ui.draw_text(box_x + 2, y, "Service: rmdbd (systemd ou OpenRC)");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "✗ Erreur lors de l'installation.");
                    if !output.stderr.is_empty() {
                        y += 1;
                        self.ui.set_color(Color::Fg);
                        self.ui.draw_text(box_x + 4, y, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    /// Affiche le menu d'installation
    fn show_install_menu(&mut self) {
        use crate::pres::install_menu::get_install_menu;
        let install_menu = get_install_menu();
        let mut selected = 0;
        let mut menu_offset = 0;

        loop {
            self.ui.clear_screen();
            self.ui.draw_header("Installation RMDB");

            let (box_x, box_y, box_w, box_h) = self.ui.get_box_dimensions();
            let mut y = box_y + 2;

            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 2, y, "Choisissez une option :");
            y += 2;

            // Afficher les options du menu
            let visible_items = (box_h as usize).saturating_sub(6).min(install_menu.len());
            let start = menu_offset.min(install_menu.len().saturating_sub(visible_items));

            for i in start..(start + visible_items).min(install_menu.len()) {
                let item = &install_menu[i];
                let prefix = if i == selected { "> " } else { "  " };
                let color = if i == selected { Color::Selection } else { Color::Fg };

                self.ui.set_color(color);
                self.ui.draw_text(box_x + 2, y, &format!("{}{}", prefix, item.label));
                y += 1;
            }

            // Instructions
            y = box_y + box_h - 3;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Flèches: Naviguer | Entrée: Sélectionner | Q: Retour");

            match self.input_reader.read_key() {
                Ok(Key::Quit) => break,
                Ok(Key::Up) => {
                    if selected > 0 {
                        selected -= 1;
                        if selected < menu_offset {
                            menu_offset = selected;
                        }
                    } else {
                        selected = install_menu.len() - 1;
                        menu_offset = selected.saturating_sub(visible_items - 1);
                    }
                }
                Ok(Key::Down) => {
                    if selected < install_menu.len() - 1 {
                        selected += 1;
                        if selected >= menu_offset + visible_items {
                            menu_offset = selected - visible_items + 1;
                        }
                    } else {
                        selected = 0;
                        menu_offset = 0;
                    }
                }
                Ok(Key::Enter) => {
                    use crate::pres::install_menu::InstallMenuAction;
                    match install_menu[selected].action {
                        InstallMenuAction::SelectInstallationMode => {
                            self.select_installation_mode();
                        }
                        InstallMenuAction::InstallOnHost => {
                            self.install_on_host();
                        }
                        InstallMenuAction::InstallInContainer => {
                            self.install_in_container();
                        }
                        InstallMenuAction::InstallInVM => {
                            self.install_in_vm();
                        }
                        InstallMenuAction::ConfigureInstallation => {
                            self.configure_installation();
                        }
                        InstallMenuAction::Back => break,
                    }
                }
                _ => {}
            }
        }
    }

    /// Installation sur le host (nouvelle version avec sélection du mode)
    fn install_on_host(&mut self) {
        self.host_install(); // Utilise la fonction existante qui a été mise à jour
    }

    /// Installation dans un container
    fn install_in_container(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Installation RMDB dans un container Alpine");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 3;

        // Vérifier les privilèges
        if !self.ensure_admin() {
            return;
        }

        // Demander le nom du container
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "Nom du container (par défaut: rmdb) :");
        y += 1;
        let container_name = "rmdb".to_string(); // TODO: Implémenter saisie interactive
        let alpine_version = "3.19".to_string();

        // Trouver le répertoire source
        let rmdb_source = match self.find_rmdb_source() {
            Some(path) => path,
            None => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, "✗ Répertoire source RMDB introuvable");
                y += 2;
                self.ui.set_color(Color::Reset);
                self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                let _ = self.input_reader.read_key();
                return;
            }
        };

        // Demander le mode d'utilisation
        let mode = self.get_installation_mode();
        if mode.is_none() {
            self.ui.set_color(Color::Warning);
            self.ui.draw_text(box_x + 2, y, "⚠ Mode d'utilisation non sélectionné. Installation annulée.");
            y += 2;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, &format!("Container : {}", container_name));
        y += 1;
        self.ui.draw_text(box_x + 2, y, &format!("Version Alpine : {}", alpine_version));
        y += 2;

        // Créer la configuration
        let logger = DeploymentLogger::new().unwrap_or_else(|_| DeploymentLogger::default());
        let mut config = InstallationConfig::new(InstallationType::ContainerAlpine, rmdb_source.clone())
            .with_logger(logger)
            .with_container_name(container_name.clone())
            .with_alpine_version(alpine_version)
            .with_rust_install(true)
            .with_go_install(true);
        
        if let Some(m) = mode {
            config = config.with_installation_mode(m);
        }

        self.ui.set_color(Color::Warning);
        self.ui.draw_text(box_x + 2, y, "Démarrer l'installation ? (O/n)");
        y += 1;
        self.ui.set_color(Color::Reset);

        match self.input_reader.read_key() {
            Ok(Key::Char('o')) | Ok(Key::Char('O')) | Ok(Key::Enter) => {
                self.ui.clear_screen();
                self.ui.draw_header("Installation en cours...");

                let installer = RMDBInstaller::new(config);
                match installer.install(&self.executor) {
                    Ok(output) => {
                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 2, box_y + 5, "✓ Installation terminée avec succès !");
                        if !output.stdout.is_empty() {
                            self.ui.set_color(Color::Fg);
                            self.ui.draw_text(box_x + 2, box_y + 7, &output.stdout);
                        }
                    }
                    Err(e) => {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, box_y + 5, &format!("✗ Erreur : {}", e));
                    }
                }

                self.ui.set_color(Color::Reset);
                self.ui.draw_text(box_x + 2, box_y + 10, "Appuyez sur une touche pour continuer...");
                let _ = self.input_reader.read_key();
            }
            _ => {}
        }
    }

    /// Installation dans une VM
    fn install_in_vm(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Installation RMDB dans une VM Rocky Linux");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 3;

        // Vérifier les privilèges
        if !self.ensure_admin() {
            return;
        }

        // Pour l'instant, utiliser des valeurs par défaut
        let vm_name = "rmdb-vm".to_string(); // TODO: Implémenter saisie interactive
        let rocky_version = "9".to_string();

        // Trouver le répertoire source
        let rmdb_source = match self.find_rmdb_source() {
            Some(path) => path,
            None => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, "✗ Répertoire source RMDB introuvable");
                y += 2;
                self.ui.set_color(Color::Reset);
                self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                let _ = self.input_reader.read_key();
                return;
            }
        };

        // Demander le mode d'utilisation
        let mode = self.get_installation_mode();
        if mode.is_none() {
            self.ui.set_color(Color::Warning);
            self.ui.draw_text(box_x + 2, y, "⚠ Mode d'utilisation non sélectionné. Installation annulée.");
            y += 2;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }

        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, &format!("VM : {}", vm_name));
        y += 1;
        self.ui.draw_text(box_x + 2, y, &format!("Version Rocky : {}", rocky_version));
        y += 2;

        // Créer la configuration
        let logger = DeploymentLogger::new().unwrap_or_else(|_| DeploymentLogger::default());
        let mut config = InstallationConfig::new(InstallationType::VMRocky, rmdb_source.clone())
            .with_logger(logger)
            .with_vm_name(vm_name.clone())
            .with_rocky_version(rocky_version)
            .with_rust_install(true)
            .with_go_install(true);
        
        if let Some(m) = mode {
            config = config.with_installation_mode(m);
        }

        self.ui.set_color(Color::Warning);
        self.ui.draw_text(box_x + 2, y, "Démarrer l'installation ? (O/n)");
        y += 1;
        self.ui.set_color(Color::Reset);

        match self.input_reader.read_key() {
            Ok(Key::Char('o')) | Ok(Key::Char('O')) | Ok(Key::Enter) => {
                self.ui.clear_screen();
                self.ui.draw_header("Installation en cours...");

                let installer = RMDBInstaller::new(config);
                match installer.install(&self.executor) {
                    Ok(output) => {
                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 2, box_y + 5, "✓ Installation terminée avec succès !");
                        if !output.stdout.is_empty() {
                            self.ui.set_color(Color::Fg);
                            self.ui.draw_text(box_x + 2, box_y + 7, &output.stdout);
                        }
                    }
                    Err(e) => {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, box_y + 5, &format!("✗ Erreur : {}", e));
                    }
                }

                self.ui.set_color(Color::Reset);
                self.ui.draw_text(box_x + 2, box_y + 10, "Appuyez sur une touche pour continuer...");
                let _ = self.input_reader.read_key();
            }
            _ => {}
        }
    }

    fn host_status(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Statut RMDB sur le système hôte");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let host_deploy = HostDeployment::new();

        // Vérifier l'installation
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "Installation:");
        y += 1;
        if host_deploy.check_rmdb_installed(&self.executor) {
            self.ui.set_color(Color::Success);
            self.ui.draw_text(box_x + 4, y, "✓ RMDB est installé");
        } else {
            self.ui.set_color(Color::Error);
            self.ui.draw_text(box_x + 4, y, "✗ RMDB n'est pas installé");
            y += 2;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
            let _ = self.input_reader.read_key();
            return;
        }
        y += 2;

        // Statut du service
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "Statut du service:");
        y += 1;
        match host_deploy.get_rmdb_status(&self.executor) {
            Ok(status) => {
                if status == "active" || status == "running" {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 4, y, &format!("✓ Service actif ({})", status));
                } else {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 4, y, &format!("⚠ Service inactif ({})", status));
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 4, y, &format!("✗ Erreur: {}", e));
            }
        }
        y += 2;

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn host_start(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Démarrer RMDB sur le système hôte");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let host_deploy = HostDeployment::new();
        match host_deploy.start_rmdb(&self.executor) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "✓ RMDB démarré avec succès!");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "✗ Échec du démarrage.");
                    if !output.stderr.is_empty() {
                        y += 1;
                        self.ui.set_color(Color::Fg);
                        self.ui.draw_text(box_x + 4, y, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn host_stop(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Arrêter RMDB sur le système hôte");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let host_deploy = HostDeployment::new();
        match host_deploy.stop_rmdb(&self.executor) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "✓ RMDB arrêté avec succès!");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "✗ Échec de l'arrêt.");
                    if !output.stderr.is_empty() {
                        y += 1;
                        self.ui.set_color(Color::Fg);
                        self.ui.draw_text(box_x + 4, y, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn host_restart(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Redémarrer RMDB sur le système hôte");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let host_deploy = HostDeployment::new();
        match host_deploy.restart_rmdb(&self.executor) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "✓ RMDB redémarré avec succès!");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "✗ Échec du redémarrage.");
                    if !output.stderr.is_empty() {
                        y += 1;
                        self.ui.set_color(Color::Fg);
                        self.ui.draw_text(box_x + 4, y, &output.stderr.lines().next().unwrap_or("Erreur inconnue"));
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn host_enable(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Activer RMDB au démarrage");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let host_deploy = HostDeployment::new();
        match host_deploy.enable_rmdb(&self.executor) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "✓ RMDB activé au démarrage!");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "✗ Échec de l'activation.");
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn host_disable(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Désactiver RMDB au démarrage");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let host_deploy = HostDeployment::new();
        match host_deploy.disable_rmdb(&self.executor) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "✓ RMDB désactivé au démarrage!");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "✗ Échec de la désactivation.");
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    fn host_uninstall(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Désinstaller RMDB du système hôte");
        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 5;

        let confirm = self.ask_yes_no("Désinstallation", "Êtes-vous sûr de vouloir désinstaller RMDB du système hôte ? Cette action est irréversible.");
        if !confirm {
            return;
        }

        let host_deploy = HostDeployment::new();
        match host_deploy.uninstall_rmdb(&self.executor) {
            Ok(output) => {
                if output.exit_code == Some(0) {
                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, "✓ RMDB désinstallé avec succès!");
                } else {
                    self.ui.set_color(Color::Error);
                    self.ui.draw_text(box_x + 2, y, "✗ Échec de la désinstallation.");
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    /// Trouve le répertoire source RMDB
    fn find_rmdb_source(&self) -> Option<String> {
        let possible_paths = vec![
            "./rmdb_source",
            "../rmdb_source",
            "/usr/local/share/rmdb/rmdb_source",
            "/opt/rmdb/rmdb_source",
        ];

        for path in &possible_paths {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }

        None
    }

    /// Obtient le mode d'installation sélectionné (ou demande à l'utilisateur)
    fn get_installation_mode(&mut self) -> Option<InstallationMode> {
        // Pour l'instant, on demande toujours à l'utilisateur
        // En production, on pourrait lire depuis un fichier de configuration
        use crate::pres::install_menu::get_mode_selection_menu;
        let modes = get_mode_selection_menu();
        let mut selected = 0;

        self.ui.clear_screen();
        self.ui.draw_header("Sélection du mode d'utilisation");

        let (box_x, box_y, box_w, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 2;

        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "Choisissez le mode d'utilisation de RMDB :");
        y += 2;

        loop {
            // Afficher les options
            let visible_items = (box_h as usize).saturating_sub(6).min(modes.len());
            let start = 0.min(modes.len().saturating_sub(visible_items));
            let mut display_y = box_y + 4;

            for i in start..(start + visible_items).min(modes.len()) {
                let (mode, label) = &modes[i];
                let prefix = if i == selected { "> " } else { "  " };
                let color = if i == selected { Color::Selection } else { Color::Fg };

                self.ui.set_color(color);
                self.ui.draw_text(box_x + 2, display_y, &format!("{}{}", prefix, label));
                display_y += 1;
            }

            // Instructions
            let inst_y = box_y + box_h - 3;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, inst_y, "Flèches: Naviguer | Entrée: Sélectionner");

            match self.input_reader.read_key() {
                Ok(Key::Up) => {
                    if selected > 0 {
                        selected -= 1;
                    } else {
                        selected = modes.len() - 1;
                    }
                }
                Ok(Key::Down) => {
                    if selected < modes.len() - 1 {
                        selected += 1;
                    } else {
                        selected = 0;
                    }
                }
                Ok(Key::Enter) => {
                    let (selected_mode, _) = &modes[selected];
                    return Some(*selected_mode);
                }
                Ok(Key::Quit) => {
                    return None;
                }
                _ => {}
            }
        }
    }

    /// Sélection du mode d'installation
    fn select_installation_mode(&mut self) {
        use crate::pres::install_menu::get_mode_selection_menu;
        let modes = get_mode_selection_menu();
        let mut selected = 0;
        let mut menu_offset = 0;

        loop {
            self.ui.clear_screen();
            self.ui.draw_header("Sélection du mode d'utilisation");

            let (box_x, box_y, _box_w, box_h) = self.ui.get_box_dimensions();
            let mut y = box_y + 2;

            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 2, y, "Choisissez le mode d'utilisation de RMDB :");
            y += 2;

            // Afficher les options du menu
            let visible_items = (box_h as usize).saturating_sub(6).min(modes.len());
            let start = menu_offset.min(modes.len().saturating_sub(visible_items));

            for i in start..(start + visible_items).min(modes.len()) {
                let (_mode, label) = &modes[i];
                let prefix = if i == selected { "> " } else { "  " };
                let color = if i == selected { Color::Selection } else { Color::Fg };

                self.ui.set_color(color);
                self.ui.draw_text(box_x + 2, y, &format!("{}{}", prefix, label));
                y += 1;
            }

            // Instructions
            y = box_y + box_h - 3;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Flèches: Naviguer | Entrée: Sélectionner | Q: Retour");

            match self.input_reader.read_key() {
                Ok(Key::Quit) => break,
                Ok(Key::Up) => {
                    if selected > 0 {
                        selected -= 1;
                        if selected < menu_offset {
                            menu_offset = selected;
                        }
                    } else {
                        selected = modes.len() - 1;
                        menu_offset = selected.saturating_sub(visible_items - 1);
                    }
                }
                Ok(Key::Down) => {
                    if selected < modes.len() - 1 {
                        selected += 1;
                        if selected >= menu_offset + visible_items {
                            menu_offset = selected - visible_items + 1;
                        }
                    } else {
                        selected = 0;
                        menu_offset = 0;
                    }
                }
                Ok(Key::Enter) => {
                    let (selected_mode, _) = &modes[selected];
                    
                    // Afficher confirmation
                    self.ui.clear_screen();
                    self.ui.draw_header("Mode sélectionné");

                    let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
                    let mut y = box_y + 3;

                    self.ui.set_color(Color::Success);
                    self.ui.draw_text(box_x + 2, y, &format!("✓ Mode sélectionné : {}", selected_mode.display_name()));
                    y += 2;

                    self.ui.set_color(Color::Fg);
                    self.ui.draw_text(box_x + 2, y, "Ce mode sera utilisé lors de l'installation.");
                    y += 1;
                    self.ui.draw_text(box_x + 2, y, "Vous pourrez le modifier dans la configuration.");
                    y += 2;

                    self.ui.set_color(Color::Info);
                    self.ui.draw_text(box_x + 2, y, "Le mode sera appliqué lors de l'installation de RMDB.");
                    y += 2;

                    self.ui.set_color(Color::Reset);
                    self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
                    let _ = self.input_reader.read_key();
                    break;
                }
                _ => {}
            }
        }
    }

    /// Configuration de l'installation
    fn configure_installation(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Configuration de l'installation");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 3;

        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "Configuration actuelle :");
        y += 2;
        self.ui.draw_text(box_x + 4, y, "Rust requis : 1.70.0+");
        y += 1;
        self.ui.draw_text(box_x + 4, y, "Go requis : 1.21.0+");
        y += 2;
        self.ui.draw_text(box_x + 2, y, "Cette fonctionnalité sera implémentée prochainement.");
        y += 2;

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    /// Affiche la liste des VMs
    fn show_vms_list(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Liste des Machines Virtuelles");

        let (box_x, box_y, _box_w, box_h) = self.ui.get_box_dimensions();
        let mut y = box_y + 2;

        // Créer le client API (par défaut localhost:8080)
        let api_client = APIClient::new("http://localhost:8080".to_string());

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Chargement des VMs...");
        y += 1;

        match api_client.get_vms(None) {
            Ok(vms) => {
                if vms.is_empty() {
                    self.ui.set_color(Color::Warning);
                    self.ui.draw_text(box_x + 2, y, "Aucune VM trouvée.");
                    y += 2;
                } else {
                    self.ui.set_color(Color::Fg);
                    self.ui.draw_text(box_x + 2, y, &format!("Total: {} VM(s)", vms.len()));
                    y += 2;

                    // Afficher les VMs (limité à la taille de l'écran)
                    let max_items = (box_h as usize).saturating_sub(8).min(vms.len());
                    for (i, vm) in vms.iter().take(max_items).enumerate() {
                        self.ui.set_color(Color::Fg);
                        self.ui.draw_text(box_x + 2, y, &format!("{}. {}", i + 1, vm.name));
                        y += 1;
                        self.ui.set_color(Color::Info);
                        self.ui.draw_text(box_x + 4, y, &format!("ID: {} | Catégorie: {} | Format: {}", 
                            vm.id, vm.category, vm.format));
                        y += 1;
                        if !vm.description.is_empty() {
                            self.ui.set_color(Color::Fg);
                            self.ui.draw_text(box_x + 4, y, &format!("Description: {}", vm.description));
                            y += 1;
                        }
                        y += 1;
                    }

                    if vms.len() > max_items {
                        self.ui.set_color(Color::Warning);
                        self.ui.draw_text(box_x + 2, y, &format!("... et {} VM(s) supplémentaire(s)", vms.len() - max_items));
                        y += 1;
                    }
                }
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur lors du chargement: {}", e));
                y += 1;
                self.ui.set_color(Color::Fg);
                self.ui.draw_text(box_x + 4, y, "Assurez-vous que le serveur RMDB est démarré");
                y += 1;
                self.ui.draw_text(box_x + 4, y, "et accessible sur http://localhost:8080");
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    /// Affiche le formulaire de création de VM
    fn show_vm_create(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Créer une Machine Virtuelle");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 3;

        // Créer le client API
        let api_client = APIClient::new("http://localhost:8080".to_string());

        // Récupérer les catégories disponibles
        let categories = match api_client.get_vm_categories() {
            Ok(cats) => cats,
            Err(_) => vec![],
        };

        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "Remplissez les informations de la VM :");
        y += 2;

        // Nom de la VM
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Nom de la VM :");
        y += 1;
        self.ui.set_color(Color::Fg);
        let vm_name = self.read_text_input(box_x + 4, y, 40);
        if vm_name.is_empty() {
            self.show_message("Erreur", "Le nom de la VM est requis.");
            return;
        }
        y += 2;

        // Description
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Description (optionnel) :");
        y += 1;
        self.ui.set_color(Color::Fg);
        let description = self.read_text_input(box_x + 4, y, 40);
        y += 2;

        // Catégorie
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Catégorie :");
        y += 1;
        let category = if !categories.is_empty() {
            // Afficher les catégories disponibles
            self.ui.set_color(Color::Fg);
            for (i, cat) in categories.iter().enumerate() {
                self.ui.draw_text(box_x + 4, y, &format!("{}. {}", i + 1, cat.name));
                y += 1;
            }
            self.ui.draw_text(box_x + 4, y, "0. Aucune catégorie");
            y += 1;
            self.ui.set_color(Color::Info);
            self.ui.draw_text(box_x + 2, y, "Choisissez une catégorie (numéro) :");
            y += 1;
            let choice = self.read_text_input(box_x + 4, y, 5);
            if let Ok(idx) = choice.parse::<usize>() {
                if idx > 0 && idx <= categories.len() {
                    categories[idx - 1].name.clone()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            self.ui.set_color(Color::Fg);
            let cat = self.read_text_input(box_x + 4, y, 40);
            y += 1;
            cat
        };
        y += 2;

        // Format
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Format (qcow2, raw, vmdk, vhd) :");
        y += 1;
        self.ui.set_color(Color::Fg);
        let format = self.read_text_input(box_x + 4, y, 10);
        if format.is_empty() {
            self.show_message("Erreur", "Le format est requis.");
            return;
        }
        y += 2;

        // Chemin du disque
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Chemin du fichier disque :");
        y += 1;
        self.ui.set_color(Color::Fg);
        let disk_path = self.read_text_input(box_x + 4, y, 60);
        if disk_path.is_empty() {
            self.show_message("Erreur", "Le chemin du disque est requis.");
            return;
        }
        y += 2;

        // Confirmation
        self.ui.set_color(Color::Warning);
        self.ui.draw_text(box_x + 2, y, "Créer cette VM ? (O/n)");
        y += 1;
        self.ui.set_color(Color::Reset);

        match self.input_reader.read_key() {
            Ok(Key::Char('o')) | Ok(Key::Char('O')) | Ok(Key::Enter) => {
                self.ui.clear_screen();
                self.ui.draw_header("Création en cours...");

                match api_client.create_vm(&vm_name, &description, &category, &format, &disk_path) {
                    Ok(vm) => {
                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 2, box_y + 5, &format!("✓ VM '{}' créée avec succès !", vm.name));
                        self.ui.set_color(Color::Info);
                        self.ui.draw_text(box_x + 2, box_y + 7, &format!("ID: {}", vm.id));
                    }
                    Err(e) => {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, box_y + 5, &format!("✗ Erreur lors de la création: {}", e));
                    }
                }

                self.ui.set_color(Color::Reset);
                self.ui.draw_text(box_x + 2, box_y + 10, "Appuyez sur une touche pour continuer...");
                let _ = self.input_reader.read_key();
            }
            _ => {}
        }
    }

    /// Lit une entrée texte de l'utilisateur (version simplifiée)
    fn read_text_input(&mut self, x: u16, y: u16, _max_len: usize) -> String {
        // Version simplifiée : afficher un prompt et lire ligne par ligne
        // TODO: Implémenter une vraie saisie interactive avec curseur
        self.ui.set_color(Color::Fg);
        self.ui.draw_text(x, y, "> ");
        
        // Pour l'instant, utiliser une méthode simple
        // En production, on utiliserait une bibliothèque comme tui-rs ou crossterm
        let mut buffer = String::new();
        let mut done = false;

        while !done {
            match self.input_reader.read_key() {
                Ok(Key::Char(c)) if c.is_ascii() && c != '\n' && c != '\r' => {
                    buffer.push(c);
                    self.ui.draw_text(x + 2 + buffer.len() as u16 - 1, y, &c.to_string());
                }
                Ok(Key::Backspace) if !buffer.is_empty() => {
                    buffer.pop();
                    // Effacer la ligne et réafficher
                    self.ui.draw_text(x, y, &format!("> {}{}", buffer, " "));
                }
                Ok(Key::Enter) => {
                    done = true;
                }
                Ok(Key::Quit) => {
                    buffer.clear();
                    done = true;
                }
                _ => {}
            }
        }

        buffer.trim().to_string()
    }

    /// Affiche le menu de gestion des VMs
    fn show_vms_manage(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Gestion des Machines Virtuelles");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 3;

        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, "Cette fonctionnalité sera implémentée prochainement.");
        y += 1;
        self.ui.draw_text(box_x + 2, y, "Pour l'instant, utilisez l'interface web ou l'API REST.");
        y += 2;

        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    /// Affiche la gestion des overlays de VMs
    fn show_vm_overlays(&mut self) {
        let api_client = APIClient::new("http://localhost:8080".to_string());
        
        // Charger la liste des overlays
        let overlays = match api_client.get_overlays() {
            Ok(overlays) => overlays,
            Err(e) => {
                self.show_error_message("Erreur", &format!("Impossible de charger les overlays: {}", e));
                return;
            }
        };

        if overlays.is_empty() {
            self.show_message("Overlays", "Aucun overlay disponible.");
            return;
        }

        // Menu de sélection d'overlay
        let mut selected = 0;
        let mut menu_offset = 0;

        loop {
            self.ui.clear_screen();
            self.ui.draw_header("Gestion des Overlays de VMs");

            let (box_x, box_y, _box_w, box_h) = self.ui.get_box_dimensions();
            let mut y = box_y + 2;

            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 2, y, &format!("Total: {} overlay(s)", overlays.len()));
            y += 2;

            // Afficher les overlays
            let visible_items = (box_h as usize).saturating_sub(8).min(overlays.len());
            let start = menu_offset.min(overlays.len().saturating_sub(visible_items));

            for i in start..(start + visible_items).min(overlays.len()) {
                let overlay = &overlays[i];
                let prefix = if i == selected { "> " } else { "  " };
                let color = if i == selected { Color::Selection } else { Color::Fg };

                self.ui.set_color(color);
                self.ui.draw_text(box_x + 2, y, &format!("{}MAC: {} | VM ID: {}", prefix, overlay.mac_address, overlay.vm_id));
                y += 1;
                self.ui.set_color(Color::Info);
                self.ui.draw_text(box_x + 4, y, &format!("Chemin: {} | Taille: {} octets", overlay.overlay_path, overlay.size));
                y += 1;
            }

            // Instructions
            y = box_y + box_h - 4;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Flèches: Naviguer | Entrée: Détails | S=Supprimer | Q=Retour");
            y += 1;
            self.ui.draw_text(box_x + 2, y, "C=Créer overlay | M=Rechercher par MAC");

            match self.input_reader.read_key() {
                Ok(Key::Quit) => break,
                Ok(Key::Up) => {
                    if selected > 0 {
                        selected -= 1;
                        if selected < menu_offset {
                            menu_offset = selected;
                        }
                    } else {
                        selected = overlays.len() - 1;
                        menu_offset = selected.saturating_sub(visible_items - 1);
                    }
                }
                Ok(Key::Down) => {
                    if selected < overlays.len() - 1 {
                        selected += 1;
                        if selected >= menu_offset + visible_items {
                            menu_offset = selected - visible_items + 1;
                        }
                    } else {
                        selected = 0;
                        menu_offset = 0;
                    }
                }
                Ok(Key::Enter) => {
                    self.show_overlay_details(&overlays[selected]);
                }
                Ok(Key::Char('s')) | Ok(Key::Char('S')) => {
                    if self.ask_yes_no("Suppression", &format!("Supprimer l'overlay pour MAC '{}' ?", overlays[selected].mac_address)) {
                        match api_client.delete_overlay(&overlays[selected].id) {
                            Ok(_) => {
                                self.show_message("Succès", &format!("Overlay supprimé avec succès."));
                                break; // Retour au menu principal
                            }
                            Err(e) => {
                                self.show_error_message("Erreur", &format!("Impossible de supprimer: {}", e));
                            }
                        }
                    }
                }
                Ok(Key::Char('c')) | Ok(Key::Char('C')) => {
                    self.create_overlay_interactive();
                    break; // Retour au menu principal après création
                }
                Ok(Key::Char('m')) | Ok(Key::Char('M')) => {
                    self.search_overlay_by_mac();
                }
                _ => {}
            }
        }
    }

    /// Affiche les détails d'un overlay
    fn show_overlay_details(&mut self, overlay: &VMOverlay) {
        self.ui.clear_screen();
        self.ui.draw_header(&format!("Détails Overlay: {}", overlay.mac_address));

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 3;

        self.ui.set_color(Color::Fg);
        self.ui.draw_text(box_x + 2, y, &format!("ID: {}", overlay.id));
        y += 1;
        self.ui.draw_text(box_x + 2, y, &format!("VM ID: {}", overlay.vm_id));
        y += 1;
        self.ui.draw_text(box_x + 2, y, &format!("MAC Address: {}", overlay.mac_address));
        y += 1;
        self.ui.draw_text(box_x + 2, y, &format!("Chemin: {}", overlay.overlay_path));
        y += 1;
        self.ui.draw_text(box_x + 2, y, &format!("Taille: {} octets", overlay.size));
        y += 1;
        self.ui.draw_text(box_x + 2, y, &format!("Créé le: {}", overlay.created_at));
        y += 1;
        self.ui.draw_text(box_x + 2, y, &format!("Modifié le: {}", overlay.updated_at));

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }

    /// Crée un overlay de manière interactive
    fn create_overlay_interactive(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Créer un Overlay de VM");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 3;

        let api_client = APIClient::new("http://localhost:8080".to_string());

        // Charger les VMs pour sélection
        let vms = match api_client.get_vms(None) {
            Ok(vms) => vms,
            Err(e) => {
                self.show_error_message("Erreur", &format!("Impossible de charger les VMs: {}", e));
                return;
            }
        };

        if vms.is_empty() {
            self.show_message("Erreur", "Aucune VM disponible. Créez d'abord une VM.");
            return;
        }

        // Sélection de la VM
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Sélectionnez une VM:");
        y += 2;

        let mut selected_vm = 0;
        loop {
            for (i, vm) in vms.iter().enumerate() {
                let prefix = if i == selected_vm { "> " } else { "  " };
                let color = if i == selected_vm { Color::Selection } else { Color::Fg };
                self.ui.set_color(color);
                self.ui.draw_text(box_x + 2, y + i as u16, &format!("{}{}", prefix, vm.name));
            }

            match self.input_reader.read_key() {
                Ok(Key::Up) => {
                    if selected_vm > 0 {
                        selected_vm -= 1;
                    } else {
                        selected_vm = vms.len() - 1;
                    }
                }
                Ok(Key::Down) => {
                    if selected_vm < vms.len() - 1 {
                        selected_vm += 1;
                    } else {
                        selected_vm = 0;
                    }
                }
                Ok(Key::Enter) => break,
                Ok(Key::Quit) => return,
                _ => {}
            }
        }

        y += vms.len() as u16 + 2;

        // Saisie de l'adresse MAC
        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Adresse MAC (format: XX:XX:XX:XX:XX:XX):");
        y += 1;
        self.ui.set_color(Color::Fg);
        let mac_address = self.read_text_input(box_x + 4, y, 20);
        if mac_address.is_empty() {
            self.show_message("Erreur", "L'adresse MAC est requise.");
            return;
        }

        // Confirmation
        y += 2;
        self.ui.set_color(Color::Warning);
        self.ui.draw_text(box_x + 2, y, "Créer cet overlay ? (O/n)");
        y += 1;
        self.ui.set_color(Color::Reset);

        match self.input_reader.read_key() {
            Ok(Key::Char('o')) | Ok(Key::Char('O')) | Ok(Key::Enter) => {
                self.ui.clear_screen();
                self.ui.draw_header("Création en cours...");

                match api_client.create_overlay(&vms[selected_vm].id, &mac_address) {
                    Ok(overlay) => {
                        self.ui.set_color(Color::Success);
                        self.ui.draw_text(box_x + 2, box_y + 5, &format!("✓ Overlay créé avec succès !"));
                        self.ui.set_color(Color::Info);
                        self.ui.draw_text(box_x + 2, box_y + 7, &format!("MAC: {}", overlay.mac_address));
                    }
                    Err(e) => {
                        self.ui.set_color(Color::Error);
                        self.ui.draw_text(box_x + 2, box_y + 5, &format!("✗ Erreur: {}", e));
                    }
                }

                self.ui.set_color(Color::Reset);
                self.ui.draw_text(box_x + 2, box_y + 10, "Appuyez sur une touche pour continuer...");
                let _ = self.input_reader.read_key();
            }
            _ => {}
        }
    }

    /// Recherche un overlay par MAC address
    fn search_overlay_by_mac(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Rechercher Overlay par MAC");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 3;

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Adresse MAC (format: XX:XX:XX:XX:XX:XX):");
        y += 1;
        self.ui.set_color(Color::Fg);
        let mac = self.read_text_input(box_x + 4, y, 20);

        if mac.is_empty() {
            return;
        }

        let api_client = APIClient::new("http://localhost:8080".to_string());
        match api_client.get_overlay_by_mac(&mac) {
            Ok(overlay) => {
                self.show_overlay_details(&overlay);
            }
            Err(e) => {
                self.show_error_message("Erreur", &format!("Overlay non trouvé: {}", e));
            }
        }
    }

    /// Menu pour les modules avancés (Repair, Test, Security)
    fn show_advanced_modules_menu(&mut self) {
        let mut selected = 0;
        let modules = vec![
            ("Réparation Système", "Repair"),
            ("Tests Système", "Test"),
            ("Sécurité", "Security"),
        ];

        loop {
            self.ui.clear_screen();
            self.ui.draw_header("Modules Avancés");

            let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
            let mut y = box_y + 3;

            for (i, (name, _)) in modules.iter().enumerate() {
                let prefix = if i == selected { "> " } else { "  " };
                let color = if i == selected { Color::Selection } else { Color::Fg };

                self.ui.set_color(color);
                self.ui.draw_text(box_x + 2, y, &format!("{}{}", prefix, name));
                y += 1;
            }

            y += 2;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Flèches: Naviguer | Entrée: Sélectionner | Q: Retour");

            match self.input_reader.read_key() {
                Ok(Key::Quit) => break,
                Ok(Key::Up) => {
                    if selected > 0 {
                        selected -= 1;
                    } else {
                        selected = modules.len() - 1;
                    }
                }
                Ok(Key::Down) => {
                    if selected < modules.len() - 1 {
                        selected += 1;
                    } else {
                        selected = 0;
                    }
                }
                Ok(Key::Enter) => {
                    match modules[selected].1 {
                        "Repair" => self.show_repair_module(),
                        "Test" => self.show_test_module(),
                        "Security" => self.show_security_module(),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    /// Affiche le module de réparation
    fn show_repair_module(&mut self) {
        let mut selected = 0;
        let repair_types = vec![
            ("DNS Resolution", "dns"),
            ("Network Connectivity", "network"),
            ("Configuration", "config"),
            ("Services", "services"),
        ];

        loop {
            self.ui.clear_screen();
            self.ui.draw_header("Module de Réparation");

            let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
            let mut y = box_y + 3;

            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 2, y, "Sélectionnez le type de réparation:");
            y += 2;

            for (i, (name, _)) in repair_types.iter().enumerate() {
                let prefix = if i == selected { "> " } else { "  " };
                let color = if i == selected { Color::Selection } else { Color::Fg };

                self.ui.set_color(color);
                self.ui.draw_text(box_x + 2, y, &format!("{}{}", prefix, name));
                y += 1;
            }

            y += 2;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Flèches: Naviguer | Entrée: Exécuter | Q: Retour");

            match self.input_reader.read_key() {
                Ok(Key::Quit) => break,
                Ok(Key::Up) => {
                    if selected > 0 {
                        selected -= 1;
                    } else {
                        selected = repair_types.len() - 1;
                    }
                }
                Ok(Key::Down) => {
                    if selected < repair_types.len() - 1 {
                        selected += 1;
                    } else {
                        selected = 0;
                    }
                }
                Ok(Key::Enter) => {
                    let api_client = APIClient::new("http://localhost:8080".to_string());
                    self.ui.clear_screen();
                    self.ui.draw_header("Réparation en cours...");

                    match api_client.run_repair(repair_types[selected].1) {
                        Ok(result) => {
                            if result.success {
                                self.ui.set_color(Color::Success);
                                self.ui.draw_text(box_x + 2, box_y + 5, &format!("✓ {}", result.message));
                            } else {
                                self.ui.set_color(Color::Error);
                                self.ui.draw_text(box_x + 2, box_y + 5, &format!("✗ {}", result.message));
                            }
                            if let Some(ref details) = result.details {
                                self.ui.set_color(Color::Fg);
                                self.ui.draw_text(box_x + 2, box_y + 7, details);
                            }
                        }
                        Err(e) => {
                            self.ui.set_color(Color::Error);
                            self.ui.draw_text(box_x + 2, box_y + 5, &format!("✗ Erreur: {}", e));
                        }
                    }

                    self.ui.set_color(Color::Reset);
                    self.ui.draw_text(box_x + 2, box_y + 10, "Appuyez sur une touche pour continuer...");
                    let _ = self.input_reader.read_key();
                }
                _ => {}
            }
        }
    }

    /// Affiche le module de test
    fn show_test_module(&mut self) {
        let mut selected = 0;
        let test_types = vec![
            ("Test Unitaires", "unit"),
            ("Test Intégration", "integration"),
            ("Test Connectivité", "connectivity"),
            ("Test Performance", "performance"),
        ];

        loop {
            self.ui.clear_screen();
            self.ui.draw_header("Module de Test");

            let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
            let mut y = box_y + 3;

            self.ui.set_color(Color::Fg);
            self.ui.draw_text(box_x + 2, y, "Sélectionnez le type de test:");
            y += 2;

            for (i, (name, _)) in test_types.iter().enumerate() {
                let prefix = if i == selected { "> " } else { "  " };
                let color = if i == selected { Color::Selection } else { Color::Fg };

                self.ui.set_color(color);
                self.ui.draw_text(box_x + 2, y, &format!("{}{}", prefix, name));
                y += 1;
            }

            y += 2;
            self.ui.set_color(Color::Reset);
            self.ui.draw_text(box_x + 2, y, "Flèches: Naviguer | Entrée: Exécuter | Q: Retour");

            match self.input_reader.read_key() {
                Ok(Key::Quit) => break,
                Ok(Key::Up) => {
                    if selected > 0 {
                        selected -= 1;
                    } else {
                        selected = test_types.len() - 1;
                    }
                }
                Ok(Key::Down) => {
                    if selected < test_types.len() - 1 {
                        selected += 1;
                    } else {
                        selected = 0;
                    }
                }
                Ok(Key::Enter) => {
                    let api_client = APIClient::new("http://localhost:8080".to_string());
                    self.ui.clear_screen();
                    self.ui.draw_header("Test en cours...");

                    match api_client.run_test(test_types[selected].1) {
                        Ok(result) => {
                            if result.success {
                                self.ui.set_color(Color::Success);
                                self.ui.draw_text(box_x + 2, box_y + 5, &format!("✓ {}", result.message));
                            } else {
                                self.ui.set_color(Color::Error);
                                self.ui.draw_text(box_x + 2, box_y + 5, &format!("✗ {}", result.message));
                            }
                            if let Some(ref details) = result.details {
                                self.ui.set_color(Color::Fg);
                                self.ui.draw_text(box_x + 2, box_y + 7, details);
                            }
                            if let Some(duration) = result.duration_ms {
                                self.ui.set_color(Color::Info);
                                self.ui.draw_text(box_x + 2, box_y + 9, &format!("Durée: {} ms", duration));
                            }
                        }
                        Err(e) => {
                            self.ui.set_color(Color::Error);
                            self.ui.draw_text(box_x + 2, box_y + 5, &format!("✗ Erreur: {}", e));
                        }
                    }

                    self.ui.set_color(Color::Reset);
                    self.ui.draw_text(box_x + 2, box_y + 12, "Appuyez sur une touche pour continuer...");
                    let _ = self.input_reader.read_key();
                }
                _ => {}
            }
        }
    }

    /// Affiche le module de sécurité
    fn show_security_module(&mut self) {
        self.ui.clear_screen();
        self.ui.draw_header("Module de Sécurité");

        let (box_x, box_y, _, _) = self.ui.get_box_dimensions();
        let mut y = box_y + 2;

        let api_client = APIClient::new("http://localhost:8080".to_string());

        self.ui.set_color(Color::Info);
        self.ui.draw_text(box_x + 2, y, "Chargement des métriques de sécurité...");
        y += 1;

        match api_client.get_security_metrics() {
            Ok(metrics) => {
                self.ui.set_color(Color::Fg);
                self.ui.draw_text(box_x + 2, y, "Métriques de sécurité:");
                y += 2;

                self.ui.set_color(Color::Info);
                self.ui.draw_text(box_x + 4, y, &format!("Menaces détectées: {}", metrics.threats_detected));
                y += 1;
                self.ui.draw_text(box_x + 4, y, &format!("Menaces actives: {}", metrics.active_threats));
                y += 1;
                self.ui.draw_text(box_x + 4, y, &format!("IPs bloquées: {}", metrics.blocked_ips));
                y += 1;
                self.ui.draw_text(box_x + 4, y, &format!("Tentatives de connexion échouées: {}", metrics.failed_logins));
            }
            Err(e) => {
                self.ui.set_color(Color::Error);
                self.ui.draw_text(box_x + 2, y, &format!("✗ Erreur: {}", e));
            }
        }

        y += 2;
        self.ui.set_color(Color::Reset);
        self.ui.draw_text(box_x + 2, y, "Appuyez sur une touche pour continuer...");
        let _ = self.input_reader.read_key();
    }
}


use std::io::{self, Write};
use crate::pres::terminal::Terminal;

pub struct UI {
    pub terminal: Terminal,
}

impl UI {
    pub fn new() -> Self {
        Self {
            terminal: Terminal::new(),
        }
    }

    pub fn update_terminal_size(&mut self) {
        self.terminal.update_size();
    }

    pub fn get_box_dimensions(&self) -> (u16, u16, u16, u16) {
        self.terminal.get_box_dimensions()
    }

    pub fn get_max_visible_items(&self) -> usize {
        self.terminal.get_max_visible_items()
    }

    pub fn is_valid_size(&self) -> bool {
        self.terminal.is_valid_size()
    }

    pub fn is_optimal_size(&self) -> bool {
        self.terminal.is_optimal_size()
    }

    pub fn clear_screen(&self) {
        print!("\x1B[2J\x1B[H");
        io::stdout().flush().unwrap();
    }

    pub fn set_cursor(&self, x: u16, y: u16) {
        print!("\x1B[{};{}H", y + 1, x + 1);
        io::stdout().flush().unwrap();
    }

    pub fn hide_cursor(&self) {
        print!("\x1B[?25l");
        io::stdout().flush().unwrap();
    }

    pub fn show_cursor(&self) {
        print!("\x1B[?25h");
        io::stdout().flush().unwrap();
    }

    pub fn set_color(&self, color: Color) {
        match color {
            Color::Fg => print!("\x1B[38;2;224;224;224m"), // #e0e0e0 - Texte principal
            Color::Accent => print!("\x1B[38;2;68;58;134m"), // #443a86 - Bordure et titre
            Color::Success => print!("\x1B[38;2;68;255;68m"), // #44ff44 - Succès (Vert)
            Color::Error => print!("\x1B[38;2;225;69;44m"), // #e1452c - Erreur formelle (Rouge)
            Color::Warning => print!("\x1B[38;2;235;112;14m"), // #eb700e - Sortie négative (Orange)
            Color::Info => print!("\x1B[38;2;57;175;255m"), // #39afff - Information
            Color::Selection => print!("\x1B[38;2;254;208;39m"), // #fed027 - Sélection
            Color::Yellow => print!("\x1B[38;2;255;215;0m"), // #ffd700 - Jaune pour services désactivés
            Color::Reset => print!("\x1B[0m"),
        }
        io::stdout().flush().unwrap();
    }
    
    pub fn set_bg_color(&self, color: Color) {
        match color {
            Color::Accent => print!("\x1B[48;2;68;58;134m"), // #443a86 - Fond bordure/titre
            Color::Selection => print!("\x1B[48;2;254;208;39m"), // #fed027 - Fond sélection
            Color::Error => print!("\x1B[48;2;225;69;44m"), // #e1452c - Fond erreur
            Color::Warning => print!("\x1B[48;2;235;112;14m"), // #eb700e - Fond avertissement
            _ => {}
        }
        io::stdout().flush().unwrap();
    }

    pub fn draw_box(&self, x: u16, y: u16, w: u16, h: u16) {
        self.set_color(Color::Accent);
        
        self.set_cursor(x, y);
        print!("┌");
        for _ in 0..(w - 2) {
            print!("─");
        }
        print!("┐");
        
        for i in 1..(h - 1) {
            self.set_cursor(x, y + i);
            print!("│");
            self.set_cursor(x + w - 1, y + i);
            print!("│");
        }
        
        self.set_cursor(x, y + h - 1);
        print!("└");
        for _ in 0..(w - 2) {
            print!("─");
        }
        print!("┘");
        
        self.set_color(Color::Reset);
        io::stdout().flush().unwrap();
    }

    pub fn draw_text(&self, x: u16, y: u16, text: &str) {
        self.set_cursor(x, y);
        let max_width = (self.terminal.width().saturating_sub(x)) as usize;
        let display_text = if text.chars().count() > max_width {
            text.chars().take(max_width).collect::<String>()
        } else {
            text.to_string()
        };
        print!("{}", display_text);
        io::stdout().flush().unwrap();
    }
    
    pub fn draw_label_value(&self, x: u16, y: u16, label: &str, value: &str) {
        self.set_color(Color::Warning);
        self.draw_text(x, y, label);
        self.set_color(Color::Fg);
        self.draw_text(x + label.chars().count() as u16, y, value);
    }

    pub fn draw_button(&self, x: u16, y: u16, text: &str, selected: bool) {
        self.set_cursor(x, y);
        let max_width = (self.terminal.width().saturating_sub(x).saturating_sub(10)) as usize;
        let display_text = if text.chars().count() > max_width {
            format!("{}...", text.chars().take(max_width.saturating_sub(3)).collect::<String>())
        } else {
            text.to_string()
        };
        
        if selected {
            self.set_color(Color::Selection);
            print!("▶ {} ◀", display_text);
        } else {
            self.set_color(Color::Fg);
            print!("  {}  ", display_text);
        }
        self.set_color(Color::Reset);
        io::stdout().flush().unwrap();
    }

    pub fn clear_line(&self, y: u16) {
        self.set_cursor(0, y);
        print!("\x1B[2K");
        io::stdout().flush().unwrap();
    }

    pub fn draw_header(&self, title: &str) {
        let (box_x, box_y, box_w, _) = self.get_box_dimensions();
        let title_x = box_x;
        let title_y = box_y;
        
        let fill_width = box_w as usize;
        let title_line = format!("╔{}╗", "═".repeat(fill_width.saturating_sub(2)));
        let title_text = format!("║{:^width$}║", title, width = fill_width.saturating_sub(2));
        let title_bottom = format!("╚{}╝", "═".repeat(fill_width.saturating_sub(2)));
        
        self.set_color(Color::Accent);
        self.draw_text(title_x, title_y, &title_line);
        self.draw_text(title_x, title_y + 1, &title_text);
        self.draw_text(title_x, title_y + 2, &title_bottom);
        self.set_color(Color::Reset);
    }

    pub fn draw_status_bar(&self, y: u16, message: &str) {
        let max_width = (self.terminal.width().saturating_sub(10)) as usize;
        let display_msg = if message.chars().count() > max_width {
            format!("{}...", message.chars().take(max_width.saturating_sub(3)).collect::<String>())
        } else {
            message.to_string()
        };
        
        self.set_cursor(5, y);
        self.set_color(Color::Fg);
        print!("{}", display_msg);
        self.set_color(Color::Reset);
        io::stdout().flush().unwrap();
    }

    pub fn draw_scrollbar(&self, x: u16, y: u16, height: u16, total_items: usize, visible_items: usize, offset: usize) {
        if total_items <= visible_items {
            return;
        }
        
        self.set_color(Color::Accent);
        
        let scrollbar_height = height.saturating_sub(2);
        let thumb_height = ((scrollbar_height as f64 * visible_items as f64 / total_items as f64).ceil() as u16).max(1);
        let max_thumb_pos = scrollbar_height.saturating_sub(thumb_height);
        let thumb_pos = if total_items > visible_items {
            ((offset as f64 / (total_items - visible_items) as f64) * max_thumb_pos as f64).round() as u16
        } else {
            0
        };
        
        for i in 0..scrollbar_height {
            self.set_cursor(x, y + 1 + i);
            print!("│");
        }
        
        self.set_color(Color::Selection);
        for i in 0..thumb_height {
            if thumb_pos + i < scrollbar_height {
                self.set_cursor(x, y + 1 + thumb_pos + i);
                print!("█");
            }
        }
        
        self.set_color(Color::Reset);
        io::stdout().flush().unwrap();
    }
}

#[derive(Copy, Clone)]
pub enum Color {
    Fg,           // Texte principal (#e0e0e0)
    Accent,       // Bordure et titre (#443a86)
    Success,      // Succès (#44ff44) - Vert
    Error,        // Erreur formelle (#e1452c) - Rouge
    Warning,      // Sortie négative (#eb700e) - Orange
    Info,         // Information (#39afff)
    Selection,    // Sélection (#fed027) - Jaune
    Yellow,       // Jaune pour services désactivés (#ffd700)
    Reset,        // Réinitialisation
}


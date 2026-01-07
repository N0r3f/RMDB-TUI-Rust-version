use std::process::Command;
use std::str;
use std::process::Stdio;

pub struct Terminal {
    width: u16,
    height: u16,
    pub min_width: u16,
    pub min_height: u16,
    pub optimal_width: u16,
    pub optimal_height: u16,
}

/// Active un mode terminal adapté au TUI (équivalent de `stty -echo -icanon min 1 time 0`).
/// Le mode est restauré automatiquement au drop.
pub struct RawModeGuard {
    original: Option<String>,
}

impl RawModeGuard {
    fn run_stty(args: &[&str]) -> bool {
        // IMPORTANT: stty agit sur le TTY associé à stdin → on force l’héritage de stdin
        // pour que le binaire (sans run.sh) fonctionne de la même manière.
        let candidates = ["/usr/bin/stty", "/bin/stty", "stty"];
        for cmd in candidates {
            let out = Command::new(cmd)
                .args(args)
                .stdin(Stdio::inherit())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output();
            if let Ok(o) = out {
                if o.status.success() {
                    return true;
                }
            }
        }
        false
    }

    fn get_stty_state() -> Option<String> {
        let candidates = ["/usr/bin/stty", "/bin/stty", "stty"];
        for cmd in candidates {
            let out = Command::new(cmd)
                .args(["-g"])
                .stdin(Stdio::inherit())
                .output();
            if let Ok(o) = out {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
        }
        None
    }

    pub fn enable() -> Self {
        // Sauvegarder l’état courant (best-effort)
        let original = Self::get_stty_state();

        // Appliquer un mode "raw" robuste (empêche l’écho et fournit les flèches en séquences)
        // 1) Essayer `stty raw -echo` (le plus direct)
        // 2) Fallback: équivalent run.sh
        if !Self::run_stty(&["raw", "-echo"]) {
            let _ = Self::run_stty(&["-echo", "-icanon", "min", "1", "time", "0"]);
        }

        Self { original }
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if let Some(orig) = self.original.as_deref() {
            // Restaurer exactement l’état initial
            let _ = Self::run_stty(&[orig]);
        } else {
            // fallback : au moins réactiver un mode sane
            let _ = Self::run_stty(&["sane"]);
        }
    }
}

impl Terminal {
    pub fn new() -> Self {
        let (width, height) = Self::get_size();
        Self {
            width,
            height,
            min_width: 80,
            min_height: 24,
            optimal_width: 100,
            optimal_height: 30,
        }
    }

    pub fn get_size() -> (u16, u16) {
        if let Ok(output) = Command::new("tput")
            .arg("cols")
            .output()
        {
            if let Ok(cols_str) = str::from_utf8(&output.stdout) {
                if let Ok(cols) = cols_str.trim().parse::<u16>() {
                    if let Ok(output) = Command::new("tput")
                        .arg("lines")
                        .output()
                    {
                        if let Ok(lines_str) = str::from_utf8(&output.stdout) {
                            if let Ok(lines) = lines_str.trim().parse::<u16>() {
                                return (cols, lines);
                            }
                        }
                    }
                }
            }
        }
        (80, 24)
    }

    pub fn update_size(&mut self) {
        let (w, h) = Self::get_size();
        self.width = w;
        self.height = h;
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn is_valid_size(&self) -> bool {
        self.width >= self.min_width && self.height >= self.min_height
    }

    pub fn is_optimal_size(&self) -> bool {
        self.width >= self.optimal_width && self.height >= self.optimal_height
    }

    pub fn try_resize(&self, width: u16, height: u16) -> bool {
        print!("\x1B[8;{};{}t", height, width);
        std::io::Write::flush(&mut std::io::stdout()).ok();
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        let (new_w, new_h) = Self::get_size();
        new_w == width && new_h == height
    }

    pub fn request_optimal_size(&mut self) -> bool {
        if !self.is_optimal_size() {
            self.try_resize(self.optimal_width, self.optimal_height)
        } else {
            true
        }
    }

    pub fn get_box_dimensions(&self) -> (u16, u16, u16, u16) {
        // Plein écran : la box occupe toute la zone disponible.
        // Les fonctions d’UI gèrent ensuite les marges internes (header/status/footer).
        (0, 0, self.width.max(1), self.height.max(1))
    }

    pub fn get_max_visible_items(&self) -> usize {
        let (_, _, _, box_height) = self.get_box_dimensions();
        (box_height - 10).max(5) as usize
    }
    
    pub fn calculate_required_height(&self, content_lines: usize, header_lines: usize, footer_lines: usize) -> u16 {
        let margins = 4;
        let borders = 2;
        let required = (content_lines + header_lines + footer_lines + margins + borders) as u16;
        required.max(self.min_height)
    }
    
    pub fn resize_for_content(&mut self, content_lines: usize, header_lines: usize, footer_lines: usize) -> bool {
        let required_height = self.calculate_required_height(content_lines, header_lines, footer_lines);
        let required_width = self.optimal_width;
        
        if required_height > self.height || required_width != self.width {
            self.try_resize(required_width, required_height)
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_creation() {
        let term = Terminal::new();
        assert!(term.width > 0);
        assert!(term.height > 0);
    }
}

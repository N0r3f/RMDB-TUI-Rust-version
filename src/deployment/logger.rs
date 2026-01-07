use std::fs::{File, OpenOptions};
use std::io::{Write, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct DeploymentLogger {
    log_file: Mutex<BufWriter<File>>,
    log_dir: PathBuf,
}

impl DeploymentLogger {
    pub fn new() -> Result<Self, std::io::Error> {
        // Créer le répertoire de logs
        let log_dir = PathBuf::from("/var/log/rmdb");
        if !log_dir.exists() {
            std::fs::create_dir_all(&log_dir)?;
        }

        // Créer le fichier de log avec timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let log_file_path = log_dir.join(format!("deployment_{}.log", timestamp));
        
        // Si on ne peut pas écrire dans /var/log, utiliser le répertoire courant
        let (log_dir, log_file_path) = match File::create(&log_file_path) {
            Ok(_) => (log_dir, log_file_path),
            Err(_) => {
                // Fallback vers le répertoire courant
                let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let fallback_dir = current_dir.join("logs");
                std::fs::create_dir_all(&fallback_dir).ok();
                let fallback_path = fallback_dir.join(format!("deployment_{}.log", timestamp));
                (fallback_dir, fallback_path)
            }
        };

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)?;

        let writer = BufWriter::new(file);

        Ok(Self {
            log_file: Mutex::new(writer),
            log_dir,
        })
    }

    fn log(&self, level: &str, message: &str) {
        // Utiliser SystemTime pour éviter la dépendance chrono
        let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => {
                let secs = duration.as_secs();
                let nanos = duration.subsec_nanos();
                format!("{}.{:03}", secs, nanos / 1_000_000)
            }
            Err(_) => "0.000".to_string(),
        };
        let log_line = format!("[{}] [{}] {}\n", timestamp, level, message);
        
        // Écrire dans le fichier
        if let Ok(mut writer) = self.log_file.lock() {
            let _ = writer.write_all(log_line.as_bytes());
            let _ = writer.flush();
        }

        // Aussi écrire sur stderr pour le débogage
        eprintln!("{}", log_line.trim());
    }

    pub fn info(&self, message: &str) {
        self.log("INFO", message);
    }

    pub fn warn(&self, message: &str) {
        self.log("WARN", message);
    }

    pub fn error(&self, message: &str) {
        self.log("ERROR", message);
    }

    pub fn debug(&self, message: &str) {
        self.log("DEBUG", message);
    }

    pub fn step(&self, step: u32, total: u32, message: &str) {
        let msg = format!("[Étape {}/{}] {}", step, total, message);
        self.info(&msg);
    }

    pub fn command(&self, cmd: &str) {
        self.debug(&format!("Exécution: {}", cmd));
    }

    pub fn command_output(&self, stdout: &str, stderr: &str, exit_code: Option<i32>) {
        if !stdout.is_empty() {
            for line in stdout.lines() {
                self.debug(&format!("STDOUT: {}", line));
            }
        }
        if !stderr.is_empty() {
            for line in stderr.lines() {
                self.warn(&format!("STDERR: {}", line));
            }
        }
        if let Some(code) = exit_code {
            if code != 0 {
                self.error(&format!("Code de sortie: {}", code));
            } else {
                self.debug(&format!("Code de sortie: {}", code));
            }
        }
    }

    pub fn log_path(&self) -> &Path {
        &self.log_dir
    }
}

impl Default for DeploymentLogger {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback silencieux si on ne peut pas créer le logger
            // On utilisera un logger nul
            panic!("Impossible de créer le logger de déploiement");
        })
    }
}

// Logger nul pour les cas où on ne peut pas créer de fichier
pub struct NullLogger;

impl NullLogger {
    pub fn new() -> Self {
        Self
    }

    pub fn info(&self, _message: &str) {}
    pub fn warn(&self, _message: &str) {}
    pub fn error(&self, _message: &str) {}
    pub fn debug(&self, _message: &str) {}
    pub fn step(&self, _step: u32, _total: u32, _message: &str) {}
    pub fn command(&self, _cmd: &str) {}
    pub fn command_output(&self, _stdout: &str, _stderr: &str, _exit_code: Option<i32>) {}
}


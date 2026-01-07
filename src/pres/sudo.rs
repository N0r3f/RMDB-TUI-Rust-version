use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Maintient la session sudo vivante (timestamp) sans stocker de mot de passe.
/// Le keep-alive est stoppé automatiquement au drop.
pub struct SudoKeepAliveGuard {
    stop: Arc<AtomicBool>,
    needs_reauth: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl SudoKeepAliveGuard {
    pub fn start(interval: Duration) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let needs_reauth = Arc::new(AtomicBool::new(false));
        let needs_reauth_thread = needs_reauth.clone();

        let handle = thread::spawn(move || {
            // Keep-alive best-effort : tant que l’app tourne, on refresh le timestamp sudo.
            while !stop_thread.load(Ordering::Relaxed) {
                // IMPORTANT: non-interactif pour ne jamais bloquer le TUI.
                // Si le timestamp a expiré, sudo renvoie une erreur et on marque “réauth requise”.
                let ok = Command::new("sudo")
                    .args(["-n", "-v"])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if !ok {
                    needs_reauth_thread.store(true, Ordering::Relaxed);
                }
                // Sleep par petits morceaux pour réagir vite au stop
                let mut slept = Duration::from_secs(0);
                while slept < interval && !stop_thread.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(250));
                    slept += Duration::from_millis(250);
                }
            }
        });

        Self {
            stop,
            needs_reauth,
            handle: Some(handle),
        }
    }

    pub fn needs_reauth(&self) -> bool {
        self.needs_reauth.load(Ordering::Relaxed)
    }

    pub fn clear_reauth_flag(&self) {
        self.needs_reauth.store(false, Ordering::Relaxed);
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for SudoKeepAliveGuard {
    fn drop(&mut self) {
        self.stop();
    }
}



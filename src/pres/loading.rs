use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Exécute une tâche potentiellement longue dans un thread et permet de poller l’avancement via timeout.
/// Renvoie (durée, résultat).
pub fn run_with_spinner<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> (Duration, T) {
    let (tx, rx) = mpsc::channel::<T>();
    let start = Instant::now();

    thread::spawn(move || {
        let r = work();
        let _ = tx.send(r);
    });

    // Attente bloquante côté appelant (le rendu du spinner est géré dans MainApp)
    // Ici, on attend juste le résultat.
    let r = rx.recv().expect("worker thread dropped");
    (start.elapsed(), r)
}



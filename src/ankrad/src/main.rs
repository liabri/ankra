//! Entry point and configuration loop orchestrator for the `ankrad` daemon.
//!
//! Filesystem-Driven IPC
//! Leverages flat files under `$XDG_DATA_HOME` combined with the kernel's native filesystem events
//! for cross-process control. This design allows `ankra-cli` to mutate states asynchronously
//! without requiring complex runtime socket listeners, custom network protocols, or serialization overhead.
//!
//! Lock-Free State Sharing (`Arc<AtomicBool>`)
//! The global active toggle state is read constantly on every single keyboard interaction but updated
//! rarely. Utilizing a relaxed atomic memory ordering avoids heavy context switching and lock contention
//! (`Mutex`/`RwLock`), keeping the critical typing execution path running at raw machine speed.

extern crate ankra_wayland;
mod logger;

use std::fs::read_to_string;
use std::sync::atomic::{ AtomicBool, Ordering };
use std::sync::Arc;
use std::path::Path;
use anyhow::Result;
use notify::{ Watcher, RecursiveMode, EventKind };

fn main() -> Result<()> {
    logger::init("debug").map_err(|err| eprintln!("logger failed to initialise: {:?}", err)).unwrap();

    let xdg_dirs = xdg::BaseDirectories::with_prefix("ankra")?;
    let data_home = xdg_dirs.get_data_home();

    // initialize shared state flag
    let status_path = data_home.join("status");
    let initial_status = read_to_string(&status_path).unwrap_or_else(|_| "on".to_string());
    let is_active = Arc::new(AtomicBool::new(initial_status.trim() == "on"));

    // spawn inotify background watcher
    let watcher_flag = Arc::clone(&is_active);
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        match res {
            Ok(event) => {
                // if the file was modified, update memory flag
                if event.kind.is_modify() {
                    if let Ok(new_status) = read_to_string(&status_path) {
                        let active = new_status.trim() == "on";
                        watcher_flag.store(active, Ordering::Relaxed);
                        log::info!("Status changed via CLI. Engine Active: {}", active);
                    }
                }
            }
            Err(e) => log::error!("Watch error: {:?}", e),
        }
    })?;

    // tell the kernel to monitor the specific status file
    // (if the file doesn't exist yet, might need to watch the `data_home` directory instead)
    let _ = watcher.watch(&data_home, RecursiveMode::NonRecursive);

    // pass it a clone of the atomic pointer
    let layout_path = data_home.join("current_layout");
    let id = read_to_string(&layout_path).unwrap_or_else(|_| "cangjie5".to_string());

    log::info!("Starting Ankra Input Daemon ...");

    let mut state = ankra_wayland::State::new(&id, is_active);
    state.run();

    Ok(())
}

//! Public interface and façade for the Ankra core state machine.

mod error;
pub use error::AnkraError;

mod table;
use table::TableState;
#[doc(hidden)]
pub use table::Entry;

use std::path::PathBuf;

pub struct AnkraEngine {
    table: TableState,
}

impl AnkraEngine {
    pub fn new(mut cfg: AnkraConfig) -> Self {
        //rid id of non visible characters such as "\n"
        cfg.id.retain(|c| !c.is_whitespace());
        let table = TableState::new(&cfg.id, &cfg.dir).unwrap();

        AnkraEngine { table }
    }

    pub fn on_key_press(&mut self, key_code: u16, level: usize) -> AnkraResponse {
       	self.table.on_key_press(key_code, level)
    }

    pub fn on_key_release(&mut self, key_code: u16, level: usize) -> AnkraResponse {
       	self.table.on_key_release(key_code, level)
    }

    pub fn reset(&mut self) {
    	self.table.reset();
    }

    /// Checks if we have hit the deep-focus threshold (e.g., 500)
    pub fn uncommitted_weight_mutations(&self) -> u32 {
        self.table.uncommitted_weight_mutations
    }

    /// Triggers the actual disk write
    pub fn flush(&mut self) {
        if let Err(e) = self.table.flush_to_disk() {
            log::error!("Failed to flush weights to disk: {}", e);
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum AnkraResponse {
    Commit(String),
    Suggest(String),
    Undefined, //Error
    Empty, //KeyCode found but didnt have anything to return eg. function keys
}

pub struct AnkraConfig {
    pub id: String,
    pub dir: PathBuf
}

impl Default for AnkraConfig {
    fn default() -> Self {
        AnkraConfig {
            dir: xdg::BaseDirectories::with_prefix("ankra").unwrap().get_config_home(),
            id: "layout id was not defined".to_string()
        }
    }
}

impl Drop for AnkraEngine {
    fn drop(&mut self) {
        // a graceful exit commits any uncommitted weight mutations if > 1
        if self.uncommitted_weight_mutations() > 0 {
            log::info!("Shutting down: Flushing remaining dictionary weights to disk...");
            self.flush();
        }
    }
}

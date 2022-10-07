mod error;
pub use error::AnkraError;

mod table;
use table::TableState;

use std::path::PathBuf;

pub struct AnkraEngine {
    table: TableState,
    cfg: AnkraConfig,
}

impl AnkraEngine {
    pub fn new(mut cfg: AnkraConfig) -> Self {
        //rid id of non visible characters such as "\n"
        cfg.id.retain(|c| !c.is_whitespace());
        let table = TableState::new(&cfg.id, &cfg.dir).unwrap();

        AnkraEngine { table, cfg }
    }

    pub fn on_key_press(&mut self, key_code: u16) -> AnkraResponse {
    	self.table.on_key_press(key_code)
    }

    pub fn on_key_release(&mut self, key_code: u16) -> AnkraResponse {    	
    	self.table.on_key_release(key_code)
    }

    pub fn reset(&mut self) {
    	self.table.reset();
    }
}

#[derive(Debug, PartialEq)]
pub enum AnkraResponse {
    Commit(String),
    Suggest(String),
    Undefined, //Error
    Empty, //KeyCode found but didnt have anything to return, intentional (like special keys eg. Han key)
}

pub struct AnkraConfig {
    pub id: String,
    pub dir: PathBuf
}

impl Default for AnkraConfig {
    fn default() -> Self {
        AnkraConfig {
        	dir: std::path::PathBuf::from("/home/liabri/.config/ankra"),
            // dir: xdg::BaseDirectories::with_prefix("benten").unwrap().get_config_home(),
            id: "layout id was not defined".to_string()
        }
    }
}
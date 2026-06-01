//! Pure state engine for table-based string translation and candidate lookup.
//!
//! ## Core State Mechanics & Design Rules
//!
//! ### 1. Environmental Decoupling
//! This core has zero awareness of Wayland, window focus, or I/O multiplexers.
//! It processes raw input primitives, making it hermetically testable using isolated
//! data mock fixtures without touching global user directories.
//!
//! ### 2. Predictive Prefix-Matching & Complexity ($O(N)$ Filtering)
//! Candidate matching utilizes a linear prefix scan (`starts_with`) across the entire
//! dictionary array on every non-control keystroke. This $O(N)$ traversal avoids the
//! pointer indirection and memory overhead of a prefix trie, prioritizing layout
//! predictability and straightforward incremental sequence matching.
//!
//! ### 3. Declarative Response Architecture
//! Modifications evaluate immediately into a high-level primitive layout wrapper (`AnkraResponse`).
//! This abstracts candidate matching logic away from the UI, delegating text composition
//! rendering and insertion rules entirely to outer protocol layers.
//!
//! ### 4. Viewport Memory Management
//! State transformations divide strictly to minimize processing overhead:
//! * **Structural Shifts (Keystrokes):** Triggers a full cache eviction, forcing
//!   a dynamic re-population of `relative_entries` and resetting the viewport pointer (`index = 0`).
//! * **Navigation Shifts (Page/Digit Jumps):** Operates entirely as a stateless mutation
//!   of the viewport pointer across the pre-filtered array, shielding layout navigation
//!   from allocation or database search overhead.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use crate::{ AnkraError, AnkraResponse };
use std::fs::File;
use std::io::BufReader;

type KeyCode = u16;

#[derive(Default)]
pub struct TableState {
	pub table: Table,
    pub config: TableConfig,
    pub key_sequence: String,
    pub index: usize,
	pub relative_entries: Vec<Entry>,
    pub previous_character: String,
    pub weights: HashMap<char, u32>,
    pub weights_path: std::path::PathBuf,
}

// feature: copy previous character key bind, kinda like a repition mark, will need a var "previous character" buf in TableMethod
impl TableState {
    pub fn new(id: &str, path: &Path) -> Result<Self, AnkraError> {
        Ok(Self {
            table: Table::from_path(id, path)?,
            config: TableConfig::from_path(id, path)?,
            ..Default::default()
        })
    }

    pub fn on_key_press(&mut self, key_code: u16, level: usize) -> AnkraResponse {
        let mut commit = false;
        let mut raw_commit = false; // flag to track explicit Enter bypass
        let mut is_control = false; // add a flag to track control keys

        match self.config.keycode_to_spec(&key_code, level).and_then(|x| x.chars().next()) {
            Some('C') => {
                commit = true;
                is_control = true;
            }

            Some('N') => {
                is_control = true;
                if self.index + 1 < self.relative_entries.len() {
                    self.index += 1;
                }
            }

            Some('P') => {
                is_control = true;
                if self.index != 0 {
                    self.index -= 1;
                }
            }

            Some('E') => {
                if !self.key_sequence.is_empty() {
                    self.reset();
                    return AnkraResponse::Empty;
                }
            }

            Some('R') => {
                raw_commit = true;
                is_control = true;
            }

            Some('B') => {
                self.key_sequence.pop();
                self.relative_entries.clear();
            }

            Some(x @ '0'..='9') => {
                is_control = true;
                self.index = (x as usize) - 49;
            }

            _ => {
                if let Some(c) = self.config.keycode_to_char(&key_code, level) {
                    self.key_sequence.push(*c);
                }
            }
        }

        // only rebuild relative entries if this wasn't a static control action
        if !is_control {
            self.index = 0; // reset active item index on a fresh character input entry
            self.relative_entries.clear(); // clear cache to rebuild for new sequence length

            for entry in &self.table.entries {
                if entry.sequence.starts_with(&self.key_sequence) {
                    self.relative_entries.push(entry.clone());
                }
            }
        }

        // resolve the string out of the filtered entries
        let value = if raw_commit {
            self.key_sequence.clone()
        } else if let Some(entry) = self.relative_entries.get(self.index) {
            entry.character.to_string()
        } else {
            self.key_sequence.clone()
        };

        if !self.key_sequence.is_empty() {
            if commit || raw_commit {
                self.reset();
                return AnkraResponse::Commit(value);
            } else {
                return AnkraResponse::Suggest(value);
            }
        }

        self.reset();
        AnkraResponse::Undefined
    }

    pub fn on_key_release(&mut self, _key_code: u16, _level: usize) -> AnkraResponse {
        AnkraResponse::Undefined
    }

    pub fn reset(&mut self) {
        self.index = 0;
        self.relative_entries.clear();
        self.key_sequence.clear();
        self.previous_character.clear();
    }
}

#[derive(Default, Debug, Deserialize)]
pub struct Table {
    pub entries: Vec<Entry>
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct Entry {
    pub character: char,
    pub sequence: String, // maybe try a tiny_string as this is needlessly large
}

impl Table {
    pub fn from_path(id: &str, base_dir: &Path) -> Result<Self, AnkraError> {
        let path = base_dir.join(id).join("table").with_extension("csv");
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let entries = csv::Reader::from_reader(reader).deserialize().collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            entries,
        })
    }
}

#[derive(Default, Debug, Deserialize)]
pub struct TableConfig {
    pub specs: HashMap<KeyCode, Vec<String>>,
    pub keys: HashMap<KeyCode, Vec<char>>,
}

impl TableConfig {
    pub fn from_path(id: &str, base_dir: &Path) -> Result<Self, AnkraError> {
        let path = base_dir.join(id).join("config").with_extension("zm");
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(zmerald::from_reader(reader).unwrap())
    }

    pub fn keycode_to_char(&self, keycode: &KeyCode, level: usize) -> Option<&char> {
        self.keys.get(keycode)?.get(level)
    }

    pub fn keycode_to_spec(&self, keycode: &KeyCode, level: usize) -> Option<&str> {
        self.specs.get(keycode)?.get(level).map(|x| &**x)
    }
}

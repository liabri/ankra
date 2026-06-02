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
//!   a dynamic re-population of `relative_indices` and resetting the viewport pointer (`index = 0`).
//! * **Navigation Shifts (Page/Digit Jumps):** Operates entirely as a stateless mutation
//!   of the viewport pointer across the pre-filtered array, shielding layout navigation
//!   from allocation or database search overhead.

use serde::{ Deserialize, Serialize };
use std::collections::HashMap;
use std::path::{ Path, PathBuf };
use crate::{ AnkraError, AnkraResponse };
use std::fs::File;
use std::io::{ BufReader, BufWriter };

type KeyCode = u16;

#[derive(Default)]
pub struct TableState {
	pub table: Table,
    pub config: TableConfig,
    pub key_sequence: String,
    pub index: usize,
    pub relative_indices: Vec<usize>,
    pub previous_character: String,
    pub uncommitted_weight_mutations: u32,
    pub layout_dir: PathBuf
}

// feature: copy previous character key bind, kinda like a repition mark, will need a var "previous character" buf in TableMethod
impl TableState {
    pub fn new(id: &str, path: &Path) -> Result<Self, AnkraError> {
        Ok(Self {
            table: Table::from_path(id, path)?,
            config: TableConfig::from_path(id, path)?,
            layout_dir: path.join(id),
            ..Default::default()
        })
    }

    pub fn on_key_press(&mut self, key_code: u16, level: usize) -> AnkraResponse {
        let was_empty = self.key_sequence.is_empty();

        #[derive(Debug, PartialEq)]
        enum Intent {
            Commit,
            CommitAndPass,
            RawCommit,
            Navigate,
            Backspace,
            Type(char),
            NoOp,
            PassThrough,
        }

        let intent = match self.config.keycode_to_spec(&key_code, level) {
            Some("COMMIT") => Intent::Commit,
            Some("RAWCOMMIT") => Intent::RawCommit,
            Some("BACKSPACE") => Intent::Backspace,
            Some("COMMITANDPASS") => if was_empty { Intent::PassThrough } else { Intent::CommitAndPass },
            Some("NEXT") => {
                if self.index + 1 < self.relative_indices.len() { self.index += 1; }
                Intent::Navigate
            }

            Some("PREV") => {
                if self.index > 0 { self.index -= 1; }
                Intent::Navigate
            }

            Some("ESCAPE") => {
                if !was_empty {
                    self.reset();
                    return AnkraResponse::Empty;
                }
                Intent::PassThrough
            }

            Some(x) if x.len() == 1 && x.chars().next().unwrap().is_ascii_digit() => {
                let digit_char = x.chars().next().unwrap();
                let requested_index = match digit_char {
                    '1'..='9' => (digit_char as usize) - 49,
                    '0' => 9,
                    _ => unreachable!(),
                };
                if requested_index < self.relative_indices.len() {
                    self.index = requested_index;
                }
                Intent::Navigate
            }

            _ => match self.config.keycode_to_char(&key_code, level) {
                Some(&c) => Intent::Type(c),
                None => if was_empty { Intent::PassThrough } else { Intent::NoOp },
            }
        };

        // early exit for hardware passthrough
        if matches!(intent, Intent::PassThrough) {
            return AnkraResponse::Undefined;
        }

        // if typing a new character, trigger the O(N) predictive dictionary rebuild
        if matches!(intent, Intent::Type(_) | Intent::Backspace | Intent::NoOp) {
            match intent {
                Intent::Type(c) => self.key_sequence.push(c),
                Intent::Backspace => { self.key_sequence.pop(); },
                _ => {}
            }

            self.index = 0;
            self.relative_indices.clear();

            // only sweep the dictionary if there's an active sequence to evaluate
            if !self.key_sequence.is_empty() {
                let mut exact_matches = Vec::new();
                let mut prefix_suggestions = Vec::new();

                for (i, entry) in self.table.entries.iter().enumerate() {
                    if *entry.sequence == self.key_sequence {
                        exact_matches.push(i);
                    } else if entry.sequence.starts_with(&self.key_sequence) {
                        prefix_suggestions.push(i);
                    }
                }

                self.relative_indices = exact_matches;
                self.relative_indices.extend(prefix_suggestions);
            }
        }

        // pre-calculate commitment state
        let is_committing = matches!(intent, Intent::Commit | Intent::CommitAndPass);
        let is_raw = matches!(intent, Intent::RawCommit);

        // resolve the string value and apply weight mutations
        let value = if is_raw {
            self.key_sequence.clone()
        } else if let Some(&main_idx) = self.relative_indices.get(self.index) {
            if is_committing {
                self.uncommitted_weight_mutations += 1;
                self.table.entries[main_idx].weight = self.table.entries[main_idx].weight.saturating_add(1);

                let mut curr = main_idx;
                while curr > 0 && self.table.entries[curr].weight > self.table.entries[curr - 1].weight {
                    self.table.entries.swap(curr, curr - 1);
                    curr -= 1;
                }
                self.table.entries[curr].character.to_string()
            } else {
                self.table.entries[main_idx].character.to_string()
            }
        } else {
            self.key_sequence.clone()
        };

        // yield the final declarative response
        if !self.key_sequence.is_empty() {
            if is_committing || is_raw {
                self.reset();
                if intent == Intent::CommitAndPass {
                    AnkraResponse::CommitAndPass(value)
                } else {
                    AnkraResponse::Commit(value)
                }
            } else {
                AnkraResponse::Suggest(value)
            }
        } else {
            self.reset();
            // if the buffer was already empty before this stroke, let the key fall out to the OS natively
            if intent == Intent::CommitAndPass || was_empty {
                AnkraResponse::Undefined
            } else {
                AnkraResponse::Empty
            }
        }
    }

    pub fn on_key_release(&mut self, _key_code: u16, _level: usize) -> AnkraResponse {
        AnkraResponse::Undefined
    }

    pub fn reset(&mut self) {
        self.index = 0;
        self.relative_indices.clear();
        self.key_sequence.clear();
        self.previous_character.clear();
    }

    pub fn flush_to_disk(&mut self) -> Result<(), AnkraError> {
        if self.uncommitted_weight_mutations == 0 {
            return Ok(());
        }

        let table_path = self.layout_dir.join("chars.csv");
        let phrases_path = self.layout_dir.join("phrases.csv");

        // open raw files and wrap them in memory buffers to protect SSD
        let table_file = File::create(table_path)?;
        let phrases_file = File::create(phrases_path)?;

        let mut table_wtr = csv::Writer::from_writer(BufWriter::new(table_file));
        let mut phrases_wtr = csv::Writer::from_writer(BufWriter::new(phrases_file));

        for entry in &self.table.entries {
            // O(1), if the 2nd character does not exist, it's a single char.
            if entry.character.chars().nth(1).is_none() {
                table_wtr.serialize(entry)?;
            } else {
                phrases_wtr.serialize(entry)?;
            }
        }

        table_wtr.flush()?;
        phrases_wtr.flush()?;

        self.uncommitted_weight_mutations = 0;

        Ok(())
    }
}

#[derive(Default, Debug, Deserialize)]
pub struct Table {
    pub entries: Vec<Entry>
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct Entry {
    pub character: Box<str>,
    pub sequence: Box<str>,
    #[serde(default)]
    pub weight: u32,
}

impl Table {
    pub fn from_path(id: &str, base_dir: &Path) -> Result<Self, AnkraError> {
        let layout_dir = base_dir.join(id);
        let mut entries = Vec::new();

        let mut load_csv = |file_name: &str| -> Result<(), AnkraError> {
            let path = layout_dir.join(file_name);
            if path.exists() {
                let file = File::open(path)?;
                let reader = BufReader::new(file);

                let mut csv_reader = csv::ReaderBuilder::new()
                    .comment(Some(b'#'))
                    .from_reader(reader);

                for result in csv_reader.deserialize::<Entry>() {
                    if let Ok(entry) = result {
                        entries.push(entry);
                    }
                }
            }
            Ok(())
        };

        load_csv("chars.csv")?;
        load_csv("phrases.csv")?;

        // pre-ort descending by weight. if weights are tied, sort ascending by sequence length
        entries.sort_by(|a, b| {
            b.weight.cmp(&a.weight)
                .then_with(|| a.sequence.len().cmp(&b.sequence.len()))
        });

        Ok(Self { entries })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs;

    /// Helper function to build an isolated engine state in RAM
    fn mock_state() -> TableState {
        let mut config = TableConfig::default();

        // mock the 'a' key (let's say KeyCode 30)
        config.keys.insert(30, vec!['a']);
        config.keys.insert(31, vec!['b']);

        // Mock the Commit key ('C') (let's say KeyCode 28, usually Enter)
        config.specs.insert(28, vec!["COMMIT".to_string()]);

        let entries = vec![
            Entry { character: Box::from("啊"), sequence: Box::from("a"), weight: 0 },
            Entry { character: Box::from("阿"), sequence: Box::from("a"), weight: 0 },
            Entry { character: Box::from("哎"), sequence: Box::from("a"), weight: 5 },
        ];

        let mut state = TableState::default();
        state.config = config;
        state.table = Table { entries };
        // Pre-sort just like Table::from_path does
        state.table.entries.sort_by(|a, b| b.weight.cmp(&a.weight));

        state
    }

    #[test]
    fn test_memory_mutation_and_bubble_sort() {
        let mut state = mock_state();

        // Ensure "哎" is at index 0 because it starts with weight 5
        assert_eq!(&*state.table.entries[0].character, "哎");

        // Type 'a' (KeyCode 30)
        let res = state.on_key_press(30, 0);
        assert_eq!(res, AnkraResponse::Suggest("哎".to_string()));

        // Let's navigate down the candidate list using index
        state.index = 1; // Pointing to "啊" (Weight 0)

        // Commit the selection (KeyCode 28)
        let commit_res = state.on_key_press(28, 0);
        assert_eq!(commit_res, AnkraResponse::Commit("啊".to_string()));

        // ASSERTIONS
        // 1. The weight should have increased
        assert_eq!(state.uncommitted_weight_mutations, 1);

        // 2. "啊" should have bubbled up above "阿" but stay below "哎"
        assert_eq!(&*state.table.entries[0].character, "哎"); // Weight 5
        assert_eq!(&*state.table.entries[1].character, "啊"); // Weight 1
        assert_eq!(&*state.table.entries[2].character, "阿"); // Weight 0
    }

    #[test]
    fn test_split_flush_to_disk() {
        let temp_layout_dir = temp_dir().join("ankra_test_flush_dir");
        fs::create_dir_all(&temp_layout_dir).unwrap();

        let mut state = TableState::default();
        state.layout_dir = temp_layout_dir.clone();
        state.uncommitted_weight_mutations = 1; // Force the flush to trigger

        // Mix single chars and multi-char phrases together
        state.table.entries = vec![
            Entry { character: Box::from("的"), sequence: Box::from("d"), weight: 10 },
            Entry { character: Box::from("我的"), sequence: Box::from("wd"), weight: 5 },
        ];

        // Execute the flush
        let flush_res = state.flush_to_disk();
        assert!(flush_res.is_ok());

        // Read the isolated files back from the temp directory
        let chars_content = fs::read_to_string(temp_layout_dir.join("chars.csv")).unwrap();
        let phrases_content = fs::read_to_string(temp_layout_dir.join("phrases.csv")).unwrap();

        // ASSERTIONS
        // "的" should exclusively be in chars.csv
        assert!(chars_content.contains("的"));
        assert!(!chars_content.contains("我的"));

        // "我的" should exclusively be in phrases.csv
        assert!(phrases_content.contains("我的"));
        assert!(!phrases_content.contains("的,d,10")); // Exact match safety

        // The flush should have reset the tracker
        assert_eq!(state.uncommitted_weight_mutations, 0);

        // Teardown the mock directory
        fs::remove_dir_all(temp_layout_dir).unwrap();
    }
}

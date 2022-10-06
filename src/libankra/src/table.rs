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
    pub previous_character: String
}


//feature: copy previous character key bind, kinda like a repition mark, will need a var "previous character" buf in TableMethod
impl TableState {
    pub fn new(id: &str, path: &Path) -> Result<Self, AnkraError> {
        Ok(Self {
            table: Table::from_path(id, &path)?,
            config: TableConfig::from_path(id, &path)?,
            ..Default::default()
        })
    }

    pub fn on_key_press(&mut self, key_code: u16) -> AnkraResponse {
        let mut commit = false;
    	match self.config.keycode_to_spec(&key_code).as_deref() {
    		Some("COMMIT") => commit = true,
    		Some("NEXT") => self.index = self.index+1,
    		Some("PREV") => self.index = self.index-1,
            Some("BACKSPACE") => { 
                self.key_sequence.pop();
                self.relative_entries.clear(); 
            },
    		
            _ => {
                if let Some(c) = self.config.keycode_to_char(&key_code) {
                    self.key_sequence.push(*c);
                }
            }
    	}

        //change method when key sequence is empty on backspace
        // if !commit {
        //    return AnkraResponse::Function(Function::ChangeMethodTo(changemethodto))
        // }

        println!("key_sequence: {}", self.key_sequence);

        // get value from dict.csv
        let result = {
            if self.relative_entries.is_empty() {
                for entry in &self.table.entries {
                    if entry.sequence.starts_with(&self.key_sequence) {
                        self.relative_entries.push(entry.clone());
                    }
                }
            } else {          
                self.relative_entries.retain(|entry| entry.sequence.starts_with(&self.key_sequence));
            }

            println!("relative entries: {:?}", self.relative_entries);
            if let Some(entry) = self.relative_entries.get(self.index).map(|x| x.to_owned()) {
                Some(entry.character.to_string())
            } else {
                None
            }
        };

        // interpret value from dict.csv
        if let Some(value) = result {
            if commit {
                self.reset();
                return AnkraResponse::Commit(value)
            } else {
                return AnkraResponse::Suggest(value)
            }
        } else {
            self.reset();
            return AnkraResponse::Empty
        }
    }

    pub fn on_key_release(&mut self, key_code: u16) -> AnkraResponse {
        return AnkraResponse::Undefined
    }

    fn reset(&mut self) {
        self.index = 0;
        self.relative_entries.clear();
        self.key_sequence.clear();
        self.previous_character.clear();
    }
}

#[derive(Default, Debug, PartialEq, Deserialize)]
pub struct Table {
    pub id: String,
    pub entries: Vec<Entry>
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct Entry {
    pub character: char,
    pub sequence: String, //maybe try a tiny_string as this is needlessly large
}

impl Table {
    pub fn from_path(id: &str, base_dir: &Path) -> Result<Self, AnkraError> {
        let path = base_dir.join(id).with_extension("csv");
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let entries = csv::Reader::from_reader(reader).deserialize().collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            id: id.to_string(),
            entries,
        })
    }
}

#[derive(Default, Debug, PartialEq, Deserialize)]
pub struct TableConfig {
    pub specs: HashMap<KeyCode, Vec<String>>,
    pub keys: HashMap<KeyCode, Vec<char>>,
}

impl TableConfig {
    pub fn from_path(id: &str, base_dir: &Path) -> Result<Self, AnkraError> {
        let path = base_dir.join(id).with_extension("zm");
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(zmerald::from_reader(reader).unwrap())
    }

    pub fn keycode_to_char(&self, keycode: &KeyCode) -> Option<&char> {
        self.keys.get(keycode)?.first()
    }

    pub fn keycode_to_spec(&self, keycode: &KeyCode) -> Option<&str> {
        self.specs.get(keycode)?.first().map(|x| &**x)
    }
}
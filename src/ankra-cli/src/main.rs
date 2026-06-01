//! Command-line controller and configuration utility for the Ankra input daemon.
//!
//! ## Core Architecture & CLI Design Rules
//!
//! ### 1. Stateless IPC Signaling
//! Communicates with the daemon asynchronously by writing flat state variables (`status`,
//! `current_layout`) straight into the user's unified data runtime paths. This file-driven
//! methodology acts as a signaling channel that the active background process intercepts
//! utilizing filesystem watch hooks, completely eliminating network port or socket management overhead.
//!
//! ### 2. Dynamic Table Discovery (`lookup`)
//! Enforces strict layout agnosticism by scanning every CSV dictionary file present inside the
//! selected layout configuration path at runtime. Rather than hardcoding database schemas or file
//! targets, it dynamically parses and filters arbitrary sequential rows to find candidate keystroke sequences.
//!
//! ### 3. Out-of-Band State Diagnostics (`status`)
//! Verifies system health by aggregating filesystem configurations with process-level execution metrics
//! from the kernel. By checking layout files alongside low-level system checks (`pidof`), it
//! determines if the translation service is organically responsive or suspended.

mod cook;

use std::fs::{ create_dir_all, read_to_string, write };
use std::path::Path;
use std::process::{exit, Command};
use xdg::BaseDirectories;

pub fn set_layout(data_home: &Path, config_home: &Path, raw_layout_id: &str) -> Result<String, String> {
    let layout_id = raw_layout_id.trim();
    let target_layout_dir = config_home.join(layout_id);

    if !target_layout_dir.exists() || !target_layout_dir.is_dir() {
        return Err(format!(
            "Layout directory not found at '{}'. Please ensure it exists.",
            target_layout_dir.display()
        ));
    }

    write(data_home.join("current_layout"), layout_id)
        .map_err(|e| format!("Failed to write configuration to disk: {}", e))?;

    Ok(layout_id.to_string())
}

pub fn set_status(data_home: &Path, status: &str) -> Result<(), String> {
    write(data_home.join("status"), status)
        .map_err(|e| format!("Failed to commit mode state updates: {}", e))
}

/// Helper to read the active layout
pub fn get_current_layout(data_home: &Path) -> Result<String, String> {
    read_to_string(data_home.join("current_layout"))
        .map(|s| s.trim().to_string())
        .map_err(|_| "No active layout set. Please run 'ankra-cli layout <id>' first.".to_string())
}

/// Scans all CSV files in the target layout directory for a specific character/phrase
pub fn lookup_sequence(config_home: &Path, layout_id: &str, target: &str) -> Vec<String> {
    let layout_dir = config_home.join(layout_id);
    let mut results = Vec::new();

    if let Ok(entries) = std::fs::read_dir(layout_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("csv") {
                if let Ok(file) = std::fs::File::open(&path) {
                    let mut rdr = csv::Reader::from_reader(file);
                    for record in rdr.records().flatten() {
                        if record.len() >= 2 && record[0].trim() == target {
                            results.push(record[1].trim().to_string());
                        }
                    }
                }
            }
        }
    }

    results.sort();
    results.dedup();
    results
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_help();
        exit(1);
    }

    let xdg_dirs = BaseDirectories::with_prefix("ankra").expect("Failed to resolve system XDG environment definitions");
    let data_home = xdg_dirs.get_data_home();
    let config_home = xdg_dirs.get_config_home();

    if let Err(e) = create_dir_all(&data_home) {
        eprintln!("Error: Failed to provision target state directory: {}", e);
        exit(1);
    }

    match args[1].as_str() {
        "layout" => {
            if args.len() < 3 {
                eprintln!("Error: Missing explicit target layout ID parameter.");
                eprintln!("Usage: ankra-cli layout <layout_id>");
                exit(1);
            }

            match set_layout(&data_home, &config_home, &args[2]) {
                Ok(cleaned_id) => println!("Successfully migrated engine configuration to: '{}'", cleaned_id),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    exit(1);
                }
            }
        }

        "on" => {
            if let Err(e) = set_status(&data_home, "on") {
                eprintln!("Error: {}", e);
                exit(1);
            }
            println!("Ankra Input Engine Status: ACTIVE");
        }

        "off" => {
            if let Err(e) = set_status(&data_home, "off") {
                eprintln!("Error: {}", e);
                exit(1);
            }
            println!("Ankra Input Engine Status: PASSTHROUGH");
        }

        "status" => {
            let layout_id = get_current_layout(&data_home).unwrap_or_else(|_| "[NOT SET]".to_string());

            let mode = read_to_string(data_home.join("status"))
                .unwrap_or_else(|_| "on".to_string())
                .trim()
                .to_string();

            let is_running = Command::new("pidof")
                .arg("ankrad")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            println!("● Ankra Input Engine is {}", if is_running { "RUNNING" } else { "STOPPED" });
            println!("  Mode:   {}", if mode == "off" { "PASSTHROUGH (Passive raw key propagation)" } else { "ACTIVE (Translating sequences)" });
            println!("  Layout: {}", layout_id);
        }

        "lookup" => {
            if args.len() < 3 {
                eprintln!("Error: Missing query string.");
                eprintln!("Usage: ankra-cli lookup <phrase>");
                exit(1);
            }
            let target = args[2].trim();

            let layout_id = match get_current_layout(&data_home) {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    exit(1);
                }
            };

            let sequences = lookup_sequence(&config_home, &layout_id, target);

            if sequences.is_empty() {
                exit(1);
            } else {
                for seq in sequences {
                    println!("{}", seq);
                }
            }
        }

        "cook" => {
            if args.len() < 4 {
                eprintln!("Error: Missing required cooking arguments.");
                eprintln!("Usage: ankra-cli cook <ingredient_file> <output_name>");
                exit(1);
            }
            let ingredient_file = args[2].trim();
            let output_name = args[3].trim();

            let layout_id = match get_current_layout(&data_home) {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    exit(1);
                }
            };

            println!("Active layout detected: '{}'", layout_id);
            if let Err(e) = cook::cangjie5_phrases::run(&layout_id, ingredient_file, output_name) {
                eprintln!("Compilation failed: {}", e);
                exit(1);
            }
        }
        _ => {
            print_help();
            exit(1);
        }
    }
}

fn print_help() {
    println!("Ankra IME");
    println!("\nUsage:");
    println!("  ankra-cli <COMMAND> [OPTIONS]");
    println!("\nCommands:");
    println!("  status                     Show current daemon state and active layout");
    println!("  lookup <phrase>            Find keystrokes for a specific character/word");
    println!("  layout <id>                Set active layout configuration target");
    println!("  on                         Engage engine interception loops");
    println!("  off                        Put engine into sleep pass-through mode");
    println!("  cook <file> <out_name>     Pre-bake multi-character mappings inside active layout directory");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn test_set_status_writes_on_and_off_correctly() {
        let temp_data_home = temp_dir().join("ankra_test_status_dir");
        fs::create_dir_all(&temp_data_home).unwrap();

        set_status(&temp_data_home, "on").unwrap();
        assert_eq!(fs::read_to_string(temp_data_home.join("status")).unwrap(), "on");

        set_status(&temp_data_home, "off").unwrap();
        assert_eq!(fs::read_to_string(temp_data_home.join("status")).unwrap(), "off");

        fs::remove_dir_all(temp_data_home).unwrap();
    }

    #[test]
    fn test_set_layout_success_with_trimming() {
        let temp_data_home = temp_dir().join("ankra_test_data_dir");
        let temp_config_home = temp_dir().join("ankra_test_config_dir");

        fs::create_dir_all(&temp_data_home).unwrap();
        let mock_layout_path = temp_config_home.join("cangjie5_express");
        fs::create_dir_all(&mock_layout_path).unwrap();

        let result = set_layout(&temp_data_home, &temp_config_home, "   cangjie5_express  ");
        assert_eq!(result, Ok("cangjie5_express".to_string()));

        let saved_content = fs::read_to_string(temp_data_home.join("current_layout")).unwrap();
        assert_eq!(saved_content, "cangjie5_express");

        fs::remove_dir_all(temp_data_home).unwrap();
        fs::remove_dir_all(temp_config_home).unwrap();
    }

    #[test]
    fn test_set_layout_fails_if_directory_missing() {
        let temp_data_home = temp_dir().join("ankra_test_data_dir_fail");
        let temp_config_home = temp_dir().join("ankra_test_config_dir_fail");
        fs::create_dir_all(&temp_data_home).unwrap();
        fs::create_dir_all(&temp_config_home).unwrap();

        let result = set_layout(&temp_data_home, &temp_config_home, "ghost_layout");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Layout directory not found"));
        assert!(!temp_data_home.join("current_layout").exists());

        fs::remove_dir_all(temp_data_home).unwrap();
        fs::remove_dir_all(temp_config_home).unwrap();
    }

    #[test]
    fn test_lookup_sequence_scans_multiple_csvs() {
        let temp_config_home = temp_dir().join("ankra_test_lookup_dir");
        let layout_dir = temp_config_home.join("test_layout");
        fs::create_dir_all(&layout_dir).unwrap();

        // Write a mock table.csv
        let mut table = File::create(layout_dir.join("table.csv")).unwrap();
        writeln!(table, "character,sequence").unwrap();
        writeln!(table, "我,hqi").unwrap();
        writeln!(table, "的,hpi").unwrap();

        // Write a mock phrases.csv
        let mut phrases = File::create(layout_dir.join("phrases.csv")).unwrap();
        writeln!(phrases, "character,sequence").unwrap();
        writeln!(phrases, "我的,hqhpi").unwrap();

        // Test single character lookup
        let char_res = lookup_sequence(&temp_config_home, "test_layout", "我");
        assert_eq!(char_res, vec!["hqi"]);

        // Test phrase lookup
        let phrase_res = lookup_sequence(&temp_config_home, "test_layout", "我的");
        assert_eq!(phrase_res, vec!["hqhpi"]);

        fs::remove_dir_all(temp_config_home).unwrap();
    }
}

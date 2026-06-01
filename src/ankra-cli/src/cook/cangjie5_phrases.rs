// tested with:
// 1. rime-ice.yaml

use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

const BASE_TABLE_NAME: &str = "chars.csv";

fn load_character_map(table_path: &PathBuf) -> Result<HashMap<String, String>, Box<dyn Error>> {
    let mut char_map = HashMap::new();
    let file = File::open(table_path)?;
    let mut rdr = csv::Reader::from_reader(file);

    for result in rdr.records() {
        let record = result?;
        if record.len() >= 2 {
            let character = record[0].trim().to_string();
            let sequence = record[1].trim().to_string();
            if !character.is_empty() && !sequence.is_empty() {
                char_map.entry(character).or_insert(sequence);
            }
        }
    }
    Ok(char_map)
}

/// Automatically scans the comment block or YAML headers to isolate version data
fn extract_version(input_path: &PathBuf) -> String {
    if let Ok(file) = File::open(input_path) {
        let reader = BufReader::new(file);
        for line_result in reader.lines() {
            if let Ok(line) = line_result {
                let lower = line.to_lowercase();

                // Strategy A: Explicit "version:" attribute mapping detection
                if let Some(idx) = lower.find("version:") {
                    let raw_val = &line[idx + 8..].trim();
                    let cleaned: String = raw_val
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '.')
                        .collect();
                    if !cleaned.is_empty() {
                        return cleaned;
                    }
                }

                // Strategy B: Fallback string matching on generic release date structures (e.g., 2026-01-26)
                if let Some(idx) = line.find("20") {
                    if line.len() >= idx + 10 {
                        let potential_date = &line[idx..idx + 10];
                        let parts: Vec<&str> = potential_date.split('-').collect();
                        if parts.len() == 3 && parts[0].chars().all(|c| c.is_ascii_digit()) {
                            return potential_date.to_string();
                        }
                    }
                }
            }
        }
    }
    "unknown".to_string()
}

fn encode_cangjie5(phrase: &str, char_map: &HashMap<String, String>) -> Option<String> {
    let mut seqs = Vec::with_capacity(phrase.chars().count());
    for ch in phrase.chars() {
        let seq = char_map.get(&ch.to_string())?;
        seqs.push(seq.chars().collect::<Vec<char>>());
    }

    let len = phrase.chars().count();
    let mut code = String::with_capacity(5);

    match len {
        2 => {
            let a = &seqs[0];
            let b = &seqs[1];
            code.push(a[0]);
            code.push(a[a.len() - 1]);
            code.push(b[0]);
            if b.len() > 2 { code.push(b[1]); }
            if b.len() > 1 { code.push(b[b.len() - 1]); }
        }
        3 => {
            let a = &seqs[0];
            let b = &seqs[1];
            let c = &seqs[2];
            code.push(a[0]);
            code.push(b[0]);
            if b.len() > 1 { code.push(b[b.len() - 1]); }
            code.push(c[0]);
            if c.len() > 1 { code.push(c[c.len() - 1]); }
        }
        4 => {
            let a = &seqs[0];
            let b = &seqs[1];
            let c = &seqs[2];
            let d = &seqs[3];
            code.push(a[0]);
            code.push(b[0]);
            code.push(c[0]);
            code.push(d[0]);
            if d.len() > 1 { code.push(d[d.len() - 1]); }
        }
        _ => {
            code.push(seqs[0][0]);
            code.push(seqs[1][0]);
            code.push(seqs[2][0]);
            code.push(seqs[3][0]);
            let last_seq = &seqs[seqs.len() - 1];
            code.push(last_seq[last_seq.len() - 1]);
        }
    }

    code.truncate(5);
    Some(code)
}

pub fn run(layout_id: &str, ingredient_filename: &str, output_filename: &str) -> Result<(), Box<dyn Error>> {
    let home = std::env::var("HOME").map_err(|_| "Error: $HOME environment variable is not set.")?;
    let layout_dir = PathBuf::from(home).join(".config").join("ankra").join(layout_id);

    let table_path = layout_dir.join(BASE_TABLE_NAME);
    let input_path = layout_dir.join(ingredient_filename);
    let output_path = layout_dir.join(format!("{}.csv", output_filename));

    if !table_path.exists() {
        return Err(format!("Error: chars.csv configuration missing at: {}", table_path.display()).into());
    }
    if !input_path.exists() {
        return Err(format!("Error: Ingredient file '{}' missing from layout folder: {}", ingredient_filename, layout_dir.display()).into());
    }

    // Perform the automatic version extraction pass
    let detected_version = extract_version(&input_path);
    println!("Automatically detected version: '{}'", detected_version);

    println!("Reading character map from {}...", table_path.display());
    let char_map = load_character_map(&table_path)?;

    println!("Scanning raw dictionary structures from {}...", input_path.display());
    let in_file = File::open(&input_path)?;
    let reader = BufReader::new(in_file);

    println!("Writing compiled pre-baked data to {}...", output_path.display());
    let mut out_file = File::create(&output_path)?;

    // Inject the dynamically generated version line safely as a string comment
    writeln!(out_file, "# from rime-ice version {}", detected_version)?;

    let mut writer = csv::Writer::from_writer(out_file);
    writer.write_record(&["character", "sequence"])?;

    let mut compiled_count = 0;

    for line_result in reader.lines() {
        let line = line_result?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = trimmed.split('\t').collect();
        let phrase = parts[0].trim();
        if phrase.chars().count() < 2 {
            continue;
        }

        if let Some(generated_code) = encode_cangjie5(phrase, &char_map) {
            writer.write_record(&[phrase, &generated_code])?;
            compiled_count += 1;
        }
    }

    writer.flush()?;
    println!("Success! Packed {} phrases into: {}", compiled_count, output_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs::File;
    use std::io::Write;

    /// Helper function to generate a mock character map for algorithmic testing
    fn mock_char_map() -> HashMap<String, String> {
        let mut map = HashMap::new();
        // Using explicit alphabetical codes to perfectly trace the Cangjie5 rule extractions
        map.insert("A".to_string(), "abcde".to_string()); // First/Last: a, e
        map.insert("B".to_string(), "fghij".to_string()); // First/Second/Last: f, g, j
        map.insert("C".to_string(), "klmno".to_string()); // First/Last: k, o
        map.insert("D".to_string(), "pqrst".to_string()); // First/Last: p, t
        map.insert("E".to_string(), "uvwxy".to_string()); // First/Last: u, y
        map
    }

    #[test]
    fn test_cangjie5_2_char_rule() {
        // Formula: Aa Az Ba Bb Bz
        let map = mock_char_map();
        let phrase = "AB";
        // A -> a(1), e(z)
        // B -> f(1), g(2), j(z)
        // Expected: a e f g j
        assert_eq!(encode_cangjie5(phrase, &map), Some("aefgj".to_string()));
    }

    #[test]
    fn test_cangjie5_3_char_rule() {
        // Formula: Aa Ba Bz Ca Cz
        let map = mock_char_map();
        let phrase = "ABC";
        // A -> a(1)
        // B -> f(1), j(z)
        // C -> k(1), o(z)
        // Expected: a f j k o
        assert_eq!(encode_cangjie5(phrase, &map), Some("afjko".to_string()));
    }

    #[test]
    fn test_cangjie5_4_char_rule() {
        // Formula: Aa Ba Ca Da Dz
        let map = mock_char_map();
        let phrase = "ABCD";
        // A -> a(1)
        // B -> f(1)
        // C -> k(1)
        // D -> p(1), t(z)
        // Expected: a f k p t
        assert_eq!(encode_cangjie5(phrase, &map), Some("afkpt".to_string()));
    }

    #[test]
    fn test_cangjie5_5_plus_char_rule() {
        // Formula: Aa Ba Ca Da Ez
        let map = mock_char_map();
        let phrase_5 = "ABCDE";
        // A -> a(1)
        // B -> f(1)
        // C -> k(1)
        // D -> p(1)
        // E -> y(z)
        // Expected: a f k p y
        assert_eq!(encode_cangjie5(phrase_5, &map), Some("afkpy".to_string()));

        // Ensure 6+ length phrases still strictly grab the final character's last radical
        let phrase_6 = "ABCDCB";
        // 6th char is B. Last radical of B is 'j'.
        // Expected: a f k p j
        assert_eq!(encode_cangjie5(phrase_6, &map), Some("afkpj".to_string()));
    }

    #[test]
    fn test_missing_character_graceful_fail() {
        // If a phrase contains a character missing from chars.csv, it must abort cleanly
        let map = mock_char_map();
        let phrase = "AX"; // 'X' is not in our mock map
        assert_eq!(encode_cangjie5(phrase, &map), None);
    }

    #[test]
    fn test_extract_version_explicit_tag() {
        // Test Strategy A: "version: X.X.X"
        let temp_path = temp_dir().join("test_rime_version_a.yaml");
        let mut file = File::create(&temp_path).unwrap();
        writeln!(file, "# Rime dictionary").unwrap();
        writeln!(file, "# version: 2026-06-01").unwrap();

        let version = extract_version(&temp_path);
        assert_eq!(version, "2026-06-01");

        std::fs::remove_file(temp_path).unwrap(); // Cleanup
    }

    #[test]
    fn test_extract_version_date_fallback() {
        // Test Strategy B: Raw ISO date detection inside comments
        let temp_path = temp_dir().join("test_rime_version_b.yaml");
        let mut file = File::create(&temp_path).unwrap();
        writeln!(file, "# Rime dictionary").unwrap();
        writeln!(file, "# Compiled on 2025-11-22 for release").unwrap();

        let version = extract_version(&temp_path);
        assert_eq!(version, "2025-11-22");

        std::fs::remove_file(temp_path).unwrap(); // Cleanup
    }
}

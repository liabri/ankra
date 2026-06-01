use ankra::{ AnkraEngine, AnkraResponse };
use std::path::Path;

#[track_caller]
pub fn test_input_impl(mut engine: AnkraEngine, keys: &[(u16, AnkraResponse)]) {
    for (key, response) in keys.iter() {
        let rep = engine.on_key_press(key.to_owned(), 0); // default to level 0
        eprintln!("Key: {:?}, Rep: {:?}", key, rep);
        assert_eq!(&rep, response);
    }
}

#[track_caller]
pub fn test_input_with_level_impl(mut engine: AnkraEngine, keys: &[(u16, u16, AnkraResponse)]) {
    for (key, level, response) in keys.iter() {
        let rep = engine.on_key_press(key.to_owned(), *level as usize); // pass the actual level!
        eprintln!("Key: {:?}, Level: {:?}, Rep: {:?}", key, level, rep);
        assert_eq!(&rep, response);
    }
}

/// Overwrites layout files with a pristine, statically defined test fixture dataset
pub fn reset_layout_weights(base_dir: &Path, layout_id: &str) {
    let layout_dir = base_dir.join(layout_id);
    std::fs::create_dir_all(&layout_dir).unwrap();

    // Pristine 9-row baseline layout. Notice '抙' is ordered before '𠂖'
    // so it naturally wins the tie-breaker when weights are identical!
    let chars_content = "character,sequence,weight\n\
                         日,a,0\n\
                         手,q,0\n\
                         抙,qq,0\n\
                         𠂖,qq,0\n\
                         曰,a,0\n\
                         中,l,0\n\
                         𬡂,lo,0\n\
                         掱,qqq,0\n\
                         𮕶,lol,0\n";

    let phrases_content = "character,sequence,weight\n\
                           我的,hqhpi,0\n";

    std::fs::write(layout_dir.join("chars.csv"), chars_content).unwrap();
    std::fs::write(layout_dir.join("phrases.csv"), phrases_content).unwrap();
}

#[allow(unused_macros)]
macro_rules! define_layout_test {
    ($layout:expr) => {
        use shared::{ test_input_impl, test_input_with_level_impl, reset_layout_weights };
        use ankra::{ AnkraEngine, AnkraConfig };
        use std::path::PathBuf;

        #[allow(dead_code)]
        #[track_caller]
        fn test_input(keys: &[(u16, AnkraResponse)]) {
            let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
            reset_layout_weights(&base_dir, $layout);

            let context = AnkraEngine::new(AnkraConfig {
                id: $layout.to_string(),
                dir: base_dir
            });
            test_input_impl(context, keys);
        }

        fn test_input_with_level(keys: &[(u16, u16, AnkraResponse)]) {
            let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
            reset_layout_weights(&base_dir, $layout);

            let context = AnkraEngine::new(AnkraConfig {
                id: $layout.to_string(),
                dir: base_dir
            });
            test_input_with_level_impl(context, keys);
        }
    };

    ($layout:expr) => {
        define_layout_test!($layout);
    };
}

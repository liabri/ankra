use ankra::{ AnkraEngine, AnkraResponse };

#[track_caller]
pub fn test_input_impl(mut engine: AnkraEngine, keys: &[(u16, AnkraResponse)]) {
    for (key, response) in keys.iter() {
        let rep = engine.on_key_press(key.to_owned());
        eprintln!("Key: {:?}, Rep: {:?}", key, rep);
        assert_eq!(&rep, response);
    }
}


#[track_caller]
pub fn test_input_with_level_impl(mut engine: AnkraEngine, keys: &[(u16, u16, AnkraResponse)]) {
    for (key, level, response) in keys.iter() {
        let rep = engine.on_key_press(key.to_owned());
        eprintln!("Key: {:?}, Level: {:?}, Rep: {:?}", key, level, rep);
        assert_eq!(&rep, response);
    }
}

#[allow(unused_macros)]
macro_rules! define_layout_test {
    ($layout:expr) => {
        use shared::{ test_input_impl, test_input_with_level_impl };
        use ankra::{ AnkraEngine, AnkraConfig };

        #[allow(dead_code)]
        #[track_caller]
        fn test_input(keys: &[(u16, AnkraResponse)]) {
            let context = AnkraEngine::new(AnkraConfig { 
                id: $layout.to_string(),
                ..AnkraConfig::default()
            });
            test_input_impl(context, keys);
        }

        fn test_input_with_level(keys: &[(u16, u16, AnkraResponse)]) {
            let context = AnkraEngine::new(AnkraConfig { 
                id: $layout.to_string(),
                ..AnkraConfig::default()
            });
            test_input_with_level_impl(context, keys);
        }
    };

    ($layout:expr) => {
        define_layout_test!($layout);
    };
}
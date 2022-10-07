#[macro_use]
mod shared;

use ankra::AnkraResponse;

define_layout_test!("cangjie5");

#[test]
fn cangjie_commit_key() {
    test_input(&[
        (38, AnkraResponse::Suggest(String::from("日"))),
        (65, AnkraResponse::Commit(String::from("日"))),
    ])
}

#[test]
fn cangjie_next_key() {
    test_input_with_level(&[
        (38, 0, AnkraResponse::Suggest(String::from("日"))),
        (23, 0, AnkraResponse::Suggest(String::from("曰"))), // next
        (23, 1, AnkraResponse::Suggest(String::from("日"))), // prev
        (65, 0, AnkraResponse::Commit(String::from("曰")))
    ])
}

#[test]
fn cangjie_backspace_key() {
    test_input(&[
        (24, AnkraResponse::Suggest(String::from("手"))),
        (24, AnkraResponse::Suggest(String::from("抙"))),
        (22, AnkraResponse::Suggest(String::from("手"))),
        (65, AnkraResponse::Commit(String::from("手"))),

        (24, AnkraResponse::Suggest(String::from("手"))),
        (24, AnkraResponse::Suggest(String::from("抙"))),
        (22, AnkraResponse::Suggest(String::from("手"))),
    ])
}

#[test]
fn cangjie_on_no_result() {
    test_input(&[
        (24, AnkraResponse::Suggest(String::from("手"))),
        (24, AnkraResponse::Suggest(String::from("抙"))),
        (24, AnkraResponse::Suggest(String::from("掱"))),

        (24, AnkraResponse::Empty),

        //on fail restart sequence
        (24, AnkraResponse::Suggest(String::from("手"))),
        (24, AnkraResponse::Suggest(String::from("抙"))),
        (24, AnkraResponse::Suggest(String::from("掱"))),
        (65, AnkraResponse::Commit(String::from("掱"))),
    ])
}
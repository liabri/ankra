/* =========================================================================
XKB KEYCODE REFERENCE SHEET (QWERTY layout, evdev + 8 offset)
Use these values when drafting virtual keycodes in integration tests.
=========================================================================

--- FUNCTION & ESCAPE ROW ---
9   = Escape
67  = F1        68  = F2        69  = F3        70  = F4
71  = F5        72  = F6        73  = F7        74  = F8
75  = F9        76  = F10       95  = F11       96  = F12

--- NUMBER ROW ---
49  = ` (Grave / Tilde)
10  = 1         11  = 2         12  = 3         13  = 4
14  = 5         15  = 6         16  = 7         17  = 8
18  = 9         19  = 0
20  = - (Minus) 21  = = (Equal)
22  = Backspace

--- TOP ALPHABET ROW ---
23  = Tab
24  = Q         25  = W         26  = E         27  = R
28  = T         29  = Y         30  = U         31  = I
32  = O         33  = P
34  = [         35  = ]         51  = \ (Backslash)

--- HOME ALPHABET ROW ---
66  = CapsLock
38  = A         39  = S         40  = D         41  = F
42  = G         43  = H         44  = J         45  = K
46  = L
47  = ; (Semi)  48  = ' (Quote)
36  = Enter

--- BOTTOM ALPHABET ROW ---
50  = Left Shift
52  = Z         53  = X         54  = C         55  = V
56  = B         57  = N         58  = M
59  = , (Comma) 60  = . (Dot)   61  = / (Slash)
62  = Right Shift

--- SYSTEM / CONTROL ROW ---
37  = Left Control
133 = Left Super (Windows/Meta key)
64  = Left Alt
65  = Spacebar
108 = Right Alt (AltGr)
134 = Right Super
105 = Right Control

--- EDITING & NAVIGATION BLOCK ---
118 = Insert    110 = Home      112 = Page Up
119 = Delete    115 = End       117 = Page Down

--- ARROW KEYS ---
                111 = Up Arrow
113 = Left Arrow                114 = Right Arrow
                116 = Down Arrow
========================================================================= */

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
        (65, 0, AnkraResponse::Commit(String::from("日")))
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
fn cangjie_escape_key() {
    test_input(&[
        (24, AnkraResponse::Suggest(String::from("手"))),
        (24, AnkraResponse::Suggest(String::from("抙"))),
        // 9 = Escape key. should clear the whole layout state instantly
        (9, AnkraResponse::Empty),
        // prove the buffer was wiped by starting a completely fresh sequence
        (38, AnkraResponse::Suggest(String::from("日"))),
    ])
}

#[test]
fn cangjie_direct_digit_selection() {
    test_input_with_level(&[
        (24, 0, AnkraResponse::Suggest(String::from("手"))),
        (24, 0, AnkraResponse::Suggest(String::from("抙"))),
        // Tapping '2' shifts the selection to Candidate index 1 ("𠂖")
        (11, 0, AnkraResponse::Suggest(String::from("𠂖"))),
        (65, 0, AnkraResponse::Commit(String::from("𠂖"))),
    ])
}

#[test]
fn cangjie_boundary_guards() {
    test_input_with_level(&[
        // 1. Test Underflow Guard on "q" ("手")
        (24, 0, AnkraResponse::Suggest(String::from("手"))),
        // Hammer 'Prev' (Level 1) at index 0. It should safely catch and freeze on "手"
        (23, 1, AnkraResponse::Suggest(String::from("手"))),
        (23, 1, AnkraResponse::Suggest(String::from("手"))),

        // 2. Build the sequence up to "qqq" ("掱")
        (24, 0, AnkraResponse::Suggest(String::from("抙"))), // "qq"
        (24, 0, AnkraResponse::Suggest(String::from("掱"))), // "qqq"

        // 3. Test Overflow Guard
        // Since "掱" is the end of this sequence's list, hammering 'Next'
        // should safely freeze right here instead of breaking or changing pages.
        (23, 0, AnkraResponse::Suggest(String::from("掱"))),
        (23, 0, AnkraResponse::Suggest(String::from("掱"))),
        (65, 0, AnkraResponse::Commit(String::from("掱"))),
    ])
}

#[test]
fn cangjie_alternative_arrow_navigation() {
    test_input_with_level(&[
        (38, 0, AnkraResponse::Suggest(String::from("日"))),
        // 114 = Right Arrow (NEXT at Level 0)
        (114, 0, AnkraResponse::Suggest(String::from("曰"))),
        // 113 = Left Arrow (PREV at Level 0)
        (113, 0, AnkraResponse::Suggest(String::from("日"))),
        (65, 0, AnkraResponse::Commit(String::from("日"))),
    ])
}

#[test]
fn cangjie_empty_buffer_resilience() {
    test_input(&[
        // Tapping Backspace (22) or Escape (9) on a blank slate
        // should safely yield Undefined without crashing.
        (22, AnkraResponse::Undefined),
        (9, AnkraResponse::Undefined),
        // Ensure engine still works perfectly immediately after
        (38, AnkraResponse::Suggest(String::from("日"))),
    ])
}

#[test]
fn cangjie_unmapped_key_interception() {
    test_input(&[
        (24, AnkraResponse::Suggest(String::from("手"))),
        // 67 = F1 key (completely unmapped in config.zm)
        // The engine should ignore it and preserve the current preedit suggestion.
        (67, AnkraResponse::Suggest(String::from("手"))),
        (65, AnkraResponse::Commit(String::from("手"))),
    ])
}

#[test]
fn cangjie_on_no_result() {
    test_input(&[
        (24, AnkraResponse::Suggest(String::from("手"))), // q
        (24, AnkraResponse::Suggest(String::from("抙"))), // qq
        (24, AnkraResponse::Suggest(String::from("掱"))), // qqq

        // It successfully preserves the typing buffer and suggests the raw string.
        (24, AnkraResponse::Suggest(String::from("qqqq"))),

        // Striking the Spacebar (65) commits that raw text fallback cleanly
        (65, AnkraResponse::Commit(String::from("qqqq"))),
    ])
}

#[test]
fn cangjie_raw_enter_escape_hatch() {
    test_input(&[
        (24, AnkraResponse::Suggest(String::from("手"))), // q
        // even though a Chinese match exists ("手"), hitting Enter (36)
        // must explicitly bypass it and print the raw string "q" instead.
        (36, AnkraResponse::Commit(String::from("q"))),

        // ensure the engine completely reset and is ready for fresh input
        (46, AnkraResponse::Suggest(String::from("中"))), // l
        (32, AnkraResponse::Suggest(String::from("𬡂"))), // o
        (46, AnkraResponse::Suggest(String::from("𮕶"))), // l
        (36, AnkraResponse::Commit(String::from("lol"))),
    ])
}

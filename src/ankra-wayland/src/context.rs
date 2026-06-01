//! Stateful bridge between `libankra` (engine) and Wayland protocols.
//!
//! ## Core Protocol & Implementation Rules
//!
//! ### 1. No Virtual Loopbacks
//! Keys unconsumed by the engine are injected down `zwp_virtual_keyboard_v1`.
//! The compositor explicitly bypasses our input grab for these synthetic events,
//! meaning they never loop back into this handler. No echo filtering is required.
//!
//! ### 2. Commit Serialization (`im_done_serial`)
//! The `im.commit(serial)` parameter must track the total count of compositor `Done`
//! events received, *not* the hardware keyboard event serial. Passing a hardware
//! serial violates the protocol spec, causing strict clients to drop text and freeze.
//!
//! ### 3. Key Lifecycles & Startup Race Hatch
//! * **Auto-Repeat:** Handled natively by the OS via the virtual keyboard. No internal
//!   software timers are used or needed.
//! * **Startup Hatch:** Launching the daemon via a keystroke introduces a race: the key
//!   *press* happens before the grab initializes, but the *release* happens after. We
//!   use `forwarded_presses` and a fallback control-key check (`Enter`, `Space`, etc.)
//!   to force a clean key-up and prevent stuck-modifier loop storms.

use ankra::{AnkraConfig, AnkraEngine, AnkraResponse};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::os::fd::AsFd;

use wayland_client::protocol::wl_keyboard::KeyState;
use wayland_client::WEnum;
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_keyboard_grab_v2::Event as KeyEvent,
    zwp_input_method_v2::{Event as ImEvent, ZwpInputMethodV2},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

pub struct AnkraContext {
    pub engine: AnkraEngine,
    im_active: bool,
    vk: ZwpVirtualKeyboardV1,
    im: ZwpInputMethodV2,
    keymap_init: bool,
    mod_state: bool,
    im_done_serial: u32,
    is_global_active: Arc<AtomicBool>,
    forwarded_presses: [bool; 512],
}

impl AnkraContext {
    pub fn new(layout: &str, vk: ZwpVirtualKeyboardV1, im: ZwpInputMethodV2, is_global_active: Arc<AtomicBool>) -> Self {
        Self {
            engine: AnkraEngine::new(AnkraConfig { id: layout.to_string(), ..Default::default() }),
            im_active: false,
            keymap_init: false,
            mod_state: true,
            vk, im, im_done_serial: 0, is_global_active,
            forwarded_presses: [false; 512],
        }
    }

    pub fn handle_im_ev(&mut self, ev: ImEvent) {
        match ev {
            ImEvent::Activate => self.im_active = true,
            ImEvent::Deactivate => self.im_active = false,
            ImEvent::Unavailable => panic!("Input method unavailable"),
            ImEvent::Done => {
                self.im_done_serial += 1;
                if !self.im_active {
                    self.engine.reset();
                    self.forwarded_presses = [false; 512];
                }
            }
            _ => {}
        }
    }

    pub fn handle_key_ev(&mut self, ev: KeyEvent) {
        match ev {
            KeyEvent::Keymap { format, fd, size } => {
                if !self.keymap_init {
                    let format_val = match format { WEnum::Value(f) => f as u32, _ => 1 };
                    self.vk.keymap(format_val, fd.as_fd(), size);
                    self.keymap_init = true;
                }
            }

            KeyEvent::Key { serial: _, time, key, state } => {
                let key_idx = key as usize;
                if key_idx >= 512 { return; }

                let is_pressed = matches!(state, WEnum::Value(KeyState::Pressed));

                if self.im_active && self.mod_state && self.is_global_active.load(Ordering::Relaxed) {
                    if is_pressed {
                        match self.engine.on_key_press((key + 8) as u16, 0) {
                            AnkraResponse::Suggest(s) => self.im.set_preedit_string(s.clone(), 0, s.len() as i32),
                            response => {
                                self.im.set_preedit_string(String::new(), -1, -1);
                                match response {
                                    AnkraResponse::Undefined => {
                                        self.forwarded_presses[key_idx] = true;
                                        self.vk.key(time, key, 1);
                                    }
                                    AnkraResponse::Commit(s) => self.im.commit_string(s),
                                    _ => {}
                                }
                            }
                        }
                        self.im.commit(self.im_done_serial);
                    } else {
                        let _ = self.engine.on_key_release((key + 8) as u16, 0);
                        if self.forwarded_presses[key_idx] || matches!(key, 28 | 57 | 14 | 1) {
                            self.vk.key(time, key, 0);
                            self.forwarded_presses[key_idx] = false;
                        }
                    }
                } else {
                    self.vk.key(time, key, if is_pressed { 1 } else { 0 });
                    self.forwarded_presses[key_idx] = is_pressed;
                }
            }

            KeyEvent::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                self.mod_state = mods_depressed == 0 && mods_latched == 0 && mods_locked == 0;
                self.vk.modifiers(mods_depressed, mods_latched, mods_locked, group);
            }
            _ => {}
        }
    }
}

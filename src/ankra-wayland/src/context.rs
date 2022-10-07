use ankra::{ AnkraEngine, AnkraResponse, AnkraConfig };
use std::time::{ Duration, Instant };
use mio_timerfd::TimerFd;

use wayland_client::{ DispatchData, Main };
use wayland_client::protocol::wl_keyboard::KeyState;
use wayland_protocols::misc::zwp_input_method_v2::client::{
    zwp_input_method_keyboard_grab_v2::Event as KeyEvent,
    zwp_input_method_v2::{ Event as ImEvent, ZwpInputMethodV2 },
};

use zwp_virtual_keyboard::virtual_keyboard_unstable_v1::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

pub struct AnkraContext {
    pub engine: AnkraEngine,
    current_state: InputMethodState,
    vk: Main<ZwpVirtualKeyboardV1>,
    im: Main<ZwpInputMethodV2>,
    keymap_init: bool,
    mod_state: bool,
    serial: u32,
    timer: TimerFd,
    repeat_state: Option<(RepeatInfo, PressState)>,
}

// pub enum ModifierState {
//     CONTROL = 0x4,
//     SUPER = 0x40,
//     // LALT = 0x8,
// }

#[derive(PartialEq)]
pub enum InputMethodState {
    Active,
    Inactive,
}

#[derive(Clone, Copy)]
struct RepeatInfo {
    rate: i32,
    delay: i32,
}

#[derive(Clone, Copy)]
enum PressState {
    NotPressing,
    Pressing {
        pressed_at: Instant,
        is_repeating: bool,
        key: u32,
        wayland_time: u32,
    },
}

impl PressState {
    fn is_pressing(&self, query_key: u32) -> bool {
        if let PressState::Pressing { key, .. } = self {
            *key == query_key
        } else {
            false
        }
    }
}

impl AnkraContext {
    pub fn new(layout: &str, vk: Main<ZwpVirtualKeyboardV1>, im: Main<ZwpInputMethodV2>, timer: TimerFd) -> Self { 
        Self {
            engine: AnkraEngine::new(AnkraConfig { 
                id: String::from(layout),
                ..AnkraConfig::default() 
            }),

            current_state: InputMethodState::Inactive,
            serial: 0,
            keymap_init: false,
            mod_state: true,
            vk,
            im,
            timer,
            repeat_state: None,
        }
    }

    pub fn new_data<'a>(data: &'a mut DispatchData) -> &'a mut Self {
        data.get::<Self>().unwrap()
    }

    pub fn handle_im_ev(&mut self, ev: ImEvent) {
        match ev {
            ImEvent::Activate => {
                self.current_state = InputMethodState::Active
            },

            ImEvent::Deactivate => {
                self.current_state = InputMethodState::Inactive;
            },

            ImEvent::Unavailable => {
                log::error!("input method unavailable, is another server already running ?");
                panic!("unavailable")
            },

            ImEvent::Done => {
                if self.current_state==InputMethodState::Inactive {
                    // Focus lost, reset states
                    self.engine.reset();

                    // Input deactivated, stop repeating
                    self.timer.disarm().unwrap();
                    if let Some((_, ref mut press_state)) = self.repeat_state {
                         *press_state = PressState::NotPressing
                    }
                }
            },

            _ => {}
        }
    }

    pub fn handle_key_ev(&mut self, ev: KeyEvent) {
        match ev {
            KeyEvent::Keymap { fd, format, size } => {
                if !self.keymap_init {
                    self.vk.keymap(format as _, fd, size);
                    self.keymap_init = true;
                }

                unsafe { libc::close(fd); }
            },

            KeyEvent::Key { state, key, time, .. } => {
                if self.current_state==InputMethodState::Active && self.mod_state {
                    match state {
                        KeyState::Pressed => {
                            match self.engine.on_key_press((key + 8) as u16) {
                                AnkraResponse::Empty => {
                                    self.im.set_preedit_string(String::new(), -1, -1);
                                },

                                AnkraResponse::Undefined => {
                                    self.vk.key(time, key, state as _);
                                    self.im.set_preedit_string(String::new(), -1, -1);
                                    return
                                },

                                AnkraResponse::Commit(s) => { 
                                    self.im.commit_string(s);
                                    self.im.set_preedit_string(String::new(), -1, -1);
                                },

                                AnkraResponse::Suggest(s) => {
                                    let len = s.len();
                                    self.im.set_preedit_string(s, 0, len as _);
                                }
                            }

                            self.im.commit(self.serial);
                            self.serial += 1;

                            match self.repeat_state {
                                Some((info, ref mut press_state)) if !press_state.is_pressing(key) => {
                                    let duration = Duration::from_millis(info.delay as u64);
                                    self.timer.set_timeout(&duration).unwrap();
                                    *press_state = PressState::Pressing {
                                        pressed_at: Instant::now(),
                                        is_repeating: false,
                                        key,
                                        wayland_time: time,
                                    };
                                },

                                _ => {}
                            }
                        },

                        KeyState::Released => {
                            // If user released the last pressed key, clear the timer and state
                            if let Some((.., ref mut press_state)) = self.repeat_state {
                                if press_state.is_pressing(key) {
                                    self.timer.disarm().unwrap();
                                    *press_state = PressState::NotPressing;
                                }
                            }

                            self.vk.key(time, key, state as _);
                        },

                        _ => {}
                    }
                } else {
                    self.vk.key(time, key, state as _);
                }
            },

            KeyEvent::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                self.mod_state = true;

                if mods_depressed!=0 || mods_latched !=0 || mods_locked !=0 {
                    self.mod_state = false;
                }

                self.vk.modifiers(mods_depressed, mods_latched, mods_locked, group);
            },

            KeyEvent::RepeatInfo { rate, delay } => {
                // Zero rate means disabled repeat
                self.repeat_state = if rate == 0 {
                    None
                } else {
                    let info = RepeatInfo { rate, delay };
                    let press_state = self.repeat_state.map(|pair| pair.1);
                    Some((info, press_state.unwrap_or(PressState::NotPressing)))
                }
            },

            _ => {}
        }
    }

    pub fn handle_timer_ev(&mut self) -> std::io::Result<()> {
        // Read timer, this MUST be called or timer will be broken
        let overrun_count = self.timer.read()?;
        if overrun_count != 1 {
            log::warn!("Some timer events were not properly handled!");
        }

        if let Some((
            info,
            PressState::Pressing {
                pressed_at,
                ref mut is_repeating,
                key,
                wayland_time,
            },
        )) = self.repeat_state {
            if !*is_repeating {
                // Start repeat
                log::trace!("Start repeating {}", key);
                let interval = &Duration::from_secs_f64(1.0 / info.rate as f64);
                self.timer.set_timeout_interval(interval)?;
                *is_repeating = true;
            }

            // Emit key repeat event
            let ev = KeyEvent::Key {
                serial: self.serial,
                time: wayland_time + pressed_at.elapsed().as_millis() as u32,
                key,
                state: KeyState::Pressed,
            };

            self.serial += 1;
            self.handle_key_ev(ev);
        } else {
            log::warn!("Received timer event when it has never received RepeatInfo.");
        }

        Ok(())
    }
}
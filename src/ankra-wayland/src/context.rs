use ankra::{AnkraConfig, AnkraEngine, AnkraResponse};
use std::time::{Duration, Instant};

use rustix::io::read;
use rustix::time::{timerfd_settime, Itimerspec, TimerfdTimerFlags, Timespec};
use std::os::unix::io::{AsFd, OwnedFd};

use wayland_client::protocol::wl_keyboard::KeyState;
use wayland_client::WEnum;
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_keyboard_grab_v2::Event as KeyEvent,
    zwp_input_method_v2::{Event as ImEvent, ZwpInputMethodV2},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

pub struct AnkraContext {
    pub engine: AnkraEngine,
    current_state: InputMethodState,
    vk: ZwpVirtualKeyboardV1,
    im: ZwpInputMethodV2,
    keymap_init: bool,
    mod_state: bool,
    serial: u32,
    timer: OwnedFd,
    repeat_state: Option<(RepeatInfo, PressState)>,
}

#[derive(PartialEq, Eq)]
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
    pub fn new(layout: &str, vk: ZwpVirtualKeyboardV1, im: ZwpInputMethodV2, timer: OwnedFd) -> Self {
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

    fn disarm_timer(&self) {
        let it = Itimerspec {
            it_value: Timespec { tv_sec: 0, tv_nsec: 0 },
            it_interval: Timespec { tv_sec: 0, tv_nsec: 0 },
        };
        let _ = timerfd_settime(&self.timer, TimerfdTimerFlags::empty(), &it);
    }

    fn arm_timer(&self, duration: Duration, interval: Duration) {
        let it = Itimerspec {
            it_value: Timespec {
                tv_sec: duration.as_secs() as i64,
                tv_nsec: duration.subsec_nanos() as i64,
            },
            it_interval: Timespec {
                tv_sec: interval.as_secs() as i64,
                tv_nsec: interval.subsec_nanos() as i64,
            },
        };
        let _ = timerfd_settime(&self.timer, TimerfdTimerFlags::empty(), &it);
    }

    pub fn handle_im_ev(&mut self, ev: ImEvent) {
        match ev {
            ImEvent::Activate => self.current_state = InputMethodState::Active,
            ImEvent::Deactivate => self.current_state = InputMethodState::Inactive,
            ImEvent::Unavailable => {
                log::error!("input method unavailable, is another server already running ?");
                panic!("unavailable")
            }
            ImEvent::Done => {
                if self.current_state == InputMethodState::Inactive {
                    self.engine.reset();
                    self.disarm_timer();
                    // Drop the mutable lock, just overwrite the state
                    if let Some((info, _)) = self.repeat_state {
                        self.repeat_state = Some((info, PressState::NotPressing));
                    }
                }
            }
            _ => {}
        }
    }

    pub fn handle_key_ev(&mut self, ev: KeyEvent) {
        match ev {
            KeyEvent::Keymap { format, fd, size } => {
                if !self.keymap_init {
                    let format_val = match format {
                        WEnum::Value(f) => f as u32,
                        _ => 1,
                    };
                    self.vk.keymap(format_val, fd.as_fd(), size);
                    self.keymap_init = true;
                }
            }

            KeyEvent::Key { serial: _, time, key, state } => {
                let is_pressed = matches!(state, WEnum::Value(KeyState::Pressed));

                if self.current_state == InputMethodState::Active && self.mod_state {
                    if is_pressed {
                        match self.engine.on_key_press((key + 8) as u16) {
                            AnkraResponse::Empty => self.im.set_preedit_string(String::new(), -1, -1),
                            AnkraResponse::Undefined => {
                                self.vk.key(time, key, 1);
                                self.im.set_preedit_string(String::new(), -1, -1);
                                return;
                            }
                            AnkraResponse::Commit(s) => {
                                self.im.commit_string(s);
                                self.im.set_preedit_string(String::new(), -1, -1);
                            }
                            AnkraResponse::Suggest(s) => {
                                let len = s.len();
                                self.im.set_preedit_string(s, 0, len as i32);
                            }
                        }

                        self.im.commit(self.serial);
                        self.serial += 1;

                        // No ref mut! We read it, then write a fresh state back.
                        if let Some((info, current_press)) = self.repeat_state {
                            if !current_press.is_pressing(key) {
                                self.arm_timer(Duration::from_millis(info.delay as u64), Duration::ZERO);
                                self.repeat_state = Some((info, PressState::Pressing {
                                    pressed_at: Instant::now(),
                                    is_repeating: false,
                                    key,
                                    wayland_time: time,
                                }));
                            }
                        }
                    } else if matches!(state, WEnum::Value(KeyState::Released)) {
                        if let Some((info, current_press)) = self.repeat_state {
                            if current_press.is_pressing(key) {
                                self.disarm_timer();
                                self.repeat_state = Some((info, PressState::NotPressing));
                            }
                        }
                        self.vk.key(time, key, 0);
                    }
                } else {
                    let state_val = if is_pressed { 1 } else { 0 };
                    self.vk.key(time, key, state_val);
                }
            }

            KeyEvent::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                self.mod_state = mods_depressed == 0 && mods_latched == 0 && mods_locked == 0;
                self.vk.modifiers(mods_depressed, mods_latched, mods_locked, group);
            }

            KeyEvent::RepeatInfo { rate, delay } => {
                self.repeat_state = if rate == 0 {
                    None
                } else {
                    let info = RepeatInfo { rate, delay };
                    let press_state = self.repeat_state.map(|pair| pair.1);
                    Some((info, press_state.unwrap_or(PressState::NotPressing)))
                }
            }
            _ => {}
        }
    }

    pub fn handle_timer_ev(&mut self) -> std::io::Result<()> {
        let mut buf = [0u8; 8];
        if let Ok(_) = read(&self.timer, &mut buf) {
            let overruns = u64::from_ne_bytes(buf);
            if overruns != 1 {
                log::warn!("Some timer events were not properly handled!");
            }
        }

        if let Some((info, current_press)) = self.repeat_state {
            if let PressState::Pressing { pressed_at, is_repeating, key, wayland_time } = current_press {
                let mut new_repeating = is_repeating;

                if !new_repeating {
                    let interval = Duration::from_secs_f64(1.0 / info.rate as f64);
                    self.arm_timer(interval, interval);
                    new_repeating = true;
                }

                self.repeat_state = Some((info, PressState::Pressing {
                    pressed_at,
                    is_repeating: new_repeating,
                    key,
                    wayland_time,
                }));

                let ev = KeyEvent::Key {
                    serial: self.serial,
                    time: wayland_time + pressed_at.elapsed().as_millis() as u32,
                    key,
                    state: WEnum::Value(KeyState::Pressed),
                };

                self.serial += 1;
                self.handle_key_ev(ev);
            }
        } else {
            log::warn!("Received timer event when it has never received RepeatInfo.");
        }

        Ok(())
    }
}

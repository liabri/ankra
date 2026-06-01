// src/ankra-wayland/src/lib.rs

mod context;
use context::AnkraContext;

use mio::{unix::SourceFd, Events as MioEvents, Interest, Poll, Token};
use std::os::unix::io::{AsFd, AsRawFd};

use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_keyboard_grab_v2::{self, ZwpInputMethodKeyboardGrabV2},
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
    zwp_input_method_v2::{self, ZwpInputMethodV2},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;

pub struct AppState {
    context: AnkraContext,
}

impl Dispatch<WlRegistry, GlobalListContents> for AppState {
    fn event(_: &mut Self, _: &WlRegistry, _: wayland_client::protocol::wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

delegate_noop!(AppState: ignore WlSeat);
delegate_noop!(AppState: ignore ZwpInputMethodManagerV2);
delegate_noop!(AppState: ignore ZwpVirtualKeyboardManagerV1);
delegate_noop!(AppState: ignore ZwpVirtualKeyboardV1);

impl Dispatch<ZwpInputMethodV2, ()> for AppState {
    fn event(state: &mut Self, _: &ZwpInputMethodV2, event: zwp_input_method_v2::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        state.context.handle_im_ev(event);
    }
}

impl Dispatch<ZwpInputMethodKeyboardGrabV2, ()> for AppState {
    fn event(state: &mut Self, _: &ZwpInputMethodKeyboardGrabV2, event: zwp_input_method_keyboard_grab_v2::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        state.context.handle_key_ev(event);
    }
}

pub struct State {
    conn: Connection,
    event_queue: wayland_client::EventQueue<AppState>,
    poll: Poll,
    app_state: AppState,
}

const POLL_WAYLAND: Token = Token(0);

impl State {
    pub fn new(id: &str, is_active: Arc<AtomicBool>) -> Self {
        let conn = Connection::connect_to_env().expect("Failed to connect to wayland display");
        let (globals, mut event_queue) = registry_queue_init::<AppState>(&conn).unwrap();
        let qh = event_queue.handle();

        let seat: WlSeat = globals.bind(&qh, 1..=8, ()).expect("Failed to load Seat");
        let im_manager: ZwpInputMethodManagerV2 = globals.bind(&qh, 1..=1, ()).expect("Failed to load InputManager");
        let vk_manager: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ()).expect("Failed to load VirtualKeyboardManager");

        let vk = vk_manager.create_virtual_keyboard(&seat, &qh, ());
        let im = im_manager.get_input_method(&seat, &qh, ());
        let _grab = im.grab_keyboard(&qh, ());

        let poll = Poll::new().expect("Initialize epoll()");
        let registry = poll.registry();

        registry.register(&mut SourceFd(&conn.as_fd().as_raw_fd()), POLL_WAYLAND, Interest::READABLE).unwrap();

        let context = AnkraContext::new(id, vk, im, is_active);
        let mut app_state = AppState { context };

        event_queue.roundtrip(&mut app_state).unwrap();
        log::info!("Server successfully initialised!");

        Self {
            conn,
            event_queue,
            poll,
            app_state,
        }
    }

    pub fn run(&mut self) {
        loop {
            self.conn.flush().unwrap();

            let mut events = MioEvents::with_capacity(1024);
            if let Err(e) = self.poll.poll(&mut events, None) {
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                break;
            }

            for event in &events {
                match event.token() {
                    POLL_WAYLAND => {
                        if let Some(guard) = self.conn.prepare_read() {
                            if let Err(e) = guard.read() {
                                if let wayland_client::backend::WaylandError::Io(io_err) = e {
                                    if io_err.kind() != std::io::ErrorKind::WouldBlock {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }
                        }
                        self.event_queue.dispatch_pending(&mut self.app_state).unwrap();
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

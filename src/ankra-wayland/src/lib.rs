mod context;
use context::AnkraContext;

use mio::{ unix::SourceFd, Events as MioEvents, Interest, Poll, Token };
use mio_timerfd::{ ClockId, TimerFd };

use wayland_client::{ event_enum, Display, Filter, GlobalManager, EventQueue };
use wayland_client::protocol::wl_seat::WlSeat;

use wayland_protocols::misc::zwp_input_method_v2::client::{
    zwp_input_method_v2::ZwpInputMethodV2,
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
    zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2,
};

use zwp_virtual_keyboard::virtual_keyboard_unstable_v1::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1
};

event_enum! {
    Events |
    Key => ZwpInputMethodKeyboardGrabV2,
    Im => ZwpInputMethodV2
}

pub struct State {
    context: AnkraContext,
    display: Display,
    event_queue: EventQueue,
    poll: Poll
}

const POLL_WAYLAND: Token = Token(0);
const POLL_TIMER: Token = Token(1);

impl State {
    pub fn new(id: &str) -> Self {
        let display = Display::connect_to_env().map_err(|e| log::error!("Failed to connect to wayland display: {}", e)).unwrap();
        let mut event_queue = display.create_event_queue();
        let attached_display = display.attach(event_queue.token());
        let globals = GlobalManager::new(&attached_display);

        event_queue.sync_roundtrip(&mut (), |_, _, _| ()).unwrap();

        let seat = globals.instantiate_exact::<WlSeat>(1).expect("Failed to load Seat");
        let im_manager = globals.instantiate_exact::<ZwpInputMethodManagerV2>(1).expect("Failed to load InputManager");
        let vk_manager = globals.instantiate_exact::<ZwpVirtualKeyboardManagerV1>(1).expect("Failed to load VirtualKeyboardManager");

        let filter = Filter::new(|ev, _filter, mut data| {
            let context = AnkraContext::new_data(&mut data);
            match ev {
                Events::Key { event, .. } => {
                    context.handle_key_ev(event);
                },

                Events::Im { event, .. } => {
                    context.handle_im_ev(event);
                }
            }
        });

        let vk = vk_manager.create_virtual_keyboard(&seat);
        let im = im_manager.get_input_method(&seat);
        let grab = im.grab_keyboard();
        grab.assign(filter.clone());
        im.assign(filter);

        let mut timer = TimerFd::new(ClockId::Monotonic).expect("Initialize timer");
        let poll = Poll::new().expect("Initialize epoll()");
        let registry = poll.registry();

        registry.register(
            &mut SourceFd(&display.get_connection_fd()),
            POLL_WAYLAND,
            Interest::READABLE | Interest::WRITABLE,
        ).expect("Register wayland socket to the epoll()");

        // Required for hold event of engine values
        registry.register(&mut timer, POLL_TIMER, Interest::READABLE)
            .expect("Register timer to the epoll()");

        // Initialise context
        let mut context = AnkraContext::new(id, vk, im, timer);
        event_queue.sync_roundtrip(&mut context, |_, _, _| ()).unwrap();
        log::info!("Server successfully initialised !");

        Self {
            display,
            event_queue,
            context,
            poll
        }
    }

    pub fn run(&mut self) {
        let stop_reason: Result<_, std::io::Error> = 'main: loop {
            use std::io::ErrorKind;
            let mut events = MioEvents::with_capacity(1024);

            // Sleep until next event
            if let Err(e) = self.poll.poll(&mut events, None) {
                // Should retry on EINTR
                if e.kind() == ErrorKind::Interrupted {
                    continue;
                }

                break Err(e);
            }

            for event in &events {
                match event.token() {
                    POLL_TIMER => {
                        if let Err(e) = self.context.handle_timer_ev() {
                            break 'main Err(e);
                        }
                    }

                    POLL_WAYLAND => {},
                    _ => unreachable!(),
                }
            }

            // Perform read() only when it's ready, returns None when there're already pending events
            if let Some(guard) = self.event_queue.prepare_read() {
                if let Err(e) = guard.read_events() {
                    // ErrorKind::WouldBlock here means there's no new messages to read
                    if e.kind() != ErrorKind::WouldBlock {
                        break Err(e);
                    }
                }
            }

            if let Err(e) = self.event_queue.dispatch_pending(&mut self.context, |_, _, _| {}) {
                break Err(e);
            }

            // Flush pending writes
            if let Err(e) = self.display.flush() {
                // ErrorKind::WouldBlock here means there're so many to write, retry later
                if e.kind() != ErrorKind::WouldBlock {
                    break Err(e);
                }
            }
        };

        match stop_reason {
            Ok(()) => log::info!("Server closed gracefully"),
            Err(e) => log::error!("Server aborted: {}", e),
        }
    }
}
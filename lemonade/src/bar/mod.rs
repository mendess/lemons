#![allow(dead_code)]

mod output;
mod seat;
mod surface;

use self::{output::Output, seat::Seat, surface::Surface};
use crate::bar::init::BackendInit;
use std::fmt::Debug;
pub use surface::SurfaceConfig;
use thiserror::Error;
use wayland_client::{
    protocol::{
        wl_compositor::WlCompositor, wl_display::WlDisplay, wl_output::WlOutput, wl_registry,
        wl_seat::WlSeat,
    },
    Connection, Dispatch, EventQueue, Proxy,
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

mod init {
    use wayland_client::{
        delegate_noop,
        protocol::{wl_compositor::WlCompositor, wl_registry, wl_shm::WlShm},
        Connection, Dispatch,
    };
    use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;
    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

    use super::BackendError;

    #[derive(Default)]
    pub struct BackendInit {
        pub compositor: Option<WlCompositor>,
        pub shm: Option<WlShm>,
        pub layer_shell: Option<ZwlrLayerShellV1>,
        pub cursor_shape_mgr: Option<WpCursorShapeManagerV1>,
    }

    pub fn backend(conn: &Connection) -> Result<BackendInit, BackendError> {
        let mut event_queue = conn.new_event_queue();
        let mut init = BackendInit {
            compositor: Default::default(),
            shm: Default::default(),
            cursor_shape_mgr: Default::default(),
            layer_shell: Default::default(),
        };
        let display = conn.display();
        display.get_registry(&event_queue.handle(), ());

        event_queue.roundtrip(&mut init)?;
        Ok(init)
    }

    impl Dispatch<wl_registry::WlRegistry, ()> for BackendInit {
        fn event(
            state: &mut Self,
            proxy: &wl_registry::WlRegistry,
            event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
            _data: &(),
            _conn: &Connection,
            qhandle: &wayland_client::QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global {
                name,
                interface,
                version,
            } = event
            {
                match interface.as_str() {
                    "wl_compositor" if version >= 4 => {
                        state.compositor =
                            Some(proxy.bind::<WlCompositor, _, _>(name, 1, qhandle, ()));
                    }
                    "wl_shm" => {
                        state.shm = Some(proxy.bind::<WlShm, _, _>(name, 1, qhandle, ()));
                    }
                    "zwlr_layer_shell_v1" if version >= 3 => {
                        state.layer_shell =
                            Some(proxy.bind::<ZwlrLayerShellV1, _, _>(name, 1, qhandle, ()));
                    }
                    "wp_cursor_shape_manager_v1" if version >= 1 => {
                        state.cursor_shape_mgr =
                            Some(proxy.bind::<WpCursorShapeManagerV1, _, _>(name, 1, qhandle, ()));
                    }
                    iface => {
                        eprintln!("ignored interface {iface}")
                    }
                }
            }
        }
    }

    delegate_noop!(BackendInit: ignore WlCompositor);
    delegate_noop!(BackendInit: ignore WlShm);
    delegate_noop!(BackendInit: ignore ZwlrLayerShellV1);
    delegate_noop!(BackendInit: ignore WpCursorShapeManagerV1);
}

pub struct Bar {
    conn: Connection,
    display: WlDisplay,

    compositor: WlCompositor,
    layer_shell: ZwlrLayerShellV1,
    cursor_shape_mgr: WpCursorShapeManagerV1,

    seat: Vec<Seat>,
    outputs: Vec<Output>,
    surface: Surface,
}

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("connect error: {0}")]
    Connect(#[from] wayland_client::ConnectError),
    #[error("dispatch error: {0}")]
    Dispatch(#[from] wayland_client::DispatchError),
}

impl Bar {
    pub fn new(config: SurfaceConfig) -> Result<(Self, EventQueue<Self>), BackendError> {
        let conn = Connection::connect_to_env()?;
        let BackendInit {
            compositor,
            shm,
            layer_shell,
            cursor_shape_mgr,
        } = init::backend(&conn)?;

        let mut event_queue = conn.new_event_queue();
        let display = conn.display();
        display.get_registry(&event_queue.handle(), ());

        let compositor = compositor.expect("compositor not advertized");
        let layer_shell = layer_shell.expect("layer_shell not advertized");
        let shm = shm.expect("shm not advertized");

        let mut this = Self {
            display,
            conn,
            surface: Surface::new(
                config,
                shm,
                &compositor,
                &layer_shell,
                &event_queue.handle(),
            ),
            compositor,
            cursor_shape_mgr: cursor_shape_mgr.expect("cursor_shape_mgr not advertized"),
            layer_shell,
            seat: Default::default(),
            outputs: Default::default(),
        };

        event_queue.roundtrip(&mut this)?;

        Ok((this, event_queue))
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Bar {
    fn event(
        state: &mut Self,
        proxy: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => match interface.as_str() {
                "wl_seat" => {
                    let seat = proxy.bind::<WlSeat, _, _>(name, 1, qhandle, state.seat.len());
                    state.seat.push(Seat::new(seat.id()));
                }
                "wl_output" if version >= 4 => {
                    proxy.bind::<WlOutput, _, _>(name, 1, qhandle, state.outputs.len());
                    state.outputs.push(Output::new(name));
                }
                iface => {
                    eprintln!("ignored interface {iface}")
                }
            },
            wl_registry::Event::GlobalRemove { .. } => todo!(),
            _ => todo!(),
        }
    }
}

use wayland_client::{
    protocol::wl_output::{self, WlOutput},
    Connection, Dispatch, Proxy, QueueHandle,
};

pub struct Output {
    wl_name: u32,
    name: Option<String>,
    configured: bool,
}

impl Output {
    pub fn new(wl_name: u32) -> Self {
        Self {
            wl_name,
            name: None,
            configured: false,
        }
    }
}

impl Dispatch<WlOutput, usize> for super::Bar {
    fn event(
        state: &mut Self,
        _proxy: &WlOutput,
        event: <WlOutput as Proxy>::Event,
        data: &usize,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Done => {
                eprintln!("output configured");
                state.outputs[*data].configured = true;
            }
            wl_output::Event::Name { name } => {
                state.outputs[*data].name = Some(name);
            }
            _ => {}
        }
    }
}

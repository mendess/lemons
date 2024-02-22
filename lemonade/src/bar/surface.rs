use std::{io, os::fd::AsFd, sync::atomic::AtomicU64};

use thiserror::Error;
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer::{self, WlBuffer},
        wl_callback::{self, WlCallback},
        wl_compositor::WlCompositor,
        wl_shm::{self, WlShm},
        wl_surface::WlSurface,
    },
    Dispatch, QueueHandle,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor},
};

#[derive(Default, Debug)]
struct BufferPool {
    buffers: Vec<Buffer>,
}

impl BufferPool {
    const MAX_BUFFER_MULTIPLICITY: u32 = 3;

    fn next_buffer(
        &mut self,
        shm: &WlShm,
        width: i32,
        height: i32,
        qhandle: &QueueHandle<super::Bar>,
    ) -> Result<&mut Buffer, BufferError> {
        let mut last_seen = None;
        for (i, b) in self.buffers.iter().enumerate().filter(|(_, b)| !b.busy) {
            if b.height == height && b.width == width {
                return Ok(&mut self.buffers[i]);
            }
            last_seen = Some(i)
        }
        match last_seen {
            Some(i) => Ok(&mut self.buffers[i]),
            None => self.new_buffer(shm, width, height, qhandle),
        }
    }

    fn new_buffer(
        &mut self,
        shm: &WlShm,
        width: i32,
        height: i32,
        qhandle: &QueueHandle<super::Bar>,
    ) -> Result<&mut Buffer, BufferError> {
        self.buffers.push(Buffer::new(shm, width, height, qhandle)?);
        Ok(self.buffers.last_mut().unwrap())
    }
}

#[derive(Debug)]
struct Buffer {
    buffer: WlBuffer,
    id: u64,
    width: i32,
    height: i32,
    busy: bool,
}

#[derive(Error, Debug)]
pub enum BufferError {
    #[error("memfd: {0}")]
    Memfd(#[from] memfd::Error),
    #[error("io: {0}")]
    Io(#[from] io::Error),
}

impl Buffer {
    fn new(
        shm: &WlShm,
        width: i32,
        height: i32,
        qhandle: &QueueHandle<super::Bar>,
    ) -> Result<Self, BufferError> {
        static BUFFER_ID: AtomicU64 = AtomicU64::new(0);

        let stride = width << 2;
        let size = stride * height;
        let fd = memfd::MemfdOptions::new().create("lemonade-shm-buffer-pool")?;
        fd.as_file().set_len(size.try_into().unwrap())?;
        let pool = shm.create_pool(fd.as_file().as_fd(), size, qhandle, ());
        let buffer_id = BUFFER_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let buffer = pool.create_buffer(
            0,
            width,
            height,
            stride,
            wl_shm::Format::Argb8888,
            qhandle,
            buffer_id,
        );
        Ok(Self {
            buffer,
            id: buffer_id,
            height,
            width,
            busy: false,
        })
    }
}

delegate_noop!(super::Bar: ignore wayland_client::protocol::wl_shm_pool::WlShmPool);

impl Dispatch<WlBuffer, u64> for super::Bar {
    fn event(
        state: &mut Self,
        _proxy: &WlBuffer,
        event: <WlBuffer as wayland_client::Proxy>::Event,
        data: &u64,
        _conn: &wayland_client::Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            if let Some(b) = state
                .surface
                .pool
                .buffers
                .iter_mut()
                .find(|b| b.id == *data)
            {
                b.busy = false;
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct SurfaceConfig {
    pub margins_top: i32,
    pub margins_bottom: i32,
    pub margins_right: i32,
    pub margins_left: i32,
    pub anchor: Anchor,
    pub layer: zwlr_layer_shell_v1::Layer,
    pub height: u32,
}

#[derive(Debug)]
pub struct Surface {
    surface: WlSurface,
    shm: WlShm,
    layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,

    hotspots: Vec<HotSpot>,
    pool: BufferPool,

    // True if we requested a frame callback.
    frame_pending: bool,
    // True if we need to redraw.
    dirty: bool,

    width: i32,
    height: i32,

    config: SurfaceConfig,

    configured: bool,
}

impl Surface {
    pub fn new(
        config: SurfaceConfig,
        shm: WlShm,
        compositor: &WlCompositor,
        layer_shell: &ZwlrLayerShellV1,
        qhandle: &QueueHandle<super::Bar>,
    ) -> Self {
        let surface = compositor.create_surface(qhandle, ());
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            None,
            zwlr_layer_shell_v1::Layer::Bottom,
            "panel".into(),
            qhandle,
            (),
        );

        let this = Self {
            surface,
            shm,
            layer_surface,
            hotspots: Default::default(),
            pool: Default::default(),
            frame_pending: false,
            dirty: false,
            width: 0,
            height: 0,
            config,
            configured: false,
        };

        this.resize_layer_surface();

        this
    }

    pub fn hotspot_from_point(&self, x: f64, y: f64) -> Option<&HotSpot> {
        self.hotspots.iter().find(|hs| hs.contains(x, y))
    }

    pub fn send_frame(&mut self, qhandle: &QueueHandle<super::Bar>) -> Result<(), BufferError> {
        if !self.configured {
            return Ok(());
        }

        let buffer = self
            .pool
            .next_buffer(&self.shm, self.width, self.height, qhandle)?;

        self.surface.attach(Some(&buffer.buffer), 0, 0);
        self.surface
            .damage_buffer(0, 0, buffer.width, buffer.height);
        buffer.busy = true;

        self.schedule_frame_and_commit(qhandle);
        self.dirty = false;

        Ok(())
    }

    fn resize_layer_surface(&self) {
        self.layer_surface.set_size(0, self.config.height);
        self.layer_surface.set_exclusive_zone(self.height as _);
        match self.config.layer {
            zwlr_layer_shell_v1::Layer::Overlay => self.layer_surface.set_exclusive_zone(0),
            _ => self
                .layer_surface
                .set_anchor(Anchor::Top | Anchor::Right | Anchor::Left),
        }
        self.layer_surface.set_margin(
            self.config.margins_top,
            self.config.margins_right,
            self.config.margins_bottom,
            self.config.margins_left,
        );

        self.surface.commit();
    }

    fn schedule_frame_and_commit(&mut self, qhandle: &QueueHandle<super::Bar>) {
        if self.frame_pending {
            return;
        }

        self.surface.frame(qhandle, ());
        self.surface.commit();
        self.frame_pending = true;
    }
}

delegate_noop!(super::Bar: ignore WlSurface);

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for super::Bar {
    fn event(
        state: &mut Self,
        _proxy: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: <zwlr_layer_surface_v1::ZwlrLayerSurfaceV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                let surface = &mut state.surface;
                surface.layer_surface.ack_configure(serial);
                if surface.configured
                    && surface.width == width as i32
                    && surface.height == height as i32
                {
                    surface.surface.commit();
                    return;
                }

                surface.configured = true;
                surface.width = width as _;
                surface.height = height as _;
                let _ = surface.send_frame(qhandle);
            }
            zwlr_layer_surface_v1::Event::Closed => {}
            _ => {}
        }
    }
}

impl Dispatch<WlCallback, ()> for super::Bar {
    fn event(
        state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.surface.frame_pending = false;
            if state.surface.dirty {
                let _ = state.surface.send_frame(qhandle);
            }
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.configured = false;
        self.layer_surface.destroy();
        self.surface.destroy();
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct HotSpot {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    action: [Option<String>; 5],
}

impl HotSpot {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    pub fn act(&self, index: usize) {
        if let Some(Some(action)) = self.action.get(index) {
            println!("{action}")
        }
    }
}

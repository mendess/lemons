use wayland_client::{
    backend::ObjectId,
    delegate_noop,
    protocol::{
        wl_pointer::{self, WlPointer},
        wl_seat::{self, WlSeat},
    },
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::{self, WpCursorShapeDeviceV1},
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};

use super::surface::{HotSpot, Surface};

pub struct Seat {
    id: ObjectId,
    pointer: Option<WlPointer>,
    cursor_shape_device: Option<WpCursorShapeDeviceV1>,
    pointer_x: f64,
    pointer_y: f64,
    cursor_shape: Option<wp_cursor_shape_device_v1::Shape>,
    last_enter_serial: Option<u32>,
    press_hotspot: Option<HotSpot>,
}

impl Seat {
    pub fn new(id: ObjectId) -> Self {
        Self {
            id,
            pointer: None,
            cursor_shape_device: None,
            pointer_x: 0.0,
            pointer_y: 0.0,
            cursor_shape: None,
            last_enter_serial: None,
            press_hotspot: None,
        }
    }

    fn bind_pointer(
        &mut self,
        cursor_shape_mgr: &WpCursorShapeManagerV1,
        seat: &WlSeat,
        qhandle: &QueueHandle<super::Bar>,
    ) {
        if self.pointer.is_some() {
            return;
        }
        self.pointer = Some(seat.get_pointer(qhandle, self.id.clone()));

        self.cursor_shape_device =
            Some(cursor_shape_mgr.get_pointer(self.pointer.as_ref().unwrap(), qhandle, ()));
    }

    fn release_pointer(&mut self) {
        self.press_hotspot = None;
        if let Some(p) = self.pointer.take() {
            p.release();
        }
        if let Some(dev) = self.cursor_shape_device.take() {
            dev.destroy();
        }
        todo!("cursor_theme cursor_shape cursor_surface");
    }

    fn update_pointer(
        &mut self,
        surface: &Surface,
        surface_x: f64,
        surface_y: f64,
        serial: Option<u32>,
    ) {
        self.pointer_x = surface_x;
        self.pointer_y = surface_y;
        if let Some(serial) = serial {
            self.last_enter_serial = Some(serial)
        }
        if surface
            .hotspot_from_point(self.pointer_x, self.pointer_y)
            .is_some()
        {
            self.set_cursor(wp_cursor_shape_device_v1::Shape::Pointer);
        } else {
            self.set_cursor(wp_cursor_shape_device_v1::Shape::Default);
        }
    }

    fn set_cursor(&mut self, shape: wp_cursor_shape_device_v1::Shape) {
        if self.cursor_shape == Some(shape) {
            return;
        }
        self.cursor_shape = Some(shape);

        // If the Wayland server supports the CursorShapeV1 protocol, use that
        // to set the cursor. Otherwise we need to go through wl-cursor.
        if let Some(csd) = self.cursor_shape_device.as_ref() {
            csd.set_shape(
                self.last_enter_serial
                    .expect("this should be set by update pointer"),
                shape,
            )
        }
    }
}

impl Dispatch<WlSeat, usize> for super::Bar {
    fn event(
        state: &mut Self,
        proxy: &WlSeat,
        event: <WlSeat as wayland_client::Proxy>::Event,
        index: &usize,
        _conn: &Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            wayland_client::protocol::wl_seat::Event::Capabilities {
                capabilities: WEnum::Value(wl_seat::Capability::Pointer),
            } => {
                state.seat[*index].bind_pointer(&state.cursor_shape_mgr, proxy, qhandle);
            }
            wayland_client::protocol::wl_seat::Event::Capabilities { capabilities: _ } => {
                state.seat[*index].release_pointer();
            }
            wayland_client::protocol::wl_seat::Event::Name { .. } => {}
            _ => todo!(),
        }
    }
}

impl Dispatch<WlPointer, ObjectId> for super::Bar {
    fn event(
        state: &mut Self,
        _proxy: &WlPointer,
        event: <WlPointer as wayland_client::Proxy>::Event,
        data: &ObjectId,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let Some(seat) = state.seat.iter_mut().find(|s| s.id == *data) else {
            eprintln!("{data} didn't map to any seat");
            return;
        };
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface: _,
                surface_x,
                surface_y,
            } => {
                seat.update_pointer(&state.surface, surface_x, surface_y, Some(serial));
            }
            wl_pointer::Event::Leave { .. } => todo!(),
            wl_pointer::Event::Motion {
                time: _,
                surface_x,
                surface_y,
            } => seat.update_pointer(&state.surface, surface_x, surface_y, None),
            wl_pointer::Event::Button {
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => {
                seat.press_hotspot = state
                    .surface
                    .hotspot_from_point(seat.pointer_x, seat.pointer_y)
                    .cloned();
            }
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(wl_pointer::ButtonState::Released),
                ..
            } => {
                let Some(hostpot) = seat.press_hotspot.take() else {
                    return;
                };
                if let Some(new_hotspot) = state
                    .surface
                    .hotspot_from_point(seat.pointer_x, seat.pointer_y)
                {
                    if hostpot == *new_hotspot {
                        hostpot.act(button as usize)
                    };
                }
            }
            _ => {}
        }
    }
}

delegate_noop!(super::Bar: ignore WpCursorShapeDeviceV1);

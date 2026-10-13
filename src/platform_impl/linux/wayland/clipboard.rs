//! Shares the seat's existing data device with clipboard consumers.
use super::seat::dnd::OfferData;
use ahash::AHashMap;
use sctk::reexports::client::protocol::{wl_data_device::WlDataDevice, wl_data_offer::WlDataOffer};
use sctk::reexports::client::{backend::ObjectId, Proxy};
use std::sync::{Arc, Mutex, PoisonError};

/// An owned clipboard view of the app's Wayland connection. It shares winit's data devices,
/// selection offers and focus serials; another data device or keyboard is not created.
#[derive(Clone)]
pub struct WaylandClipboard {
    pub(crate) state: Arc<Mutex<ClipboardState>>,
    pub(crate) waker: calloop::ping::Ping,
}
impl WaylandClipboard {
    /// Stable identity for sharing one worker across all windows of this event loop.
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.state) as usize
    }
    /// Wakes protocol dispatch without requesting a redraw. A background event queue must
    /// call this after dispatch, since reading the shared socket can queue events for winit.
    pub fn wake_event_loop(&self) {
        self.waker.ping();
    }
    /// The focused seat's data device and latest input serial.
    pub fn focused_device(&self) -> Result<(WlDataDevice, u32), String> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let seat = state.active()?;
        Ok((seat.device.clone(), seat.serial))
    }
    /// The focused seat's current selection and bounded advertised MIME types.
    pub fn selection(&self) -> Result<Option<(WlDataOffer, Vec<String>)>, String> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let seat = state.active()?;
        Ok(seat.offer.as_ref().filter(|offer| offer.is_alive()).map(|offer| {
            let mimes = offer.data::<OfferData>().map(OfferData::mime_types).unwrap_or_default();
            (offer.clone(), mimes)
        }))
    }
}
struct Seat {
    device: WlDataDevice,
    offer: Option<WlDataOffer>,
    focused: bool,
    serial: u32,
    order: u64,
}
#[derive(Default)]
pub(crate) struct ClipboardState {
    seats: AHashMap<ObjectId, Seat>,
    order: u64,
}
impl ClipboardState {
    fn active(&self) -> Result<&Seat, String> {
        self.seats
            .values()
            .filter(|seat| seat.focused && seat.device.is_alive())
            .max_by_key(|seat| seat.order)
            .ok_or_else(|| "The app does not have clipboard focus".into())
    }
    pub fn register(&mut self, seat: ObjectId, device: WlDataDevice) {
        if self.seats.contains_key(&seat) || self.seats.len() >= 16 {
            return;
        }
        self.seats.insert(seat, Seat { device, offer: None, focused: false, serial: 0, order: 0 });
    }
    pub fn focus(&mut self, id: &ObjectId, focused: bool, serial: u32) {
        if let Some(seat) = self.seats.get_mut(id) {
            self.order = self.order.saturating_add(1);
            seat.focused = focused;
            seat.serial = serial;
            seat.order = self.order;
        }
    }
    pub fn input(&mut self, id: &ObjectId, serial: u32) {
        if let Some(seat) = self.seats.get_mut(id).filter(|seat| seat.focused) {
            self.order = self.order.saturating_add(1);
            seat.serial = serial;
            seat.order = self.order;
        }
    }
    pub fn selection(&mut self, device: &WlDataDevice, offer: Option<WlDataOffer>) {
        if let Some(seat) = self.seats.values_mut().find(|seat| seat.device == *device) {
            if let Some(old) = seat.offer.take().filter(|old| old.is_alive()) {
                old.destroy();
            }
            seat.offer = offer;
        } else if let Some(offer) = offer.filter(|offer| offer.is_alive()) {
            offer.destroy();
        }
    }
    pub fn remove(&mut self, id: &ObjectId) {
        if let Some(seat) = self.seats.remove(id) {
            if let Some(offer) = seat.offer.filter(|offer| offer.is_alive()) {
                offer.destroy();
            }
        }
    }
}

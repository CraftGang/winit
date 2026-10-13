//! Bounded asynchronous native file drops. Based on MixaDoDs' 0.30 compatibility
//! patch; transfers own their offers so another drag/seat cannot replace a pending drop.
use crate::dpi::LogicalPosition;
use crate::event::WindowEvent;
use crate::platform_impl::wayland::state::WinitState;
use crate::platform_impl::wayland::{self, DeviceId, WindowId};
use ahash::AHashMap;
use calloop::{PostAction, RegistrationToken};
use sctk::compositor::SurfaceData;
use sctk::reexports::client::backend::ObjectId;
use sctk::reexports::client::protocol::{
    wl_data_device::{self, WlDataDevice},
    wl_data_device_manager::{DndAction, WlDataDeviceManager},
    wl_data_offer::{self, WlDataOffer},
    wl_surface::WlSurface,
};
use sctk::reexports::client::{
    event_created_child, Connection, Dispatch, Proxy, QueueHandle, WEnum,
};
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use tracing::warn;
const URI_LIST: &str = "text/uri-list";
const PORTAL: &str = "application/vnd.portal.filetransfer";
const MAX_URI_LIST: usize = 16 << 20;
const MAX_FILES: usize = 4096;
const MAX_TRANSFERS: usize = 8;
#[derive(Debug, Default)]
pub struct OfferData(Mutex<OfferState>);
#[derive(Debug)]
struct OfferState {
    files: bool,
    portal: bool,
    mimes: Vec<String>,
    selected: DndAction,
}
impl Default for OfferState {
    fn default() -> Self {
        Self { files: false, portal: false, mimes: Vec::new(), selected: DndAction::empty() }
    }
}
impl OfferData {
    pub(crate) fn mime_types(&self) -> Vec<String> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).mimes.clone()
    }
}
#[derive(Debug)]
struct Transfer {
    token: u64,
    window_id: WindowId,
    device_id: ObjectId,
    offer: WlDataOffer,
    reader: Option<RegistrationToken>,
    timeout: Option<RegistrationToken>,
    position: (f64, f64),
    buf: Vec<u8>,
    paths: Option<Vec<PathBuf>>,
    dropped: bool,
    portal: bool,
    portal_key: Option<String>,
    portal_queued: bool,
}
#[derive(Debug, Default)]
pub struct DndState {
    next_token: u64,
    transfers: AHashMap<ObjectId, Transfer>,
    current: AHashMap<ObjectId, ObjectId>,
}
impl Dispatch<WlDataDeviceManager, ()> for WinitState {
    fn event(
        _: &mut Self,
        _: &WlDataDeviceManager,
        _: <WlDataDeviceManager as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WlDataOffer, OfferData> for WinitState {
    fn event(
        _: &mut Self,
        _: &WlDataOffer,
        event: wl_data_offer::Event,
        data: &OfferData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mut offer = data.0.lock().unwrap_or_else(PoisonError::into_inner);
        match event {
            wl_data_offer::Event::Offer { mime_type } => {
                if offer.mimes.len() < 32
                    && !mime_type.is_empty()
                    && mime_type.len() <= 256
                    && mime_type.bytes().all(|b| b.is_ascii() && !b.is_ascii_control())
                    && !offer.mimes.contains(&mime_type)
                {
                    offer.mimes.push(mime_type.clone());
                }
                offer.files |= mime_type == URI_LIST;
                offer.portal |= mime_type == PORTAL;
            },
            wl_data_offer::Event::Action { dnd_action } => {
                offer.selected = match dnd_action {
                    WEnum::Value(action) => action,
                    _ => DndAction::empty(),
                }
            },
            _ => {},
        }
    }
}
impl WinitState {
    fn dnd_position(&mut self, window_id: WindowId, x: f64, y: f64) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let Some(window) = self.windows.get_mut().get(&window_id) else {
            return;
        };
        let scale = window.lock().unwrap_or_else(PoisonError::into_inner).scale_factor();
        let position = LogicalPosition::new(x, y).to_physical(scale);
        let device_id = crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(DeviceId));
        self.events_sink
            .push_window_event(WindowEvent::CursorMoved { device_id, position }, window_id);
        self.dispatched_events = true;
    }
    fn dnd_end(&mut self, id: &ObjectId, cancel: bool) {
        let Some(transfer) = self.dnd.transfers.remove(id) else {
            return;
        };
        if let Some(token) = transfer.reader {
            self.loop_handle.remove(token);
        }
        if let Some(token) = transfer.timeout {
            self.loop_handle.remove(token);
        }
        if self.dnd.current.get(&transfer.device_id) == Some(id) {
            self.dnd.current.remove(&transfer.device_id);
        }
        if let Some(window) = self.windows.get_mut().get(&transfer.window_id) {
            window
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .file_transfer_tokens
                .retain(|token| token.id != transfer.token);
        }
        if cancel && self.windows.get_mut().contains_key(&transfer.window_id) {
            self.events_sink
                .push_window_event(WindowEvent::HoveredFileCancelled, transfer.window_id);
            let device_id =
                crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(DeviceId));
            self.events_sink
                .push_window_event(WindowEvent::CursorLeft { device_id }, transfer.window_id);
        }
        if transfer.offer.is_alive() {
            transfer.offer.destroy();
        }
        self.dispatched_events = true;
    }
    pub fn dnd_remove_device(&mut self, device: &WlDataDevice) {
        let ids: Vec<_> = self
            .dnd
            .transfers
            .iter()
            .filter(|(_, t)| t.device_id == device.id())
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.dnd_end(&id, true);
        }
    }
    fn dnd_read_finished(&mut self, id: &ObjectId, success: bool) {
        let Some(transfer) = self.dnd.transfers.get_mut(id) else {
            return;
        };
        transfer.reader = None;
        transfer.portal_key =
            if success && transfer.portal { parse_portal_key(&transfer.buf) } else { None };
        let paths =
            if success && !transfer.portal { parse_uri_list(&transfer.buf) } else { Vec::new() };
        transfer.buf = Vec::new();
        if !transfer.dropped {
            for path in &paths {
                self.events_sink
                    .push_window_event(WindowEvent::HoveredFile(path.clone()), transfer.window_id);
            }
        }
        transfer.paths = Some(paths);
        let dropped = transfer.dropped;
        if dropped {
            self.dnd_deliver(id);
        }
        self.dispatched_events = true;
    }
    pub fn dnd_complete_portal(&mut self, window: WindowId, token: u64) {
        let id = self.dnd.transfers.iter().find_map(|(id, t)| {
            (t.window_id == window && t.token == token && t.portal_queued).then(|| id.clone())
        });
        if let Some(id) = id {
            if let Some(t) = self.dnd.transfers.get_mut(&id) {
                t.portal_key = None;
            }
            self.dnd_deliver(&id);
        }
    }
    pub fn dnd_remove_window(&mut self, window: WindowId) {
        let ids: Vec<_> = self
            .dnd
            .transfers
            .iter()
            .filter(|(_, t)| t.window_id == window)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.dnd_end(&id, true);
        }
    }
    fn dnd_deliver(&mut self, id: &ObjectId) {
        if let Some(transfer) = self.dnd.transfers.get_mut(id) {
            if let Some(key) = &transfer.portal_key {
                if transfer.portal_queued {
                    return;
                }
                let selected = transfer
                    .offer
                    .data::<OfferData>()
                    .map(|data| data.0.lock().unwrap_or_else(PoisonError::into_inner).selected);
                if transfer.offer.version() < 3 || selected == Some(DndAction::Copy) {
                    if let Some(window) = self.windows.get_mut().get(&transfer.window_id) {
                        let mut window = window.lock().unwrap_or_else(PoisonError::into_inner);
                        let position =
                            LogicalPosition::new(transfer.position.0, transfer.position.1)
                                .to_physical(window.scale_factor());
                        window.file_transfer_tokens.push(
                            crate::platform::wayland::FileTransferToken {
                                id: transfer.token,
                                key: key.clone(),
                                position: LogicalPosition::new(
                                    transfer.position.0,
                                    transfer.position.1,
                                ),
                            },
                        );
                        transfer.portal_queued = true;
                        self.events_sink.push_window_event(
                            WindowEvent::HoveredFileCancelled,
                            transfer.window_id,
                        );
                        self.events_sink.push_window_event(
                            WindowEvent::CursorMoved {
                                device_id: crate::event::DeviceId(
                                    crate::platform_impl::DeviceId::Wayland(DeviceId),
                                ),
                                position,
                            },
                            transfer.window_id,
                        );
                        self.dispatched_events = true;
                        return;
                    }
                }
            }
        }
        let Some(transfer) = self.dnd.transfers.remove(id) else {
            return;
        };
        if self.dnd.current.get(&transfer.device_id) == Some(id) {
            self.dnd.current.remove(&transfer.device_id);
        }
        if let Some(token) = transfer.timeout {
            self.loop_handle.remove(token);
        }
        self.dnd_position(transfer.window_id, transfer.position.0, transfer.position.1);
        let paths = transfer.paths.unwrap_or_default();
        let selected = transfer
            .offer
            .data::<OfferData>()
            .map(|data| data.0.lock().unwrap_or_else(PoisonError::into_inner).selected);
        let copy = transfer.offer.version() < 3 || selected == Some(DndAction::Copy);
        if copy && self.windows.get_mut().contains_key(&transfer.window_id) {
            for path in &paths {
                self.events_sink
                    .push_window_event(WindowEvent::DroppedFile(path.clone()), transfer.window_id);
            }
        }
        if paths.is_empty() {
            self.events_sink
                .push_window_event(WindowEvent::HoveredFileCancelled, transfer.window_id);
        }
        if transfer.offer.is_alive() {
            if copy && transfer.offer.version() >= 3 {
                transfer.offer.finish();
            }
            transfer.offer.destroy();
        }
        self.dispatched_events = true;
    }
    fn dnd_enter(
        &mut self,
        device: &WlDataDevice,
        serial: u32,
        surface: &WlSurface,
        x: f64,
        y: f64,
        offer: Option<WlDataOffer>,
    ) {
        if let Some(old) = self.dnd.current.remove(&device.id()) {
            if !self.dnd.transfers.get(&old).is_some_and(|t| t.dropped) {
                self.dnd_end(&old, true);
            }
        }
        let Some(offer) = offer else {
            return;
        };
        let parent =
            surface.data::<SurfaceData>().and_then(|d| d.parent_surface()).unwrap_or(surface);
        let window_id = wayland::make_wid(parent);
        let (files, portal) = offer
            .data::<OfferData>()
            .map(|data| {
                let data = data.0.lock().unwrap_or_else(PoisonError::into_inner);
                (
                    data.files,
                    data.portal && (std::env::var_os("FLATPAK_ID").is_some() || !data.files),
                )
            })
            .unwrap_or_default();
        if (!files && !portal)
            || !self.windows.get_mut().contains_key(&window_id)
            || self.dnd.transfers.len() >= MAX_TRANSFERS
            || self.dnd.next_token == u64::MAX
        {
            offer.accept(serial, None);
            if offer.version() >= 3 {
                offer.set_actions(DndAction::empty(), DndAction::empty());
            }
            offer.destroy();
            return;
        }
        let mime = if portal { PORTAL } else { URI_LIST };
        offer.accept(serial, Some(mime.into()));
        if offer.version() >= 3 {
            offer.set_actions(DndAction::Copy, DndAction::Copy);
        }
        let id = offer.id();
        self.dnd.next_token += 1;
        let mut transfer = Transfer {
            token: self.dnd.next_token,
            window_id,
            device_id: device.id(),
            offer: offer.clone(),
            reader: None,
            timeout: None,
            position: (x, y),
            buf: Vec::new(),
            paths: None,
            dropped: false,
            portal,
            portal_key: None,
            portal_queued: false,
        };
        match sctk::data_device_manager::data_offer::receive(&offer, mime.into()) {
            Ok(pipe) => {
                // Readiness is advisory: a nonblocking descriptor prevents a spurious wake from stalling UI.
                let flags = rustix::fs::fcntl_getfl(&pipe).and_then(|flags| {
                    rustix::fs::fcntl_setfl(&pipe, flags | rustix::fs::OFlags::NONBLOCK)
                });
                if flags.is_ok() {
                    let key = id.clone();
                    match self.loop_handle.insert_source(
                        pipe,
                        move |_, pipe, state: &mut WinitState| {
                            let mut chunk = [0u8; 8192];
                            match rustix::io::read(pipe, &mut chunk).map_err(io::Error::from) {
                                Ok(0) => {
                                    state.dnd_read_finished(&key, true);
                                    PostAction::Remove
                                },
                                Ok(n) => {
                                    let Some(transfer) = state.dnd.transfers.get_mut(&key) else {
                                        return PostAction::Remove;
                                    };
                                    if transfer.buf.len().saturating_add(n) > MAX_URI_LIST {
                                        warn!("file-drop URI list too large");
                                        state.dnd_read_finished(&key, false);
                                        PostAction::Remove
                                    } else {
                                        transfer.buf.extend_from_slice(&chunk[..n]);
                                        PostAction::Continue
                                    }
                                },
                                Err(e)
                                    if matches!(
                                        e.kind(),
                                        ErrorKind::Interrupted | ErrorKind::WouldBlock
                                    ) =>
                                {
                                    PostAction::Continue
                                },
                                Err(e) => {
                                    warn!("file-drop read failed: {e}");
                                    state.dnd_read_finished(&key, false);
                                    PostAction::Remove
                                },
                            }
                        },
                    ) {
                        Ok(token) => transfer.reader = Some(token),
                        Err(e) => warn!("file-drop readiness registration failed: {e}"),
                    }
                }
            },
            Err(e) => warn!("file-drop pipe failed: {e}"),
        }
        if transfer.reader.is_none() {
            transfer.paths = Some(Vec::new());
        }
        let key = id.clone();
        transfer.timeout = self
            .loop_handle
            .insert_source(
                calloop::timer::Timer::from_duration(std::time::Duration::from_secs(30)),
                move |_, _, state: &mut WinitState| {
                    if let Some(t) = state.dnd.transfers.get_mut(&key) {
                        t.timeout = None;
                    }
                    state.dnd_end(&key, true);
                    calloop::timer::TimeoutAction::Drop
                },
            )
            .ok();
        self.dnd.transfers.insert(id.clone(), transfer);
        self.dnd.current.insert(device.id(), id);
        self.dnd_position(window_id, x, y);
    }
}
impl Dispatch<WlDataDevice, ()> for WinitState {
    event_created_child!(WinitState,WlDataDevice,[wl_data_device::EVT_DATA_OFFER_OPCODE=>(WlDataOffer,OfferData::default())]);
    fn event(
        state: &mut Self,
        device: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::Selection { id } => {
                state
                    .clipboard
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .selection(device, id);
            },
            wl_data_device::Event::Enter { serial, surface, x, y, id } => {
                state.dnd_enter(device, serial, &surface, x, y, id)
            },
            wl_data_device::Event::Leave => {
                if let Some(id) = state.dnd.current.remove(&device.id()) {
                    if !state.dnd.transfers.get(&id).is_some_and(|t| t.dropped) {
                        state.dnd_end(&id, true);
                    }
                }
            },
            wl_data_device::Event::Motion { x, y, .. } => {
                if let Some(window) = state
                    .dnd
                    .current
                    .get(&device.id())
                    .cloned()
                    .and_then(|id| state.dnd.transfers.get_mut(&id))
                    .map(|t| {
                        if x.is_finite() && y.is_finite() {
                            t.position = (x, y);
                        }
                        t.window_id
                    })
                {
                    state.dnd_position(window, x, y);
                }
            },
            wl_data_device::Event::Drop => {
                if let Some(id) = state.dnd.current.get(&device.id()).cloned() {
                    if let Some(t) = state.dnd.transfers.get_mut(&id) {
                        t.dropped = true;
                        if t.paths.is_some() {
                            state.dnd_deliver(&id);
                        }
                    }
                }
            },
            _ => {},
        }
    }
}
fn parse_portal_key(data: &[u8]) -> Option<String> {
    if data.len() > 4096 {
        return None;
    }
    let key = std::str::from_utf8(data).ok()?.trim();
    if key.is_empty() || key.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(key.to_owned())
}

/// The local paths in a `text/uri-list` (RFC 2483): `file:` URIs, percent-decoded; comments,
/// blank lines and other schemes are skipped.
pub(crate) fn parse_uri_list(data: &[u8]) -> Vec<PathBuf> {
    use std::os::unix::ffi::OsStringExt;

    let Ok(text) = std::str::from_utf8(data) else { return Vec::new() };
    let paths: Vec<_> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|uri| {
            let (scheme, rest) = uri.split_once(':')?;
            if !scheme.eq_ignore_ascii_case("file") {
                return None;
            }
            let authority = rest.starts_with("//");
            let rest = rest.strip_prefix("//").unwrap_or(rest);
            // `file://host/path`: the host is empty or `localhost` for local files.
            let path = if rest.starts_with('/') {
                rest
            } else if authority {
                let slash = rest.find('/')?;
                let host = &rest[..slash];
                if !host.is_empty()
                    && !host.eq_ignore_ascii_case("localhost")
                    && !is_this_host(host)
                {
                    return None;
                }
                &rest[slash..]
            } else {
                return None;
            };
            // URI delimiters cannot be interpreted as literal filename characters.
            if path.contains(['#', '?']) {
                return None;
            }
            let bytes = percent_decode(path.as_bytes())?;
            Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
        })
        .take(MAX_FILES + 1)
        .collect();
    if paths.len() > MAX_FILES {
        warn!("drag and drop: too many files, ignoring");
        Vec::new()
    } else {
        paths
    }
}

fn is_this_host(host: &str) -> bool {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .is_ok_and(|h| h.trim().eq_ignore_ascii_case(host))
}

fn percent_decode(s: &[u8]) -> Option<Vec<u8>> {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let byte = if s[i] == b'%' {
            let h = hex(*s.get(i + 1)?)?;
            let l = hex(*s.get(i + 2)?)?;
            i += 3;
            h << 4 | l
        } else {
            let byte = s[i];
            i += 1;
            byte
        };
        // POSIX paths cannot contain NUL; never emit a path the file API cannot open.
        if byte == 0 {
            return None;
        }
        out.push(byte);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_keys_are_bounded_strings() {
        assert_eq!(parse_portal_key(b"key-test\r\n"), Some("key-test".into()));
        for key in [b"".as_slice(), b"key\0data", b"key\nsecond", b"\xff"] {
            assert!(parse_portal_key(key).is_none());
        }
        assert!(parse_portal_key(&vec![b'a'; 4097]).is_none());
    }
    #[test]
    fn uri_list_paths() {
        let list = b"# comment\r\nfile:///home/me/a%20b.png\r\nfile://localhost/tmp/x.psd\r\n\
            https://example.com/c.png\r\nfile:///tmp/%D1%84.jpg\r\nfile:///tmp/bad%2\n";
        let paths = parse_uri_list(list);
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/home/me/a b.png"),
                PathBuf::from("/tmp/x.psd"),
                PathBuf::from("/tmp/ф.jpg"),
            ]
        );
    }

    #[test]
    fn invalid_uri_paths_are_not_exposed_as_files() {
        let list=b"file:///tmp/ok%23name.txt\nfile:///tmp/nul%00name.txt\nfile:///tmp/bad%2\nfile:///tmp/bad%GG\nfile:///tmp/with#fragment\nfile:///tmp/query?x=1\nfile://remote.invalid/tmp/a\nfile:localhost/tmp/invalid\nfile:relative/path\n";
        assert_eq!(parse_uri_list(list), vec![PathBuf::from("/tmp/ok#name.txt")]);
    }
    #[test]
    fn raw_invalid_utf8_is_rejected_but_percent_encoded_bytes_work() {
        use std::os::unix::ffi::OsStrExt;
        assert!(parse_uri_list(b"file:///tmp/\xff.txt").is_empty());
        let paths = parse_uri_list(b"file:///tmp/%FF.txt");
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].as_os_str().as_bytes(), b"/tmp/\xff.txt");
    }

    #[test]
    fn file_count_is_bounded_without_delivering_a_partial_drop() {
        let boundary = "file:///tmp/a\n".repeat(MAX_FILES);
        assert_eq!(parse_uri_list(boundary.as_bytes()).len(), MAX_FILES);
        let too_many = boundary + "file:///tmp/b\n";
        assert!(parse_uri_list(too_many.as_bytes()).is_empty());
    }
    #[test]
    fn local_scheme_and_authority_are_case_insensitive() {
        assert_eq!(
            parse_uri_list(b"FILE://LOCALHOST/tmp/a\nfile:/tmp/b\n"),
            vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")]
        );
    }
}

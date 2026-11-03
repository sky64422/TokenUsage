//! WGC backdrop capture. All COM/D3D resources stay on the capture worker.
use super::liquid_capture::{CapturePadding, FrameCache, FrameKey};
use std::{
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HMODULE, HWND, RECT},
    Graphics::{
        Direct3D::D3D_DRIVER_TYPE_HARDWARE,
        Direct3D11::*,
        Dxgi::{Common::*, IDXGIDevice},
        Gdi::*,
    },
    System::WinRT::{Direct3D11::*, Graphics::Capture::IGraphicsCaptureItemInterop},
    UI::WindowsAndMessaging::GetWindowRect,
};
use windows::{
    core::{IInspectable_Vtbl, Interface, HRESULT},
    Graphics::{
        Capture::*,
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    UI::WindowId,
};

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        default_enabled(
            cfg!(debug_assertions),
            std::env::var("TOKENUSAGE_WGC_PREVIEW").ok().as_deref(),
        )
    })
}

// Keep the old backend available only for explicit debug comparisons.
fn default_enabled(debug: bool, override_value: Option<&str>) -> bool {
    !debug || override_value != Some("0")
}

// Microsoft windows-rs commit 317646c941aa18916ffa5daeb4ffd5ffa4cd26f3,
// Windows/Graphics/Capture/mod.rs: published IInspectable + Set, Get ABI.
windows_core::imp::define_interface!(
    DisplaySession,
    DisplaySessionVtable,
    0xbb91f61b_218a_587d_8580_2701a74c0525
);
#[repr(C)]
pub struct DisplaySessionVtable {
    base: IInspectable_Vtbl,
    set: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
        *mut u64,
    ) -> HRESULT,
    get: unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> HRESULT,
}

const FIRST_FRAME_WAIT: Duration = Duration::from_millis(250);
const FRAME_POLL: Duration = Duration::from_millis(2);
const IDLE_SESSION: Duration = Duration::from_secs(3);
const DRAG_CACHE_MARGIN_PX: i32 = 128;

// Never changes OS policy or assumes that a property
// setter grants permission. The official request may require user interaction.
fn request_borderless() -> windows::core::Result<bool> {
    if !default_enabled(
        cfg!(debug_assertions),
        std::env::var("TOKENUSAGE_WGC_BORDERLESS_PREVIEW")
            .ok()
            .as_deref(),
    ) {
        return Ok(false);
    }
    static ACCESS: OnceLock<Result<bool, String>> = OnceLock::new();
    let allowed = ACCESS.get_or_init(|| {
        super::capture_log::log_event("WGC", "requesting official borderless access");
        GraphicsCaptureAccess::RequestAccessAsync(GraphicsCaptureAccessKind::Borderless)
            .and_then(|operation| operation.get())
            .map(|status| {
                let allowed = status == windows::Security::Authorization::AppCapabilityAccess::AppCapabilityAccessStatus::Allowed;
                super::capture_log::log_event("WGC", &format!("borderless access status={} allowed={allowed}", status.0));
                allowed
            }).map_err(|error| error.to_string())
    });
    match allowed {
        Ok(true) => Ok(true),
        Ok(false) => Err(windows::core::Error::new(
            windows::core::HRESULT(0x80070005u32 as i32),
            "WGC borderless access denied",
        )),
        Err(message) => Err(windows::core::Error::new(
            windows::core::HRESULT(0x80004005u32 as i32),
            message.clone(),
        )),
    }
}

pub struct WgcCapture {
    wake: Arc<dyn Fn() + Send + Sync>,
    failure: Option<String>,
    session: Option<MonitorSession>,
    caches: [FrameCache; 2],
}
impl WgcCapture {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            wake,
            failure: None,
            session: None,
            caches: Default::default(),
        }
    }
    pub fn refresh(&mut self) -> bool {
        let Some(session) = self.session.as_mut() else {
            return false;
        };
        match session.drain_frames() {
            Ok(changed) => changed,
            Err(error) => {
                self.failure = Some(format!("WGC frame arrival: {error}"));
                self.session = None;
                true // The next renderer request reports the failure and releases buffers.
            }
        }
    }

    pub fn capture(
        &mut self,
        hwnd: usize,
        key: FrameKey,
        slot: Option<usize>,
        dragging: bool,
        padding: CapturePadding,
        active_surfaces: u8,
    ) -> Result<Vec<u8>, String> {
        self.read(hwnd, key, slot, dragging, padding, active_surfaces)
            .map_err(|error| {
                self.session = None;
                self.caches.iter_mut().for_each(FrameCache::clear);
                format!("WGC (no GDI fallback): {error}")
            })
    }
    fn read(
        &mut self,
        hwnd: usize,
        key: FrameKey,
        slot: Option<usize>,
        dragging: bool,
        padding: CapturePadding,
        active_surfaces: u8,
    ) -> Result<Vec<u8>, String> {
        if let Some(error) = self.failure.take() {
            return Err(error);
        }
        let mut window_bounds = RECT::default();
        unsafe { GetWindowRect(HWND(hwnd as *mut _), &mut window_bounds) }
            .map_err(|e| e.to_string())?;
        let key = FrameKey {
            x: window_bounds
                .left
                .checked_add(key.x)
                .ok_or("WGC origin overflow")?,
            y: window_bounds
                .top
                .checked_add(key.y)
                .ok_or("WGC origin overflow")?,
            ..key
        };
        let original_key = key;
        let key = super::liquid_capture::padded_key(key, padding.image)?;
        let rect = RECT {
            left: key.x,
            top: key.y,
            right: key
                .x
                .checked_add_unsigned(key.width)
                .ok_or("WGC region overflow")?,
            bottom: key
                .y
                .checked_add_unsigned(key.height)
                .ok_or("WGC region overflow")?,
        };
        let original_rect = RECT {
            left: original_key.x,
            top: original_key.y,
            right: original_key.x + original_key.width as i32,
            bottom: original_key.y + original_key.height as i32,
        };
        let monitor = unsafe { MonitorFromRect(&original_rect, MONITOR_DEFAULTTONULL) };
        if monitor.0.is_null() {
            if dragging {
                return Ok(Vec::new());
            }
            return Err("WGC region has no monitor".into());
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            return Err("WGC monitor bounds unavailable".into());
        }
        let bounds = info.rcMonitor;
        if crop_region(original_key, bounds).is_none() {
            if dragging {
                return Ok(Vec::new());
            }
            return Err("WGC region crosses monitor bounds".into());
        }
        let margin = self
            .session
            .as_ref()
            .filter(|s| s.monitor == monitor && s.hwnd == hwnd)
            .map_or(padding.cache, |s| s.padding.max(padding.cache));
        window_bounds.left = window_bounds.left.saturating_sub(margin as i32);
        window_bounds.top = window_bounds.top.saturating_sub(margin as i32);
        window_bounds.right = window_bounds.right.saturating_add(margin as i32);
        window_bounds.bottom = window_bounds.bottom.saturating_add(margin as i32);
        // Keep nearby desktop pixels across small moves. A static desktop need
        // not produce another WGC frame merely because the excluded notch moves.
        if dragging {
            let previous = self
                .session
                .as_ref()
                .filter(|s| {
                    s.dragging && s.monitor == monitor && s.hwnd == hwnd && s.bounds == bounds
                })
                .map(|s| s.capture_bounds);
            window_bounds = drag_capture_bounds(rect, bounds, previous);
        }
        let capture_bounds = RECT {
            left: window_bounds.left.max(bounds.left),
            top: window_bounds.top.max(bounds.top),
            right: window_bounds.right.min(bounds.right),
            bottom: window_bounds.bottom.min(bounds.bottom),
        };
        // Only actual monitor edges may replicate pixels; never a notch boundary.
        let sampled = clipped_key(key, bounds).ok_or("WGC empty monitor sample")?;
        let Some(crop) = crop_region(sampled, capture_bounds) else {
            // A rail crossing a monitor seam can temporarily span two outputs.
            // Keep the last paint until it fits instead of stopping the renderer.
            if dragging {
                return Ok(Vec::new());
            }
            return Err("WGC region crosses canvas bounds".into());
        };
        if self.session.as_ref().is_none_or(|s| {
            s.monitor != monitor || s.hwnd != hwnd || s.bounds != bounds || margin > s.padding
        }) {
            self.session = None;
            self.caches.iter_mut().for_each(FrameCache::clear);
            self.session = Some(
                MonitorSession::new(monitor, bounds, capture_bounds, hwnd, self.wake.clone())
                    .map_err(|e| e.to_string())?,
            );
        }
        let session = self.session.as_mut().unwrap();
        // A closed detail must stop waking/readback of the rail for its motion.
        // Keep the reserved GPU texture so reopening still has its first image.
        for (index, key) in session.read_keys.iter_mut().enumerate() {
            if active_surfaces & (1 << index) == 0 {
                *key = None;
            }
        }
        session.set_capture_bounds(capture_bounds);
        session.dragging = dragging;
        session.padding = margin;
        let Some(pixels) = session
            .pixels(sampled, crop, slot.unwrap_or(0), slot.map(|_| key))
            .map_err(|e| e.to_string())?
        else {
            return Ok(Vec::new());
        };
        let pixels = extend_monitor_edges(pixels, sampled, key);
        if let Some(slot) = slot {
            super::liquid_events::record_region(slot, key);
            self.caches[slot].frame(key, &pixels)
        } else {
            Ok(super::liquid_capture::bmp_frame(
                key.width, key.height, &pixels,
            ))
        }
    }
    pub fn expire_idle(&mut self) {
        if self
            .session
            .as_ref()
            .is_some_and(|s| s.last_read.elapsed() >= IDLE_SESSION)
        {
            self.session = None;
            self.caches.iter_mut().for_each(FrameCache::clear);
        }
    }
}

fn dirty_overlaps(dirty: windows::Graphics::RectInt32, monitor: RECT, region: RECT) -> bool {
    let left = i64::from(monitor.left) + i64::from(dirty.X);
    let top = i64::from(monitor.top) + i64::from(dirty.Y);
    dirty.Width > 0
        && dirty.Height > 0
        && left < i64::from(region.right)
        && top < i64::from(region.bottom)
        && left + i64::from(dirty.Width) > i64::from(region.left)
        && top + i64::from(dirty.Height) > i64::from(region.top)
}

fn clipped_key(key: FrameKey, bounds: RECT) -> Option<FrameKey> {
    let x = key.x.max(bounds.left);
    let y = key.y.max(bounds.top);
    let right = key.x.checked_add_unsigned(key.width)?.min(bounds.right);
    let bottom = key.y.checked_add_unsigned(key.height)?.min(bounds.bottom);
    (right > x && bottom > y).then_some(FrameKey {
        x,
        y,
        width: (right - x) as u32,
        height: (bottom - y) as u32,
        ..key
    })
}

fn extend_monitor_edges(pixels: Vec<u8>, sampled: FrameKey, requested: FrameKey) -> Vec<u8> {
    if sampled == requested {
        return pixels;
    }
    let stride = requested.width as usize * 4;
    let source_stride = sampled.width as usize * 4;
    let left = (sampled.x - requested.x) as usize * 4;
    let mut output = vec![0; stride * requested.height as usize];
    for (y, row) in output.chunks_exact_mut(stride).enumerate() {
        let sy = (requested.y as i64 + y as i64 - sampled.y as i64)
            .clamp(0, sampled.height as i64 - 1) as usize;
        let source = &pixels[sy * source_stride..(sy + 1) * source_stride];
        // Copy the real neighbourhood in bulk; repeat only off-monitor columns.
        row[left..left + source_stride].copy_from_slice(source);
        for pixel in row[..left].chunks_exact_mut(4) {
            pixel.copy_from_slice(&source[..4]);
        }
        for pixel in row[left + source_stride..].chunks_exact_mut(4) {
            pixel.copy_from_slice(&source[source_stride - 4..]);
        }
    }
    output
}

fn crop_region(key: FrameKey, bounds: RECT) -> Option<D3D11_BOX> {
    let x = key.x.checked_sub(bounds.left)?;
    let y = key.y.checked_sub(bounds.top)?;
    let right = x.checked_add_unsigned(key.width)?;
    let bottom = y.checked_add_unsigned(key.height)?;
    if key.width == 0
        || key.height == 0
        || u64::from(key.width) * u64::from(key.height)
            > super::liquid_capture::MAX_PADDED_FRAME_PIXELS
        || x < 0
        || y < 0
        || right > bounds.right.checked_sub(bounds.left)?
        || bottom > bounds.bottom.checked_sub(bounds.top)?
    {
        return None;
    }
    Some(D3D11_BOX {
        left: x as u32,
        top: y as u32,
        right: right as u32,
        bottom: bottom as u32,
        front: 0,
        back: 1,
    })
}

fn drag_capture_bounds(rect: RECT, monitor: RECT, previous: Option<RECT>) -> RECT {
    // Off-monitor blur support is replicated, so it must not invalidate cached
    // real pixels on every move along the physical screen edge.
    let rect = RECT {
        left: rect.left.max(monitor.left),
        top: rect.top.max(monitor.top),
        right: rect.right.min(monitor.right),
        bottom: rect.bottom.min(monitor.bottom),
    };
    if let Some(cached) = previous {
        if rect.left >= cached.left
            && rect.top >= cached.top
            && rect.right <= cached.right
            && rect.bottom <= cached.bottom
        {
            return cached;
        }
    }
    RECT {
        left: rect
            .left
            .saturating_sub(DRAG_CACHE_MARGIN_PX)
            .max(monitor.left),
        top: rect
            .top
            .saturating_sub(DRAG_CACHE_MARGIN_PX)
            .max(monitor.top),
        right: rect
            .right
            .saturating_add(DRAG_CACHE_MARGIN_PX)
            .min(monitor.right),
        bottom: rect
            .bottom
            .saturating_add(DRAG_CACHE_MARGIN_PX)
            .min(monitor.bottom),
    }
}

struct MonitorSession {
    monitor: HMONITOR,
    bounds: RECT,
    capture_bounds: RECT,
    hwnd: usize,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    // Cache the stable native canvas, including unopened detail areas. Returning
    // every source frame promptly keeps both pool buffers available to WGC.
    latest: Option<ID3D11Texture2D>,
    latest_valid: bool,
    dragging: bool,
    staging: [Option<(u32, u32, ID3D11Texture2D)>; 2],
    last_read: Instant,
    padding: u32,
    frame_token: Option<i64>,
    revision: u64,
    read_keys: [Option<(FrameKey, u64)>; 2],
}
impl Drop for MonitorSession {
    fn drop(&mut self) {
        if let Some(token) = self.frame_token.take() {
            if let Err(e) = self.pool.RemoveFrameArrived(token) {
                eprintln!("WGC frame handler: {e}");
            }
        }
        if let Err(e) = self.session.Close() {
            eprintln!("WGC session close: {e}");
        }
        if let Err(e) = self.pool.Close() {
            eprintln!("WGC pool close: {e}");
        }
        super::capture_log::log_event("WGC", "monitor session closed");
    }
}
impl MonitorSession {
    fn new(
        monitor: HMONITOR,
        bounds: RECT,
        capture_bounds: RECT,
        hwnd: usize,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> windows::core::Result<Self> {
        let borderless = request_borderless()?;
        let mut device = None;
        let mut context = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
        }
        let device = device.unwrap();
        let context = context.unwrap();
        let capture_device: IDirect3DDevice =
            unsafe { CreateDirect3D11DeviceFromDXGIDevice(&device.cast::<IDXGIDevice>()?)? }
                .cast()?;
        let factory: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = unsafe { factory.CreateForMonitor(monitor)? };
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &capture_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            item.Size()?,
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        let mut result = Self {
            monitor,
            bounds,
            capture_bounds,
            hwnd,
            device,
            context,
            pool,
            session,
            latest: None,
            latest_valid: false,
            dragging: false,
            staging: [None, None],
            last_read: Instant::now(),
            padding: 0,
            frame_token: None,
            revision: 0,
            read_keys: [None, None],
        };
        let filter: DisplaySession = result.session.cast()?;
        let excluded: windows_collections::IIterable<WindowId> =
            vec![WindowId { Value: hwnd as u64 }].into();
        let mut iteration = 0;
        unsafe {
            (filter.vtable().set)(filter.as_raw(), excluded.as_raw(), &mut iteration).ok()?;
        }
        result.session.SetIsCursorCaptureEnabled(false)?;
        if borderless {
            result.session.SetIsBorderRequired(false)?;
        }
        result.frame_token = Some(result.pool.FrameArrived(
            &windows::Foundation::TypedEventHandler::new(move |_, _| {
                wake();
                Ok(())
            }),
        )?);
        result.session.StartCapture()?;
        super::capture_log::log_event(
            "WGC",
            "monitor session started (per-session exclusion; affinity unchanged)",
        );
        Ok(result)
    }
    fn pixels(
        &mut self,
        key: FrameKey,
        crop: D3D11_BOX,
        slot: usize,
        requested: Option<FrameKey>,
    ) -> windows::core::Result<Option<Vec<u8>>> {
        self.last_read = Instant::now();
        loop {
            self.drain_frames()?;
            if self.latest_valid {
                break;
            }
            // A moved ROI cannot read pixels from the previous origin. If the
            // pool has no new frame, retry on the next bounded renderer tick.
            if self.latest.is_some() {
                return Ok(None);
            }
            if self.last_read.elapsed() >= FIRST_FRAME_WAIT {
                return Err(windows::core::Error::new(
                    windows::core::HRESULT(0x800705B4u32 as i32),
                    "WGC first frame timeout",
                ));
            }
            std::thread::sleep(FRAME_POLL);
        }
        if requested.is_some_and(|key| self.read_keys[slot] == Some((key, self.revision))) {
            return Ok(None);
        }
        let texture = self.latest.as_ref().unwrap();
        let surface = &mut self.staging[slot];
        if surface
            .as_ref()
            .is_none_or(|(w, h, _)| *w != key.width || *h != key.height)
        {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: key.width,
                Height: key.height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                ..Default::default()
            };
            let mut staging = None;
            unsafe {
                self.device
                    .CreateTexture2D(&desc, None, Some(&mut staging))?;
            }
            *surface = Some((key.width, key.height, staging.unwrap()));
        }
        let staging = &surface.as_ref().unwrap().2;
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .CopySubresourceRegion(staging, 0, 0, 0, 0, texture, 0, Some(&crop));
            self.context
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        }
        let row_bytes = key.width as usize * 4;
        let mut pixels = vec![0; row_bytes * key.height as usize];
        for (row, dst) in pixels.chunks_exact_mut(row_bytes).enumerate() {
            unsafe {
                dst.copy_from_slice(std::slice::from_raw_parts(
                    (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                    row_bytes,
                ));
            }
        }
        unsafe {
            self.context.Unmap(staging, 0);
        }
        if let Some(key) = requested {
            self.read_keys[slot] = Some((key, self.revision));
        }
        Ok(Some(pixels))
    }
    fn drain_frames(&mut self) -> windows::core::Result<bool> {
        let mut relevant = false;
        // Bound work to pool capacity. Frames are returned even when copying fails.
        for _ in 0..2 {
            match self.pool.TryGetNextFrame() {
                Ok(frame) => {
                    let result = (|| {
                        let dirty = match frame.DirtyRegions() {
                            Ok(regions) => Some(regions.into_iter().collect::<Vec<_>>()),
                            Err(error)
                                if error.code() == windows::Win32::Foundation::E_NOINTERFACE =>
                            {
                                None
                            }
                            Err(error) => return Err(error),
                        };
                        let touches = |rect| {
                            dirty.as_ref().is_none_or(|regions| {
                                regions
                                    .iter()
                                    .any(|r| dirty_overlaps(*r, self.bounds, rect))
                            })
                        };
                        let changed = self.read_keys.iter().flatten().any(|(key, _)| {
                            touches(RECT {
                                left: key.x,
                                top: key.y,
                                right: key.x + key.width as i32,
                                bottom: key.y + key.height as i32,
                            })
                        });
                        if !self.latest_valid || touches(self.capture_bounds) {
                            self.copy_canvas(&frame)?;
                            self.revision = self.revision.wrapping_add(1);
                        }
                        Ok(changed)
                    })();
                    frame.Close()?;
                    relevant |= result?;
                }
                Err(error) if error.code().is_ok() => break,
                Err(error) => return Err(error),
            }
        }
        Ok(relevant)
    }
    fn set_capture_bounds(&mut self, next: RECT) {
        if self.capture_bounds == next {
            return;
        }
        // Keep the allocation until a source frame is available, even when the
        // size changes. An established session must retry, not re-enter the
        // first-frame timeout just because a drag rotates the rail.
        self.capture_bounds = next;
        self.latest_valid = false;
    }
    fn copy_canvas(&mut self, frame: &Direct3D11CaptureFrame) -> windows::core::Result<()> {
        let size = frame.ContentSize()?;
        let access: IDirect3DDxgiInterfaceAccess = frame.Surface()?.cast()?;
        let texture: ID3D11Texture2D = unsafe { access.GetInterface()? };
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            texture.GetDesc(&mut desc);
        }
        if size.Width != self.bounds.right - self.bounds.left
            || size.Height != self.bounds.bottom - self.bounds.top
            || desc.Width < size.Width as u32
            || desc.Height < size.Height as u32
        {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x8000000Bu32 as i32),
                "WGC monitor size changed",
            ));
        }
        let source = D3D11_BOX {
            left: (self.capture_bounds.left - self.bounds.left) as u32,
            top: (self.capture_bounds.top - self.bounds.top) as u32,
            right: (self.capture_bounds.right - self.bounds.left) as u32,
            bottom: (self.capture_bounds.bottom - self.bounds.top) as u32,
            front: 0,
            back: 1,
        };
        let needs_texture = self.latest.as_ref().is_none_or(|texture| {
            let mut target = D3D11_TEXTURE2D_DESC::default();
            unsafe {
                texture.GetDesc(&mut target);
            }
            target.Width != source.right - source.left
                || target.Height != source.bottom - source.top
        });
        if needs_texture {
            desc.Width = source.right - source.left;
            desc.Height = source.bottom - source.top;
            desc.BindFlags = 0;
            desc.MiscFlags = 0;
            let mut resized = None;
            unsafe {
                self.device
                    .CreateTexture2D(&desc, None, Some(&mut resized))?;
            }
            self.latest = resized;
        }
        unsafe {
            self.context.CopySubresourceRegion(
                self.latest.as_ref().unwrap(),
                0,
                0,
                0,
                0,
                &texture,
                0,
                Some(&source),
            );
        }
        self.latest_valid = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn padded_drag_at_monitor_edge_reuses_static_pixels_across_small_moves() {
        let monitor = RECT {
            left: 0,
            top: 0,
            right: 2560,
            bottom: 1440,
        };
        let padded = RECT {
            left: 2454,
            top: 358,
            right: 2602,
            bottom: 642,
        };
        let cached = drag_capture_bounds(padded, monitor, None);
        let moved = RECT {
            top: 378,
            bottom: 662,
            ..padded
        };
        assert_eq!(drag_capture_bounds(moved, monitor, Some(cached)), cached);
    }

    #[test]
    fn tall_detail_padding_at_two_hundred_percent_keeps_supported_content() {
        let key = FrameKey {
            x: 0,
            y: 0,
            width: 520,
            height: 1200,
            generation: 1,
        };
        let padded = super::super::liquid_capture::padded_key(key, 144).unwrap();
        assert_eq!((padded.width, padded.height), (808, 1488));
        let pixels = vec![0; padded.width as usize * padded.height as usize * 4];
        assert!(!FrameCache::default()
            .frame(padded, &pixels)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn padding_clips_only_at_monitor_edges_and_preserves_real_neighbours() {
        let original = FrameKey {
            x: -2,
            y: 1,
            width: 2,
            height: 2,
            generation: 3,
        };
        let requested = super::super::liquid_capture::padded_key(original, 1).unwrap();
        let bounds = RECT {
            left: -8,
            top: 0,
            right: 0,
            bottom: 8,
        };
        let sampled = clipped_key(requested, bounds).unwrap();
        assert_eq!(
            (sampled.x, sampled.y, sampled.width, sampled.height),
            (-3, 0, 3, 4)
        );
        let pixels: Vec<u8> = (0..12).flat_map(|n| [n, 0, 0, 255]).collect();
        let expanded = extend_monitor_edges(pixels, sampled, requested);
        assert_eq!(expanded.len(), 4 * 4 * 4);
        assert_eq!(
            expanded.chunks_exact(4).map(|p| p[0]).collect::<Vec<_>>(),
            vec![0, 1, 2, 2, 3, 4, 5, 5, 6, 7, 8, 8, 9, 10, 11, 11]
        );
        assert!(super::super::liquid_capture::padded_key(original, 513).is_err());
        assert!(super::super::liquid_capture::padded_key(
            FrameKey {
                width: u32::MAX,
                ..original
            },
            1
        )
        .is_err());
    }

    #[test]
    fn dirty_regions_use_monitor_coordinates_and_skip_unrelated_motion() {
        let bounds = RECT {
            left: -1024,
            top: -200,
            right: 0,
            bottom: 1080,
        };
        let region = RECT {
            left: -64,
            top: 400,
            right: 0,
            bottom: 600,
        };
        assert!(dirty_overlaps(
            windows::Graphics::RectInt32 {
                X: 960,
                Y: 600,
                Width: 64,
                Height: 1
            },
            bounds,
            region
        ));
        assert!(!dirty_overlaps(
            windows::Graphics::RectInt32 {
                X: 0,
                Y: 0,
                Width: 50,
                Height: 50
            },
            bounds,
            region
        ));
        assert!(!dirty_overlaps(
            windows::Graphics::RectInt32 {
                X: 960,
                Y: 800,
                Width: 64,
                Height: 10
            },
            bounds,
            region
        ));
    }

    #[test]
    fn release_always_uses_wgc_and_debug_requires_explicit_opt_out() {
        for value in [None, Some("0"), Some("1"), Some("invalid")] {
            assert!(default_enabled(false, value));
            assert_eq!(default_enabled(true, value), value != Some("0"));
        }
    }
    #[test]
    fn drag_cache_covers_small_moves_without_requiring_a_new_source_frame() {
        let monitor = RECT {
            left: -1024,
            top: 0,
            right: 0,
            bottom: 1280,
        };
        let rail = RECT {
            left: -64,
            top: 400,
            right: 0,
            bottom: 600,
        };
        let cached = drag_capture_bounds(rail, monitor, None);
        let moved = RECT {
            top: 450,
            bottom: 650,
            ..rail
        };
        assert_eq!(drag_capture_bounds(moved, monitor, Some(cached)), cached);
        assert!(cached.top < rail.top && cached.bottom > rail.bottom);
        assert_eq!(cached.right, monitor.right);
        let far = RECT {
            top: 800,
            bottom: 1000,
            ..rail
        };
        assert_ne!(drag_capture_bounds(far, monitor, Some(cached)), cached);
    }
    #[test]
    fn negative_monitor_origin_and_edges_are_checked() {
        let bounds = RECT {
            left: -1024,
            top: -200,
            right: 0,
            bottom: 1080,
        };
        let key = FrameKey {
            x: -1024,
            y: -200,
            width: 64,
            height: 72,
            generation: 1,
        };
        let crop = crop_region(key, bounds).unwrap();
        assert_eq!(
            (crop.left, crop.top, crop.right, crop.bottom),
            (0, 0, 64, 72)
        );
        assert!(crop_region(FrameKey { x: -32, ..key }, bounds).is_none());
        assert!(crop_region(
            FrameKey {
                width: u32::MAX,
                ..key
            },
            bounds
        )
        .is_none());
        assert!(crop_region(FrameKey { y: -201, ..key }, bounds).is_none());
    }
}

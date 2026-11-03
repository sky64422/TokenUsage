//! WGC backdrop capture. All COM/D3D resources stay on the capture worker.
use super::liquid_capture::{FrameCache, FrameKey};
use std::{
    sync::OnceLock,
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

#[derive(Default)]
pub struct WgcCapture {
    session: Option<MonitorSession>,
    caches: [FrameCache; 2],
}
impl WgcCapture {
    pub fn capture(
        &mut self,
        hwnd: usize,
        key: FrameKey,
        slot: Option<usize>,
        dragging: bool,
    ) -> Result<Vec<u8>, String> {
        self.read(hwnd, key, slot, dragging).map_err(|error| {
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
    ) -> Result<Vec<u8>, String> {
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
        let monitor = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONULL) };
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
        let Some(crop) = crop_region(key, capture_bounds) else {
            // A rail crossing a monitor seam can temporarily span two outputs.
            // Keep the last paint until it fits instead of stopping the renderer.
            if dragging {
                return Ok(Vec::new());
            }
            return Err("WGC region crosses canvas bounds".into());
        };
        if self
            .session
            .as_ref()
            .is_none_or(|s| s.monitor != monitor || s.hwnd != hwnd || s.bounds != bounds)
        {
            self.session = None;
            self.caches.iter_mut().for_each(FrameCache::clear);
            self.session = Some(
                MonitorSession::new(monitor, bounds, capture_bounds, hwnd)
                    .map_err(|e| e.to_string())?,
            );
        }
        let session = self.session.as_mut().unwrap();
        session.set_capture_bounds(capture_bounds);
        session.dragging = dragging;
        let Some(pixels) = session
            .pixels(key, crop, slot.unwrap_or(0))
            .map_err(|e| e.to_string())?
        else {
            return Ok(Vec::new());
        };
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

fn crop_region(key: FrameKey, bounds: RECT) -> Option<D3D11_BOX> {
    let x = key.x.checked_sub(bounds.left)?;
    let y = key.y.checked_sub(bounds.top)?;
    let right = x.checked_add_unsigned(key.width)?;
    let bottom = y.checked_add_unsigned(key.height)?;
    if key.width == 0
        || key.height == 0
        || u64::from(key.width) * u64::from(key.height) > super::liquid_capture::MAX_FRAME_PIXELS
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
}
impl Drop for MonitorSession {
    fn drop(&mut self) {
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
        let result = Self {
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
    ) -> windows::core::Result<Option<Vec<u8>>> {
        self.last_read = Instant::now();
        loop {
            // Drain at most the pool capacity; do not starve the request on a busy desktop.
            for _ in 0..2 {
                match self.pool.TryGetNextFrame() {
                    Ok(frame) => {
                        let copied = self.copy_canvas(&frame);
                        frame.Close()?;
                        copied?;
                    }
                    // windows-core 0.61 Type::from_abi maps S_OK + null to Error::empty().
                    Err(error) if error.code().is_ok() => break,
                    Err(error) => return Err(error),
                }
            }
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
        Ok(Some(pixels))
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

//! In-memory Windows backdrop capture, enabled only by the Liquid theme.
//! Capture -> web lens follows https://github.com/molian313/dynamic-island;
//! this probe uses raw BMP IPC instead of its JPEG/base64 dependencies.

use crate::domain::types::AppTheme;
use crate::state::AppHandleState;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{Manager, State, WebviewWindow};

const MAX_CAPTURE_PIXELS: u64 = 1_048_576;
const CAPTURE_LEASE: Duration = Duration::from_millis(750);
const LEASE_CHECK: Duration = Duration::from_millis(250);

#[derive(Default)]
pub struct LiquidBackdrop(Mutex<BackdropState>);

#[derive(Default)]
struct BackdropState {
    lease: Option<Lease>,
    watching: bool,
}
struct Lease {
    hwnd: usize,
    original: u32,
    renewed: Instant,
}

/// Navigation restores normal screen sharing even if the previous renderer died.
pub fn reset_on_load(view: &tauri::Webview) {
    if let Some(window) = view.get_webview_window("main") {
        if let Err(error) = release_liquid_backdrop(window) {
            eprintln!("Liquid backdrop reset: {error}");
        }
    }
}

#[tauri::command]
pub fn release_liquid_backdrop(window: WebviewWindow) -> Result<(), String> {
    if let Some(state) = window.try_state::<LiquidBackdrop>() {
        let mut guard = state.0.lock().map_err(|e| e.to_string())?;
        if let Some(lease) = &guard.lease {
            set_affinity(lease.hwnd, lease.original)?;
            guard.lease = None;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn set_affinity(hwnd: usize, value: u32) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, WINDOW_DISPLAY_AFFINITY,
    };
    unsafe { SetWindowDisplayAffinity(HWND(hwnd as *mut _), WINDOW_DISPLAY_AFFINITY(value)) }
        .map_err(|e| format!("Liquid capture exclusion: {e}"))
}
#[cfg(not(windows))]
fn set_affinity(_: usize, _: u32) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn live_window(hwnd: usize) -> bool {
    use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsWindow};
    unsafe { IsWindow(Some(HWND(hwnd as *mut _))).as_bool() }
}
#[cfg(not(windows))]
fn live_window(_: usize) -> bool { false }

fn watch_lease(window: WebviewWindow) {
    std::thread::spawn(move || loop {
        std::thread::sleep(LEASE_CHECK);
        let state = window.state::<LiquidBackdrop>();
        let mut guard = state.0.lock().unwrap_or_else(|p| p.into_inner());
        if guard
            .lease
            .as_ref()
            .is_some_and(|lease| lease.renewed.elapsed() < CAPTURE_LEASE)
        {
            continue;
        }
        if let Some(lease) = &guard.lease {
            if let Err(error) = set_affinity(lease.hwnd, lease.original) {
                window.state::<AppHandleState>().core.note_diag(error);
                // Preserve the original affinity on a live HWND so a retry cannot
                // accidentally treat our own exclusion as its original state.
                if live_window(lease.hwnd) { continue; }
            }
            guard.lease = None;
        }
        guard.watching = false;
        break;
    });
}

fn valid_region(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    canvas_width: u32,
    canvas_height: u32,
) -> Result<bool, String> {
    let (Some(right), Some(bottom)) = (x.checked_add(width), y.checked_add(height)) else {
        return Err("Liquid capture region overflow".into());
    };
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_CAPTURE_PIXELS {
        return Err("Invalid Liquid capture dimensions".into());
    }
    Ok(right <= canvas_width && bottom <= canvas_height)
}

#[tauri::command]
pub fn capture_liquid_backdrop(
    window: WebviewWindow,
    state: State<'_, AppHandleState>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<tauri::ipc::Response, String> {
    if state.core.get_state().settings.theme != AppTheme::Liquid {
        return Err("Desktop capture requires the Liquid theme".into());
    }
    let size = window.inner_size().map_err(|e| e.to_string())?;
    if !valid_region(x, y, width, height, size.width, size.height)? {
        // Native canvas placement precedes the matching frontend layout event.
        // An empty frame explicitly skips a stale rectangle without stopping capture.
        return Ok(tauri::ipc::Response::new(Vec::<u8>::new()));
    }
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::DwmFlush;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
        };
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let pos = window.inner_position().map_err(|e| e.to_string())?;
        // Resolve Tauri getters before the mutex: getters can marshal to the UI thread.
        let backdrop = window.state::<LiquidBackdrop>();
        let mut guard = backdrop.0.lock().map_err(|e| e.to_string())?;
        if guard.lease.is_none() {
            let mut original = 0;
            unsafe {
                GetWindowDisplayAffinity(HWND(hwnd.0), &mut original).map_err(|e| e.to_string())?;
            }
            set_affinity(hwnd.0 as usize, WDA_EXCLUDEFROMCAPTURE.0)
                .inspect_err(|e| state.core.note_diag(e.clone()))?;
            guard.lease = Some(Lease {
                hwnd: hwnd.0 as usize,
                original,
                renewed: Instant::now(),
            });
            if !guard.watching {
                guard.watching = true;
                watch_lease(window.clone());
            }
            // The first read must not sample a composition that still includes our foreground.
            unsafe {
                DwmFlush().map_err(|e| e.to_string())?;
            }
        }
        guard.lease.as_mut().unwrap().renewed = Instant::now();
        // Capture coordinates remain signed for displays left/above the primary.
        let bytes = unsafe { capture(pos.x + x as i32, pos.y + y as i32, width, height) }
            .inspect_err(|e| state.core.note_diag(e.clone()))?;
        Ok(tauri::ipc::Response::new(bytes))
    }
    #[cfg(not(windows))]
    Err("Live Liquid backdrop requires Windows".into())
}

#[cfg(test)]
mod tests {
    use super::valid_region;
    #[test]
    fn bounds_reject_zero_overflow_and_unbounded_pixel_allocations() {
        assert_eq!(valid_region(268, 372, 56, 376, 324, 1120), Ok(true));
        assert_eq!(valid_region(300, 0, 56, 1, 324, 1120), Ok(false));
        for (x, y, w, h) in [
            (0, 0, 0, 1),
            (u32::MAX, 0, 1, 1),
            (0, 0, 2048, 2048),
        ] {
            assert!(valid_region(x, y, w, h, 324, 1120).is_err());
        }
    }
}

#[cfg(windows)]
unsafe fn capture(x: i32, y: i32, width: u32, height: u32) -> Result<Vec<u8>, String> {
    use windows::Win32::Graphics::Gdi::*;

    // Always release GDI resources, including on a failed blit/read.
    struct CaptureResources {
        desktop: HDC,
        memory: HDC,
        bitmap: HBITMAP,
        previous: HGDIOBJ,
    }
    impl Drop for CaptureResources {
        fn drop(&mut self) {
            unsafe {
                if !self.previous.0.is_null() {
                    SelectObject(self.memory, self.previous);
                }
                if !self.bitmap.0.is_null() {
                    let _ = DeleteObject(self.bitmap.into());
                }
                if !self.memory.0.is_null() {
                    let _ = DeleteDC(self.memory);
                }
                if !self.desktop.0.is_null() {
                    ReleaseDC(None, self.desktop);
                }
            }
        }
    }
    let mut resources = CaptureResources {
        desktop: GetDC(None),
        memory: HDC::default(),
        bitmap: HBITMAP::default(),
        previous: HGDIOBJ::default(),
    };
    if resources.desktop.0.is_null() {
        return Err("Glass GetDC failed".into());
    }
    resources.memory = CreateCompatibleDC(Some(resources.desktop));
    if resources.memory.0.is_null() {
        return Err("Glass CreateCompatibleDC failed".into());
    }
    resources.bitmap = CreateCompatibleBitmap(resources.desktop, width as i32, height as i32);
    if resources.bitmap.0.is_null() {
        return Err("Glass CreateCompatibleBitmap failed".into());
    }
    resources.previous = SelectObject(resources.memory, resources.bitmap.into());
    if resources.previous.0.is_null() || resources.previous.0 as isize == -1 {
        resources.previous = HGDIOBJ::default();
        return Err("Glass SelectObject failed".into());
    }
    BitBlt(
        resources.memory,
        0,
        0,
        width as i32,
        height as i32,
        Some(resources.desktop),
        x,
        y,
        SRCCOPY,
    )
    .map_err(|e| format!("Glass BitBlt: {e}"))?;
    SelectObject(resources.memory, resources.previous);
    resources.previous = HGDIOBJ::default();
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        },
        ..Default::default()
    };
    const BMP_HEADER_SIZE: usize = 54;
    let pixel_bytes = (width * height * 4) as usize;
    let mut bytes = vec![0u8; BMP_HEADER_SIZE + pixel_bytes];
    if GetDIBits(
        resources.desktop,
        resources.bitmap,
        0,
        height,
        Some(bytes[BMP_HEADER_SIZE..].as_mut_ptr().cast()),
        &mut info,
        DIB_RGB_COLORS,
    ) != height as i32
    {
        return Err("Glass GetDIBits failed".into());
    }
    bytes[0..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&((BMP_HEADER_SIZE + pixel_bytes) as u32).to_le_bytes());
    bytes[10..14].copy_from_slice(&(BMP_HEADER_SIZE as u32).to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    bytes[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&32u16.to_le_bytes());
    Ok(bytes)
}

//! Window show/hide and geometry helpers.
//!
//! Tauri 2 has no native window opacity API. The frontend applies CSS via
//! `set_opacity`. Geometry and always-on-top use native window APIs.

use tauri::{AppHandle, Manager, WebviewWindow};

pub fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "main window not found".into())
}

pub fn apply_always_on_top(window: &WebviewWindow, on_top: bool) -> Result<(), String> {
    window.set_always_on_top(on_top).map_err(|e| e.to_string())
}

pub fn show_window(window: &WebviewWindow) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

pub fn hide_window(window: &WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

/// Kill OS/CSS fringe outside the rounded glass (Windows WebView2).
/// Note: SetWindowRgn clips break DWM transparency — use DWM corner prefs only.
pub fn apply_clean_glass_edge(window: &WebviewWindow) -> Result<(), String> {
    use tauri::window::Color;
    // Fully transparent surface; avoids default white HWND fill in corners.
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    // OS drop-shadow on transparent windows draws a light ring outside CSS radius.
    let _ = window.set_shadow(false);
    #[cfg(windows)]
    apply_custom_notch_corners(window)?;
    Ok(())
}

/// The SVG notch owns its concave corners; DWM rounding would cut off its shoulders.
#[cfg(windows)]
fn apply_custom_notch_corners(window: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
        DWM_WINDOW_CORNER_PREFERENCE,
    };

    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let pref = DWMWCP_DONOTROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            HWND(hwnd.0),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const DWM_WINDOW_CORNER_PREFERENCE as *const _,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
    }
    Ok(())
}

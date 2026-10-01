//! Owns physical geometry and transparent input; frontend never moves the HWND.
use crate::{domain::notch::*, state::AppHandleState};
use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Emitter, Manager};
#[cfg(not(windows))]
use tauri::{PhysicalPosition, PhysicalSize};

const POINTER_INTERVAL: Duration = Duration::from_millis(32);
const DISPLAY_CHECK_TICKS: usize = 32;
#[derive(Default)]
pub struct NotchController(Mutex<Runtime>);
#[derive(Default)]
struct Runtime {
    revision: u64,
    expanded: bool,
    height: f64,
    layout: Option<NotchLayout>,
    ignoring: bool,
    preview: Option<NotchPlacement>,
}

pub fn preview(app: &AppHandle, placement: Option<NotchPlacement>) -> Result<NotchLayout, String> {
    if let Some(p) = &placement {
        p.validate()?;
    }
    app.state::<NotchController>()
        .0
        .lock()
        .map_err(|_| "Notch lock poisoned")?
        .preview = placement;
    apply(app, None)
}

pub fn monitors(app: &AppHandle) -> Result<Vec<MonitorArea>, String> {
    let primary = app.primary_monitor().map_err(|e| e.to_string())?;
    let mut monitors = app.available_monitors().map_err(|e| e.to_string())?;
    if let Some(p) = primary {
        monitors.sort_by_key(|m| m.position() != p.position());
    }
    Ok(monitors
        .into_iter()
        .map(|m| {
            let pos = m.position();
            let size = m.size();
            let work = m.work_area();
            MonitorArea {
                name: m
                    .name()
                    .cloned()
                    .unwrap_or_else(|| format!("{},{}", pos.x, pos.y)),
                bounds: Rect {
                    x: pos.x as f64,
                    y: pos.y as f64,
                    width: size.width as f64,
                    height: size.height as f64,
                },
                work: Rect {
                    x: work.position.x as f64,
                    y: work.position.y as f64,
                    width: work.size.width as f64,
                    height: work.size.height as f64,
                },
                scale: m.scale_factor(),
            }
        })
        .collect())
}

pub fn apply(app: &AppHandle, request: Option<(u64, bool, f64)>) -> Result<NotchLayout, String> {
    let controller = app.state::<NotchController>();
    let mut rt = controller.0.lock().map_err(|_| "Notch lock poisoned")?;
    if let Some((revision, expanded, height)) = request {
        if revision <= rt.revision {
            return rt.layout.clone().ok_or("Notch not ready".into());
        }
        if !height.is_finite() {
            return Err("Invalid detail height".into());
        }
        rt.revision = revision;
        rt.expanded = expanded;
        rt.height = height;
    }
    let state = app.state::<AppHandleState>();
    let settings = state.core.get_state().settings;
    let placement = rt.preview.as_ref().unwrap_or(&settings.notch);
    let areas = monitors(app)?;
    let monitor = areas
        .iter()
        .find(|m| Some(&m.name) == placement.monitor_hint.as_ref())
        .or(areas.first())
        .ok_or("No monitor available")?;
    let count = [
        settings.claude.enabled,
        settings.codex.enabled,
        settings.grok.enabled,
    ]
    .into_iter()
    .filter(|v| *v)
    .count();
    let layout = calculate_layout(
        monitor,
        placement,
        count,
        rt.expanded && state.core.is_visible(),
        rt.height,
    )?;
    let changed = rt.layout.as_ref() != Some(&layout);
    if changed {
        let win = super::window_ctl::main_window(app)?;
        #[cfg(windows)]
        {
            use windows::Win32::{
                Foundation::HWND,
                UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER},
            };
            let hwnd = win.hwnd().map_err(|e| e.to_string())?;
            // One native move+resize keeps the screen-edge contact fixed throughout.
            unsafe {
                SetWindowPos(
                    HWND(hwnd.0),
                    None,
                    layout.window.x as i32,
                    layout.window.y as i32,
                    layout.window.width as i32,
                    layout.window.height as i32,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )
                .map_err(|e| e.to_string())?;
            }
        }
        #[cfg(not(windows))]
        {
            win.set_size(PhysicalSize::new(
                layout.window.width as u32,
                layout.window.height as u32,
            ))
            .map_err(|e| e.to_string())?;
            win.set_position(PhysicalPosition::new(
                layout.window.x as i32,
                layout.window.y as i32,
            ))
            .map_err(|e| e.to_string())?;
        }
        app.emit("notch-layout", &layout)
            .map_err(|e| e.to_string())?;
        rt.layout = Some(layout.clone());
    }
    Ok(layout)
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut tick = 0;
        let mut last_error = String::new();
        loop {
            if tick % DISPLAY_CHECK_TICKS == 0 {
                let handle = app.clone();
                if let Err(e) = app.run_on_main_thread(move || {
                    if let Err(e) = apply(&handle, None) {
                        eprintln!("notch placement: {e}");
                    }
                }) {
                    eprintln!("notch dispatch: {e}");
                }
            }
            tick += 1;
            let result = (|| -> Result<(), String> {
                let win = super::window_ctl::main_window(&app)?;
                let pos = win.cursor_position().map_err(|e| e.to_string())?;
                let controller = app.state::<NotchController>();
                let (ignore, changed) = {
                    let rt = controller.0.lock().map_err(|_| "Notch lock poisoned")?;
                    let ignore = rt.preview.is_none()
                        && rt.layout.as_ref().is_none_or(|l| !l.hit(pos.x, pos.y));
                    (ignore, ignore != rt.ignoring)
                };
                if changed {
                    win.set_ignore_cursor_events(ignore)
                        .map_err(|e| e.to_string())?;
                    controller
                        .0
                        .lock()
                        .map_err(|_| "Notch lock poisoned")?
                        .ignoring = ignore;
                }
                Ok(())
            })();
            if let Err(e) = result {
                if e != last_error {
                    eprintln!("notch input: {e}");
                    last_error = e;
                }
            }
            std::thread::sleep(POINTER_INTERVAL);
        }
    });
}

//! Window-event hints for the GDI backdrop; the heartbeat covers pixel-only changes.
use super::liquid_capture::FrameKey;
use std::sync::Mutex;

static REGIONS: Mutex<[Option<FrameKey>; 2]> = Mutex::new([None, None]);

pub fn record_region(slot: usize, key: FrameKey) {
    if let Ok(mut regions) = REGIONS.lock() {
        regions[slot] = Some(key);
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{
        cell::RefCell,
        collections::HashMap,
        time::{Duration, Instant},
    };
    use tauri::{Emitter, Manager};
    use windows::Win32::{
        Foundation::HWND,
        UI::{Accessibility::*, WindowsAndMessaging::*},
    };

    const EVENT_INTERVAL: Duration = Duration::from_millis(33);
    const MAX_TRACKED_WINDOWS: usize = 512;
    struct Watcher {
        app: tauri::AppHandle,
        previous: HashMap<isize, windows::Win32::Foundation::RECT>,
        last_emit: Option<Instant>,
        timer: usize,
    }
    thread_local! { static WATCHER: RefCell<Option<Watcher>> = const { RefCell::new(None) }; }

    fn emit(watcher: &mut Watcher) {
        watcher.last_emit = Some(Instant::now());
        if let Err(error) = watcher.app.emit("liquid-backdrop-changed", ()) {
            eprintln!("Liquid window event: {error}");
        }
    }

    unsafe extern "system" fn trailing(_: HWND, _: u32, id: usize, _: u32) {
        let _ = KillTimer(None, id);
        WATCHER.with(|cell| {
            if let Some(watcher) = cell.borrow_mut().as_mut() {
                watcher.timer = 0;
                emit(watcher);
            }
        });
    }

    fn overlaps(a: windows::Win32::Foundation::RECT, b: FrameKey) -> bool {
        i64::from(a.left) < i64::from(b.x) + i64::from(b.width)
            && i64::from(a.right) > i64::from(b.x)
            && i64::from(a.top) < i64::from(b.y) + i64::from(b.height)
            && i64::from(a.bottom) > i64::from(b.y)
    }

    unsafe extern "system" fn changed(
        _: HWINEVENTHOOK,
        event: u32,
        hwnd: HWND,
        object: i32,
        child: i32,
        _: u32,
        _: u32,
    ) {
        if hwnd.0.is_null() || object != OBJID_WINDOW.0 || child != 0 {
            return;
        }
        if !matches!(
            event,
            EVENT_SYSTEM_FOREGROUND
                | EVENT_OBJECT_SHOW
                | EVENT_OBJECT_HIDE
                | EVENT_OBJECT_DESTROY
                | EVENT_OBJECT_LOCATIONCHANGE
        ) {
            return;
        }
        if event != EVENT_OBJECT_DESTROY && GetAncestor(hwnd, GA_ROOT) != hwnd {
            return;
        }
        WATCHER.with(|cell| {
            let Ok(mut guard) = cell.try_borrow_mut() else {
                return;
            };
            let Some(watcher) = guard.as_mut() else {
                return;
            };
            let mut rect = windows::Win32::Foundation::RECT::default();
            let current = (event != EVENT_OBJECT_DESTROY && GetWindowRect(hwnd, &mut rect).is_ok())
                .then_some(rect);
            let old = watcher.previous.remove(&(hwnd.0 as isize));
            if let Some(rect) = current {
                if watcher.previous.len() >= MAX_TRACKED_WINDOWS {
                    watcher.previous.clear();
                }
                watcher.previous.insert(hwnd.0 as isize, rect);
            }
            let state = watcher.app.state::<crate::state::AppHandleState>();
            if !state.core.is_visible()
                || !matches!(
                    state.core.get_state().settings.theme,
                    crate::domain::types::AppTheme::Glass | crate::domain::types::AppTheme::Liquid
                )
            {
                return;
            }
            let Ok(regions) = REGIONS.lock() else {
                return;
            };
            // The first move may already have left the sampled region. Be conservative
            // until this HWND has a previous rectangle, then filter both positions.
            let relevant = old.is_none()
                || regions.iter().flatten().any(|key| {
                    old.is_some_and(|r| overlaps(r, *key))
                        || current.is_some_and(|r| overlaps(r, *key))
                });
            drop(regions);
            if !relevant || watcher.timer != 0 {
                return;
            }
            if let Some(remaining) = watcher
                .last_emit
                .and_then(|t| EVENT_INTERVAL.checked_sub(t.elapsed()))
            {
                watcher.timer = SetTimer(None, 0, remaining.as_millis() as u32 + 1, Some(trailing));
                if watcher.timer != 0 {
                    return;
                }
            }
            emit(watcher);
        });
    }

    pub fn start(app: tauri::AppHandle) {
        // Out-of-context hooks require a message loop on their installing thread.
        if let Err(error) = std::thread::Builder::new()
            .name("liquid-events".into())
            .spawn(move || unsafe {
                WATCHER.with(|cell| {
                    *cell.borrow_mut() = Some(Watcher {
                        app,
                        previous: HashMap::new(),
                        last_emit: None,
                        timer: 0,
                    })
                });
                let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
                let hooks = [
                    SetWinEventHook(
                        EVENT_SYSTEM_FOREGROUND,
                        EVENT_SYSTEM_FOREGROUND,
                        None,
                        Some(changed),
                        0,
                        0,
                        flags,
                    ),
                    SetWinEventHook(
                        EVENT_OBJECT_DESTROY,
                        EVENT_OBJECT_LOCATIONCHANGE,
                        None,
                        Some(changed),
                        0,
                        0,
                        flags,
                    ),
                ];
                if hooks.iter().any(|h| h.0.is_null()) {
                    eprintln!("Liquid window hooks unavailable; heartbeat capture remains active");
                } else {
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                for hook in hooks {
                    if !hook.0.is_null() {
                        let _ = UnhookWinEvent(hook);
                    }
                }
                WATCHER.with(|cell| *cell.borrow_mut() = None);
            })
        {
            eprintln!("Liquid window watcher: {error}");
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn overlap_handles_negative_monitors_and_excludes_touching_edges() {
            let key = FrameKey {
                x: -100,
                y: 20,
                width: 64,
                height: 100,
                generation: 0,
            };
            let rect = |left, right| windows::Win32::Foundation::RECT {
                left,
                right,
                top: 30,
                bottom: 60,
            };
            assert!(overlaps(rect(-80, -40), key));
            assert!(!overlaps(rect(-36, 10), key));
            assert!(!overlaps(rect(-200, -100), key));
        }
    }
}

#[cfg(windows)]
pub use platform::start;
#[cfg(not(windows))]
pub fn start(_: tauri::AppHandle) {}

//! Bounded capture buffers. GDI handles are created, used and dropped on one worker thread.
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use tokio::sync::oneshot;

const FRAME_HEADER: usize = 12;
pub(super) const MAX_FRAME_PIXELS: u64 = 1_048_576;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameKey {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub generation: u64,
}

#[derive(Default)]
pub struct FrameCache {
    key: Option<FrameKey>,
    bytes: Vec<u8>,
    source: Vec<u8>,
}
impl FrameCache {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn frame(&mut self, key: FrameKey, bgra: &[u8]) -> Result<Vec<u8>, String> {
        let count = u64::from(key.width) * u64::from(key.height);
        if count == 0 || count > MAX_FRAME_PIXELS || bgra.len() != count as usize * 4 {
            return Err("Invalid Liquid pixel buffer".into());
        }
        // Most idle desktops are byte-identical. Compare in bulk before converting pixels.
        // Keep the alpha-insensitive conversion below for GDI's reserved-alpha changes.
        if self.key == Some(key) && self.source == bgra {
            return Ok(Vec::new());
        }
        let mut changed = self.key != Some(key);
        self.bytes.resize(FRAME_HEADER + bgra.len(), 0);
        self.bytes[..4].copy_from_slice(b"LQ01");
        self.bytes[4..8].copy_from_slice(&key.width.to_le_bytes());
        self.bytes[8..12].copy_from_slice(&key.height.to_le_bytes());
        // Ignore GDI's reserved alpha byte; desktop samples are opaque.
        for (src, dst) in bgra
            .chunks_exact(4)
            .zip(self.bytes[FRAME_HEADER..].chunks_exact_mut(4))
        {
            let rgba = [src[2], src[1], src[0], 255];
            changed |= dst != rgba;
            dst.copy_from_slice(&rgba);
        }
        self.key = Some(key);
        self.source.clear();
        self.source.extend_from_slice(bgra);
        Ok(if changed {
            self.bytes.clone()
        } else {
            Vec::new()
        })
    }
}

struct Job {
    epoch: u64,
    hwnd: usize,
    key: FrameKey,
    surface: Option<usize>,
    dragging: bool,
    reply: oneshot::Sender<Result<Vec<u8>, String>>,
}
enum Message {
    Capture(Job),
    Reset,
}
#[derive(Default)]
pub struct CaptureWorker {
    sender: Mutex<Option<mpsc::SyncSender<Message>>>,
    epoch: Arc<AtomicU64>,
}
impl CaptureWorker {
    fn sender(&self) -> Result<mpsc::SyncSender<Message>, String> {
        let mut sender = self.sender.lock().map_err(|e| e.to_string())?;
        if let Some(sender) = &*sender {
            return Ok(sender.clone());
        }
        let (tx, rx) = mpsc::sync_channel(2);
        let epoch = self.epoch.clone();
        std::thread::Builder::new()
            .name("liquid-capture".into())
            .spawn(move || {
                let mut applied_epoch = epoch.load(Ordering::SeqCst);
                #[cfg(windows)]
                let mut surfaces: [Option<Surface>; 2] = [None, None];
                #[cfg(windows)]
                let mut wgc = super::liquid_wgc::WgcCapture::default();
                #[cfg(windows)]
                let apartment = if super::liquid_wgc::enabled() {
                    unsafe {
                        windows::Win32::System::WinRT::RoInitialize(
                            windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
                        )
                    }
                } else {
                    Ok(())
                };
                loop {
                    #[cfg(windows)]
                    let timed = super::liquid_wgc::enabled();
                    #[cfg(not(windows))]
                    let timed = false;
                    let received = if timed {
                        rx.recv_timeout(std::time::Duration::from_secs(1))
                    } else {
                        rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                    };
                    let message = match received {
                        Ok(message) => message,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            #[cfg(windows)]
                            wgc.expire_idle();
                            continue;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let current_epoch = epoch.load(Ordering::SeqCst);
                    if current_epoch != applied_epoch {
                        #[cfg(windows)]
                        {
                            surfaces = [None, None];
                            wgc = Default::default();
                        }
                        applied_epoch = current_epoch;
                    }
                    match message {
                        Message::Reset => {}
                        Message::Capture(job) => {
                            if job.epoch != current_epoch {
                                // Cancellation is not a capture failure: the current renderer retries.
                                let _ = job.reply.send(Ok(Vec::new()));
                                continue;
                            }
                            #[cfg(windows)]
                            let result = if super::liquid_wgc::enabled() {
                                match &apartment {
                                    Ok(()) => {
                                        wgc.capture(job.hwnd, job.key, job.surface, job.dragging)
                                    }
                                    Err(error) => Err(format!("WGC apartment: {error}")),
                                }
                            } else {
                                capture(&mut surfaces, job.key, job.surface)
                            };
                            #[cfg(not(windows))]
                            let result = {
                                let _ = (job.key, job.surface);
                                Err("Live Liquid backdrop requires Windows".into())
                            };
                            let _ = job.reply.send(result);
                        }
                    }
                }
                #[cfg(windows)]
                {
                    drop(wgc);
                    if super::liquid_wgc::enabled() && apartment.is_ok() {
                        unsafe {
                            windows::Win32::System::WinRT::RoUninitialize();
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        *sender = Some(tx.clone());
        Ok(tx)
    }
    pub async fn frame(
        &self,
        hwnd: usize,
        key: FrameKey,
        surface: Option<u8>,
        dragging: bool,
    ) -> Result<Vec<u8>, String> {
        if surface.is_some_and(|s| s > 1) {
            return Err("Invalid Liquid surface".into());
        }
        let sender = self.sender()?;
        let (reply, result) = oneshot::channel();
        match sender.try_send(Message::Capture(Job {
            epoch: self.epoch.load(Ordering::SeqCst),
            hwnd,
            key,
            surface: surface.map(usize::from),
            dragging,
            reply,
        })) {
            Ok(()) => {}
            // A transient backlog is not a failed capture backend. Keep the last
            // painted frame and let the bounded renderer schedule its next read.
            Err(mpsc::TrySendError::Full(_)) => return Ok(Vec::new()),
            Err(error) => return Err(format!("Liquid capture queue: {error}")),
        }
        result
            .await
            .map_err(|e| format!("Liquid capture worker: {e}"))?
    }
    pub fn reset(&self) -> Result<(), String> {
        let sender = self.sender.lock().map_err(|e| e.to_string())?;
        self.epoch.fetch_add(1, Ordering::SeqCst);
        if let Some(sender) = &*sender {
            match sender.try_send(Message::Reset) {
                Ok(()) | Err(mpsc::TrySendError::Full(_)) => {} // Queued work will observe the epoch before reading.
                Err(error) => return Err(format!("Liquid reset queue: {error}")),
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
use windows::Win32::Graphics::Gdi::*;
#[cfg(windows)]
struct Surface {
    desktop: HDC,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    pixels: *mut u8,
    width: u32,
    height: u32,
    cache: FrameCache,
}
#[cfg(windows)]
impl Drop for Surface {
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
#[cfg(windows)]
impl Surface {
    unsafe fn new(width: u32, height: u32) -> Result<Self, String> {
        let mut surface = Self {
            desktop: GetDC(None),
            memory: HDC::default(),
            bitmap: HBITMAP::default(),
            previous: HGDIOBJ::default(),
            pixels: std::ptr::null_mut(),
            width,
            height,
            cache: FrameCache::default(),
        };
        if surface.desktop.0.is_null() {
            return Err("Liquid GetDC failed".into());
        }
        surface.memory = CreateCompatibleDC(Some(surface.desktop));
        if surface.memory.0.is_null() {
            return Err("Liquid CreateCompatibleDC failed".into());
        }
        let info = BITMAPINFO {
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
        let mut pixels = std::ptr::null_mut();
        surface.bitmap = CreateDIBSection(
            Some(surface.desktop),
            &info,
            DIB_RGB_COLORS,
            &mut pixels,
            None,
            0,
        )
        .map_err(|e| format!("Liquid CreateDIBSection: {e}"))?;
        surface.pixels = pixels.cast();
        surface.previous = SelectObject(surface.memory, surface.bitmap.into());
        if surface.previous.0.is_null() || surface.previous.0 as isize == -1 {
            surface.previous = HGDIOBJ::default();
            return Err("Liquid SelectObject failed".into());
        }
        Ok(surface)
    }
    unsafe fn read(&mut self, key: FrameKey, raw: bool) -> Result<Vec<u8>, String> {
        BitBlt(
            self.memory,
            0,
            0,
            self.width as i32,
            self.height as i32,
            Some(self.desktop),
            key.x,
            key.y,
            SRCCOPY,
        )
        .map_err(|e| format!("Liquid BitBlt: {e}"))?;
        if !GdiFlush().as_bool() {
            return Err("Liquid GdiFlush failed".into());
        }
        let pixels =
            std::slice::from_raw_parts(self.pixels, (self.width * self.height * 4) as usize);
        if raw {
            return self.cache.frame(key, pixels);
        }
        Ok(bmp_frame(self.width, self.height, pixels))
    }
}

// Preserve BMP for existing native diagnostics and older callers on both backends.
pub(super) fn bmp_frame(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    const BMP_HEADER: usize = 54;
    let mut bytes = vec![0u8; BMP_HEADER + pixels.len()];
    bytes[..2].copy_from_slice(b"BM");
    let len = bytes.len() as u32;
    bytes[2..6].copy_from_slice(&len.to_le_bytes());
    bytes[10..14].copy_from_slice(&(BMP_HEADER as u32).to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    bytes[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&32u16.to_le_bytes());
    bytes[BMP_HEADER..].copy_from_slice(pixels);
    bytes
}
#[cfg(windows)]
fn capture(
    surfaces: &mut [Option<Surface>; 2],
    key: FrameKey,
    slot: Option<usize>,
) -> Result<Vec<u8>, String> {
    let count = u64::from(key.width) * u64::from(key.height);
    if count == 0 || count > MAX_FRAME_PIXELS {
        return Err("Invalid Liquid capture dimensions".into());
    }
    if let Some(slot) = slot {
        super::liquid_events::record_region(slot, key);
    }
    let surface = &mut surfaces[slot.unwrap_or(0)];
    if surface
        .as_ref()
        .is_none_or(|s| s.width != key.width || s.height != key.height)
    {
        *surface = Some(unsafe { Surface::new(key.width, key.height)? });
    }
    unsafe { surface.as_mut().unwrap().read(key, slot.is_some()) }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[tokio::test]
    async fn capture_congestion_returns_no_frame_but_disconnect_is_an_error() {
        let (sender, receiver) = mpsc::sync_channel(2);
        let worker = CaptureWorker {
            sender: Mutex::new(Some(sender)),
            epoch: Arc::new(AtomicU64::new(0)),
        };
        worker.reset().unwrap();
        worker.reset().unwrap();
        let key = FrameKey {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
            generation: 1,
        };
        assert_eq!(worker.frame(0, key, Some(0), false).await, Ok(Vec::new()));
        drop(receiver);
        assert!(worker.frame(0, key, Some(0), false).await.is_err());
    }
    #[test]
    fn repeated_reset_is_not_lost_when_worker_queue_is_full() {
        let (sender, receiver) = mpsc::sync_channel(2);
        let worker = CaptureWorker {
            sender: Mutex::new(Some(sender)),
            epoch: Arc::new(AtomicU64::new(0)),
        };
        for _ in 0..5 {
            worker.reset().unwrap();
        }
        assert_eq!(worker.epoch.load(Ordering::SeqCst), 5);
        assert!(matches!(receiver.try_recv(), Ok(Message::Reset)));
        assert!(matches!(receiver.try_recv(), Ok(Message::Reset)));
        assert!(receiver.try_recv().is_err());
    }
}

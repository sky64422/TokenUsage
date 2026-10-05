//! Physical screen placement; no OS calls, persistence, or vendor policy.
use serde::{Deserialize, Serialize};

pub const DEPTH: f64 = 64.;
pub const CELL: f64 = 72.;
pub const SHOULDER: f64 = DEPTH / 2.;
pub const INNER_RADIUS: f64 = DEPTH - SHOULDER;
// Content nestles into the curved ends instead of starting after the entire curve.
pub const END_PADDING: f64 = 44.;
pub const DETAIL_RADIUS: f64 = 16.;
pub const DETAIL_WIDTH: f64 = 260.;
pub const DETAIL_HEIGHT: f64 = 560.;
pub const MIN_DETAIL_HEIGHT: f64 = 48.;
pub const GAP: f64 = 4.;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NotchEdge {
    Top,
    #[default]
    Right,
    Bottom,
    Left,
}
impl NotchEdge {
    pub fn vertical(self) -> bool {
        matches!(self, Self::Right | Self::Left)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotchPlacement {
    pub edge: NotchEdge,
    pub monitor_hint: Option<String>,
    pub offset: f64,
}
impl Default for NotchPlacement {
    fn default() -> Self {
        Self {
            edge: NotchEdge::Right,
            monitor_hint: None,
            offset: 0.5,
        }
    }
}
impl NotchPlacement {
    pub fn validate(&self) -> Result<(), String> {
        if !self.offset.is_finite() || !(0. ..=1.).contains(&self.offset) {
            return Err("Notch offset must be between 0 and 1".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
    fn union(self, r: Self) -> Self {
        let x = self.x.min(r.x);
        let y = self.y.min(r.y);
        Self {
            x,
            y,
            width: (self.x + self.width).max(r.x + r.width) - x,
            height: (self.y + self.height).max(r.y + r.height) - y,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MonitorArea {
    pub name: String,
    pub bounds: Rect,
    pub work: Rect,
    pub scale: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorX {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorY {
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotchLayout {
    pub edge: NotchEdge,
    pub anchor_x: AnchorX,
    pub anchor_y: AnchorY,
    pub notch: Rect,
    pub detail: Option<Rect>,
    pub window: Rect,
    pub scale: f64,
    pub monitor: String,
    pub metrics: NotchMetrics,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotchMetrics {
    pub depth: f64,
    pub cell: f64,
    pub shoulder: f64,
    pub inner_radius: f64,
    pub inset: f64,
    pub detail_radius: f64,
    pub drag_threshold: f64,
    pub rest_depth: f64,
    pub rest_length: f64,
}
impl NotchLayout {
    pub fn hit(&self, x: f64, y: f64) -> bool {
        if let Some(d) = self.detail {
            if d.contains(x, y) {
                let r = (DETAIL_RADIUS * self.scale)
                    .min(d.width / 2.)
                    .min(d.height / 2.);
                let dx = x - x.clamp(d.x + r, d.x + d.width - r);
                let dy = y - y.clamp(d.y + r, d.y + d.height - r);
                if dx * dx + dy * dy <= r * r {
                    return true;
                }
            }
            // A narrow bridge keeps hover alive while crossing to the detail.
            let n = self.notch;
            let bridge = if self.edge.vertical() {
                Rect {
                    x: n.x.min(d.x + d.width),
                    y: n.y.max(d.y),
                    width: GAP * self.scale,
                    height: (n.y + n.height).min(d.y + d.height) - n.y.max(d.y),
                }
            } else {
                Rect {
                    x: n.x.max(d.x),
                    y: n.y.min(d.y + d.height),
                    width: (n.x + n.width).min(d.x + d.width) - n.x.max(d.x),
                    height: GAP * self.scale,
                }
            };
            let bridge = match self.edge {
                NotchEdge::Left => Rect {
                    x: n.x + n.width,
                    ..bridge
                },
                NotchEdge::Top => Rect {
                    y: n.y + n.height,
                    ..bridge
                },
                _ => bridge,
            };
            if bridge.contains(x, y) {
                return true;
            }
        }
        if !self.notch.contains(x, y) {
            return false;
        }
        let n = self.notch;
        let s = self.scale;
        let (across, along, length) = match self.edge {
            NotchEdge::Right => ((x - n.x) / s, (y - n.y) / s, n.height / s),
            NotchEdge::Left => ((n.x + n.width - x) / s, (y - n.y) / s, n.height / s),
            NotchEdge::Bottom => ((y - n.y) / s, (x - n.x) / s, n.width / s),
            NotchEdge::Top => ((n.y + n.height - y) / s, (x - n.x) / s, n.width / s),
        };
        let end = along.min(length - along);
        let boundary = if end < SHOULDER {
            DEPTH - SHOULDER + (SHOULDER.powi(2) - end.powi(2)).max(0.).sqrt()
        } else if end < SHOULDER + INNER_RADIUS {
            INNER_RADIUS
                - (INNER_RADIUS.powi(2) - (end - SHOULDER - INNER_RADIUS).powi(2))
                    .max(0.)
                    .sqrt()
        } else {
            0.
        };
        across >= boundary
    }
}

pub fn calculate_layout(
    m: &MonitorArea,
    p: &NotchPlacement,
    count: usize,
    expanded: bool,
    detail_height: f64,
) -> Result<NotchLayout, String> {
    calculate_layout_target(m, p, count, expanded, detail_height, None)
}

pub fn calculate_layout_target(
    m: &MonitorArea,
    p: &NotchPlacement,
    count: usize,
    expanded: bool,
    detail_height: f64,
    target: Option<f64>,
) -> Result<NotchLayout, String> {
    p.validate()?;
    if !m.scale.is_finite() || m.scale <= 0. || count == 0 || count > super::types::ProviderId::all().len() {
        return Err("Invalid notch dimensions".into());
    }
    let b = m.bounds;
    let w = m.work;
    let s = m.scale;
    let available = |e| match e {
        NotchEdge::Right => w.x + w.width >= b.x + b.width,
        NotchEdge::Left => w.x <= b.x,
        NotchEdge::Top => w.y <= b.y,
        // User-selected bottom docking overlays the taskbar at the physical screen edge.
        NotchEdge::Bottom => true,
    };
    let edge = [
        p.edge,
        NotchEdge::Right,
        NotchEdge::Left,
        NotchEdge::Top,
        NotchEdge::Bottom,
    ]
    .into_iter()
    .find(|e| available(*e))
    .ok_or("No screen edge available")?;
    let len = ((count as f64 * CELL + END_PADDING * 2.) * s).round();
    let depth = (DEPTH * s).round();
    let span = if edge.vertical() { w.height } else { w.width };
    if len > span || depth > w.width.min(w.height) {
        return Err("Display is too small for the notch".into());
    }
    let offset = ((span - len) * p.offset).round();
    let notch = match edge {
        NotchEdge::Right => Rect {
            x: b.x + b.width - depth,
            y: w.y + offset,
            width: depth,
            height: len,
        },
        NotchEdge::Left => Rect {
            x: b.x,
            y: w.y + offset,
            width: depth,
            height: len,
        },
        NotchEdge::Top => Rect {
            x: w.x + offset,
            y: b.y,
            width: len,
            height: depth,
        },
        NotchEdge::Bottom => Rect {
            x: w.x + offset,
            y: b.y + b.height - depth,
            width: len,
            height: depth,
        },
    };
    let anchor_x = match edge {
        NotchEdge::Left => AnchorX::Left,
        NotchEdge::Right => AnchorX::Right,
        NotchEdge::Top | NotchEdge::Bottom => {
            if p.offset <= 0.5 {
                AnchorX::Left
            } else {
                AnchorX::Right
            }
        }
    };
    let anchor_y = match edge {
        NotchEdge::Top | NotchEdge::Left | NotchEdge::Right => AnchorY::Top,
        NotchEdge::Bottom => AnchorY::Bottom,
    };
    let detail = if expanded {
        let dw = (DETAIL_WIDTH * s)
            .ceil()
            .min(w.width - if edge.vertical() { depth + GAP * s } else { 0. });
        let dh = (detail_height.clamp(MIN_DETAIL_HEIGHT, DETAIL_HEIGHT) * s)
            .ceil()
            .min(w.height - if edge.vertical() { 0. } else { depth + GAP * s });
        if dw <= 0. || dh <= 0. {
            return Err("Display is too small for usage details".into());
        }
        let (dx, dy) = if edge.vertical() {
            let x = match edge {
                NotchEdge::Right => notch.x - GAP * s - dw,
                NotchEdge::Left => notch.x + depth + GAP * s,
                _ => unreachable!(),
            };
            let y = if let Some(t) = target {
                let center = notch.y + t * s;
                let ideal = center - dh / 2.0;
                if dh <= notch.height {
                    ideal.clamp(notch.y, notch.y + notch.height - dh)
                } else {
                    ideal.clamp(w.y, (w.y + w.height - dh).max(w.y))
                }
            } else {
                notch.y.clamp(w.y, (w.y + w.height - dh).max(w.y))
            };
            (x, y)
        } else {
            let y = match edge {
                NotchEdge::Top => notch.y + depth + GAP * s,
                NotchEdge::Bottom => notch.y - GAP * s - dh,
                _ => unreachable!(),
            };
            let x = if let Some(t) = target {
                let center = notch.x + t * s;
                let ideal = center - dw / 2.0;
                if dw <= notch.width {
                    ideal.clamp(notch.x, notch.x + notch.width - dw)
                } else {
                    ideal.clamp(w.x, (w.x + w.width - dw).max(w.x))
                }
            } else {
                match anchor_x {
                    AnchorX::Left => notch.x.clamp(w.x, (w.x + w.width - dw).max(w.x)),
                    AnchorX::Right => {
                        (notch.x + notch.width - dw).clamp(w.x, (w.x + w.width - dw).max(w.x))
                    }
                }
            };
            (x, y)
        };
        let d = Rect {
            x: dx.round(),
            y: dy.round(),
            width: dw,
            height: dh,
        };
        Some(d)
    } else {
        None
    };
    // A stable transparent canvas prevents HWND moves racing the WebView's local
    // notch offset when a taller provider card extends above the rail.
    let window = surface_canvas(notch, w, edge, s);
    Ok(NotchLayout {
        edge,
        anchor_x,
        anchor_y,
        notch,
        detail,
        window,
        scale: s,
        monitor: m.name.clone(),
        metrics: NotchMetrics {
            depth: DEPTH,
            cell: CELL,
            shoulder: SHOULDER,
            inner_radius: INNER_RADIUS,
            inset: END_PADDING,
            detail_radius: DETAIL_RADIUS,
            drag_threshold: DRAG_THRESHOLD,
            rest_depth: super::reveal::REST_DEPTH,
            rest_length: super::reveal::REST_LENGTH,
        },
    })
}

fn surface_canvas(notch: Rect, work: Rect, edge: NotchEdge, scale: f64) -> Rect {
    let gap = GAP * scale;
    let width = (DETAIL_WIDTH * scale).ceil()
        .min(work.width - if edge.vertical() { notch.width + gap } else { 0. });
    let height = (DETAIL_HEIGHT * scale).ceil()
        .min(work.height - if edge.vertical() { 0. } else { notch.height + gap });
    if width <= 0. || height <= 0. { return notch; }
    let reserve = if edge.vertical() {
        let top = (notch.y - height / 2.).floor().clamp(work.y, work.y + work.height - height);
        let bottom = (notch.y + notch.height + height).ceil().min(work.y + work.height);
        Rect {
            x: if edge == NotchEdge::Left { notch.x + notch.width + gap } else { notch.x - gap - width },
            y: top, width, height: bottom - top,
        }
    } else {
        let left = (notch.x - width / 2.).floor().clamp(work.x, work.x + work.width - width);
        let right = (notch.x + notch.width + width / 2.).ceil().min(work.x + work.width);
        Rect {
            x: left,
            y: if edge == NotchEdge::Top { notch.y + notch.height + gap } else { notch.y - gap - height },
            width: right - left, height,
        }
    };
    notch.union(reserve)
}

mod drag;
use drag::DRAG_THRESHOLD;
pub use drag::NotchDrag;

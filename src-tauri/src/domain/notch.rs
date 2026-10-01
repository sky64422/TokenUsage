//! Physical screen placement; no OS calls, persistence, or vendor policy.
use serde::{Deserialize, Serialize};

pub const DEPTH: f64 = 72.;
pub const CELL: f64 = 104.;
pub const SHOULDER: f64 = DEPTH / 2.;
pub const INNER_RADIUS: f64 = DEPTH - SHOULDER;
// Content nestles into the curved ends instead of starting after the entire curve.
pub const END_PADDING: f64 = 50.;
pub const DETAIL_RADIUS: f64 = 16.;
pub const DETAIL_WIDTH: f64 = 340.;
pub const DETAIL_HEIGHT: f64 = 560.;
pub const GAP: f64 = 8.;

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
#[derive(Debug, Clone, Serialize)]
pub struct MonitorArea {
    pub name: String,
    pub bounds: Rect,
    pub work: Rect,
    pub scale: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotchLayout {
    pub edge: NotchEdge,
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
    p.validate()?;
    if !m.scale.is_finite() || m.scale <= 0. || count == 0 || count > 3 {
        return Err("Invalid notch dimensions".into());
    }
    let b = m.bounds;
    let w = m.work;
    let s = m.scale;
    let available = |e| match e {
        NotchEdge::Right => w.x + w.width >= b.x + b.width,
        NotchEdge::Left => w.x <= b.x,
        NotchEdge::Top => w.y <= b.y,
        NotchEdge::Bottom => w.y + w.height >= b.y + b.height,
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
    let detail = if expanded {
        let dw = (DETAIL_WIDTH * s)
            .ceil()
            .min(w.width - if edge.vertical() { depth + GAP * s } else { 0. });
        let dh = (detail_height.clamp(100., DETAIL_HEIGHT) * s)
            .ceil()
            .min(w.height - if edge.vertical() { 0. } else { depth + GAP * s });
        if dw <= 0. || dh <= 0. {
            return Err("Display is too small for usage details".into());
        }
        let mut d = Rect {
            x: (notch.x + notch.width / 2. - dw / 2.).clamp(w.x, w.x + w.width - dw),
            y: (notch.y + notch.height / 2. - dh / 2.).clamp(w.y, w.y + w.height - dh),
            width: dw,
            height: dh,
        };
        match edge {
            NotchEdge::Right => d.x = notch.x - GAP * s - dw,
            NotchEdge::Left => d.x = notch.x + depth + GAP * s,
            NotchEdge::Top => d.y = notch.y + depth + GAP * s,
            NotchEdge::Bottom => d.y = notch.y - GAP * s - dh,
        }
        d.x = d.x.round();
        d.y = d.y.round();
        Some(d)
    } else {
        None
    };
    let window = detail.map(|d| notch.union(d)).unwrap_or(notch);
    Ok(NotchLayout {
        edge,
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
        },
    })
}

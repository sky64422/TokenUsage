//! Physical screen placement; no OS calls, persistence, or vendor policy.
use serde::{Deserialize, Serialize};

pub const DEPTH: f64 = 64.;
pub const CELL: f64 = 88.;
pub const SHOULDER: f64 = DEPTH / 2.;
pub const INNER_RADIUS: f64 = DEPTH - SHOULDER;
// Content nestles into the curved ends instead of starting after the entire curve.
pub const END_PADDING: f64 = 44.;
pub const DETAIL_RADIUS: f64 = 16.;
pub const DETAIL_WIDTH: f64 = 280.;
pub const DETAIL_HEIGHT: f64 = 560.;
pub const MIN_DETAIL_HEIGHT: f64 = 48.;
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
        let mut dh = (detail_height.clamp(MIN_DETAIL_HEIGHT, DETAIL_HEIGHT) * s)
            .ceil()
            .min(w.height - if edge.vertical() { 0. } else { depth + GAP * s });
        if edge.vertical() && target.is_none() {
            dh = dh.min((w.y + w.height - notch.y).max(MIN_DETAIL_HEIGHT * s));
        }
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
    let window = detail.map(|d| notch.union(d)).unwrap_or(notch);
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

const DRAG_THRESHOLD: f64 = 5.;
const EDGE_ENTRY: f64 = 64.;
const EDGE_HYSTERESIS: f64 = 16.;
const MONITOR_ENTRY: f64 = 24.;
const EDGE_ORDER: [NotchEdge; 4] = [
    NotchEdge::Right,
    NotchEdge::Left,
    NotchEdge::Top,
    NotchEdge::Bottom,
];

/// Pure drag selection in physical desktop coordinates. Persistence belongs to the controller.
pub struct NotchDrag {
    pub placement: NotchPlacement,
    pub active: bool,
    start: (f64, f64),
    previous: (f64, f64),
    start_scale: f64,
    grab_ratio: f64,
    count: usize,
}
impl NotchDrag {
    pub fn new(layout: &NotchLayout, x: f64, y: f64, count: usize) -> Self {
        let n = layout.notch;
        let (along, origin, len) = if layout.edge.vertical() {
            (y, n.y, n.height)
        } else {
            (x, n.x, n.width)
        };
        Self {
            placement: NotchPlacement {
                edge: layout.edge,
                monitor_hint: Some(layout.monitor.clone()),
                offset: 0.5,
            },
            active: false,
            start: (x, y),
            previous: (x, y),
            start_scale: layout.scale,
            grab_ratio: ((along - origin) / len).clamp(0., 1.),
            count,
        }
    }
    pub fn update(&mut self, monitors: &[MonitorArea], x: f64, y: f64) -> Result<(), String> {
        if !x.is_finite() || !y.is_finite() {
            return Err("Invalid drag coordinates".into());
        }
        let previous = std::mem::replace(&mut self.previous, (x, y));
        if !self.active {
            if (x - self.start.0).hypot(y - self.start.1) < DRAG_THRESHOLD * self.start_scale {
                return Ok(());
            }
            self.active = true;
        }
        let current = monitors
            .iter()
            .find(|m| Some(&m.name) == self.placement.monitor_hint.as_ref())
            .ok_or("Drag display disappeared")?;
        let mut target = current;
        let mut edge = self.placement.edge;
        if let Some(other) = monitors.iter().find(|m| {
            m.name != current.name && m.bounds.contains(x, y) && entered_monitor(current, m, x, y)
        }) {
            if let Some(candidate) = self
                .closest_edge(other, x, y)
                .or_else(|| self.crossed_entry(current, other, previous, (x, y)))
            {
                target = other;
                edge = candidate;
            }
        } else if current.bounds.contains(x, y) {
            if let Some(candidate) = self.closest_edge(current, x, y) {
                if candidate != edge
                    && edge_distance(current, candidate, x, y) + EDGE_HYSTERESIS
                        <= edge_distance(current, edge, x, y)
                {
                    edge = candidate;
                }
            }
        }
        let mut next = NotchPlacement {
            edge,
            monitor_hint: Some(target.name.clone()),
            offset: 0.,
        };
        let l = calculate_layout(target, &next, self.count, false, 100.)?;
        let (pointer, origin, span, len) = if edge.vertical() {
            (y, target.work.y, target.work.height, l.notch.height)
        } else {
            (x, target.work.x, target.work.width, l.notch.width)
        };
        let travel = span - len;
        next.offset = if travel > 0. {
            ((pointer - self.grab_ratio * len - origin) / travel).clamp(0., 1.)
        } else {
            0.
        };
        self.placement = next;
        Ok(())
    }
    // A native sample may jump over the entire 24..64 DIP docking band.
    // Test the swept segment at entry instead of depending on sampling speed.
    fn crossed_entry(
        &self,
        from: &MonitorArea,
        to: &MonitorArea,
        previous: (f64, f64),
        next: (f64, f64),
    ) -> Option<NotchEdge> {
        EDGE_ORDER
            .into_iter()
            .filter_map(|edge| {
                let before = edge_distance(to, edge, previous.0, previous.1);
                let after = edge_distance(to, edge, next.0, next.1);
                if before >= MONITOR_ENTRY || after < MONITOR_ENTRY {
                    return None;
                }
                let t = (MONITOR_ENTRY - before) / (after - before);
                let x = previous.0 + (next.0 - previous.0) * t;
                let y = previous.1 + (next.1 - previous.1) * t;
                // The destination and hysteresis still apply, including diagonal seams.
                if !to.bounds.contains(x, y) || !entered_monitor(from, to, x, y) {
                    return None;
                }
                self.closest_edge(to, x, y).map(|candidate| (t, candidate))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, edge)| edge)
    }
    fn closest_edge(&self, m: &MonitorArea, x: f64, y: f64) -> Option<NotchEdge> {
        EDGE_ORDER
            .into_iter()
            .filter(|e| {
                let d = edge_distance(m, *e, x, y);
                let p = NotchPlacement {
                    edge: *e,
                    monitor_hint: Some(m.name.clone()),
                    offset: 0.,
                };
                (0.0..=EDGE_ENTRY).contains(&d)
                    && calculate_layout(m, &p, self.count, false, 100.).is_ok_and(|l| l.edge == *e)
            })
            .min_by(|a, b| edge_distance(m, *a, x, y).total_cmp(&edge_distance(m, *b, x, y)))
    }
}
fn edge_distance(m: &MonitorArea, edge: NotchEdge, x: f64, y: f64) -> f64 {
    let b = m.bounds;
    (match edge {
        NotchEdge::Left => x - b.x,
        NotchEdge::Right => b.x + b.width - x,
        NotchEdge::Top => y - b.y,
        NotchEdge::Bottom => b.y + b.height - y,
    }) / m.scale
}

fn entered_monitor(from: &MonitorArea, to: &MonitorArea, x: f64, y: f64) -> bool {
    let a = from.bounds;
    let b = to.bounds;
    let inset = MONITOR_ENTRY * to.scale;
    // Only the crossed seam needs hysteresis; the pointer may hug an outer edge.
    if b.x + b.width <= a.x && b.x + b.width - x < inset {
        return false;
    }
    if b.x >= a.x + a.width && x - b.x < inset {
        return false;
    }
    if b.y + b.height <= a.y && b.y + b.height - y < inset {
        return false;
    }
    if b.y >= a.y + a.height && y - b.y < inset {
        return false;
    }
    true
}

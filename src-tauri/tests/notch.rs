use token_usage_lib::domain::notch::*;
use token_usage_lib::domain::types::PersistedState;

fn monitor(scale: f64) -> MonitorArea {
    MonitorArea {
        name: "test".into(),
        bounds: Rect {
            x: -1920.,
            y: 0.,
            width: 1920.,
            height: 1080.,
        },
        work: Rect {
            x: -1920.,
            y: 0.,
            width: 1920.,
            height: 1040.,
        },
        scale,
    }
}

#[test]
fn edges_anchor_and_detail_stays_inside_work_area() {
    for edge in [NotchEdge::Right, NotchEdge::Left, NotchEdge::Top] {
        for scale in [1., 1.25, 1.5, 2.] {
            for offset in [0., 0.5, 1.] {
                let layout = calculate_layout(
                    &monitor(scale),
                    &NotchPlacement {
                        edge,
                        offset,
                        monitor_hint: None,
                    },
                    3,
                    true,
                    360.,
                )
                .unwrap();
                let r = layout.notch;
                match edge {
                    NotchEdge::Right => assert_eq!(r.x + r.width, 0.),
                    NotchEdge::Left => assert_eq!(r.x, -1920.),
                    _ => assert_eq!(r.y, 0.),
                }
                let d = layout.detail.unwrap();
                assert!(d.x >= -1920. && d.x + d.width <= 0.);
                assert!(d.y >= 0. && d.y + d.height <= 1040.);
            }
        }
    }
}

#[test]
fn taskbar_edge_falls_back_and_invalid_offset_errors() {
    let mut p = NotchPlacement::default();
    p.edge = NotchEdge::Bottom;
    assert_ne!(
        calculate_layout(&monitor(1.), &p, 3, false, 360.)
            .unwrap()
            .edge,
        NotchEdge::Bottom
    );
    p.offset = f64::NAN;
    assert!(calculate_layout(&monitor(1.), &p, 3, false, 360.).is_err());
}

#[test]
fn transparent_corners_are_not_interactive_but_body_is() {
    let l = calculate_layout(&monitor(1.), &NotchPlacement::default(), 3, false, 360.).unwrap();
    assert!(!l.hit(l.notch.x + 1., l.notch.y + 1.));
    assert!(l.hit(l.notch.x + 36., l.notch.y + l.notch.height / 2.));
    assert!(!l.hit(l.notch.x - 10., l.notch.y + 100.));
}

#[test]
fn old_settings_keep_preferences_and_gain_default_placement() {
    let mut value = serde_json::to_value(PersistedState::default()).unwrap();
    value["settings"].as_object_mut().unwrap().remove("notch");
    value["settings"]["opacity"] = serde_json::json!(0.65);
    let restored: PersistedState = serde_json::from_value(value).unwrap();
    assert_eq!(restored.settings.notch, NotchPlacement::default());
    assert_eq!(restored.settings.opacity, 0.65);
}

#[test]
fn all_four_edges_and_open_corridors_work_without_taskbar() {
    let mut m = monitor(1.);
    m.work = m.bounds;
    for edge in [
        NotchEdge::Top,
        NotchEdge::Right,
        NotchEdge::Bottom,
        NotchEdge::Left,
    ] {
        let p = NotchPlacement {
            edge,
            ..Default::default()
        };
        let l = calculate_layout(&m, &p, 1, true, 200.).unwrap();
        assert_eq!(l.edge, edge);
        let n = l.notch;
        let d = l.detail.unwrap();
        let (x, y) = match edge {
            NotchEdge::Right => (n.x - 4., n.y + n.height / 2.),
            NotchEdge::Left => (n.x + n.width + 4., n.y + n.height / 2.),
            NotchEdge::Top => (n.x + n.width / 2., n.y + n.height + 4.),
            NotchEdge::Bottom => (n.x + n.width / 2., n.y - 4.),
        };
        assert!(l.hit(x, y), "bridge missing for {edge:?}");
        assert!(l.hit(d.x + d.width / 2., d.y + d.height / 2.));
    }
}

#[test]
fn insufficient_detail_space_is_an_error_not_a_negative_rectangle() {
    let mut m = monitor(1.);
    m.bounds.width = 75.;
    m.work = m.bounds;
    assert!(calculate_layout(&m, &NotchPlacement::default(), 1, true, 200.).is_err());
}

#[test]
fn detail_rounded_corners_pass_through_at_every_scale() {
    for scale in [1., 1.25, 1.5, 2.] {
        let l =
            calculate_layout(&monitor(scale), &NotchPlacement::default(), 3, true, 200.).unwrap();
        let d = l.detail.unwrap();
        assert!(!l.hit(d.x + scale, d.y + scale));
        assert!(l.hit(d.x + 20. * scale, d.y + 20. * scale));
    }
}

#[test]
fn failed_placement_save_keeps_committed_settings() {
    use token_usage_lib::application::service::AppCore;
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("not-a-directory");
    std::fs::write(&path, "occupied").unwrap();
    let core = AppCore::new(PersistedState::default(), path);
    assert!(core
        .set_notch_placement(NotchPlacement {
            offset: 0.1,
            ..Default::default()
        })
        .is_err());
    assert_eq!(core.get_state().settings.notch, NotchPlacement::default());
}

#[test]
fn circular_end_hit_regions_follow_the_visible_silhouette() {
    for scale in [1., 1.25, 1.5, 2.] {
        let l = calculate_layout(&monitor(scale), &NotchPlacement::default(), 3, false, 360.).unwrap();
        // Midpoints on each half of the S curve; check both sides of the arc.
        for (along, boundary) in [(18., 36. + 972_f64.sqrt()), (54., 36. - 972_f64.sqrt())] {
            for end in [along * scale, l.notch.height - along * scale] {
                assert!(!l.hit(l.notch.x + (boundary - 0.5) * scale, l.notch.y + end));
                assert!(l.hit(l.notch.x + (boundary + 0.5) * scale, l.notch.y + end));
            }
        }
    }
}

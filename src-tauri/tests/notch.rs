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
fn bottom_overlaps_taskbar_and_invalid_offset_errors() {
    let mut p = NotchPlacement {
        edge: NotchEdge::Bottom,
        ..Default::default()
    };
    assert_eq!(
        calculate_layout(&monitor(1.), &p, 3, false, 360.)
            .unwrap()
            .edge,
        NotchEdge::Bottom
    );
    let m = monitor(1.);
    let l = calculate_layout(&m, &p, 3, true, 360.).unwrap();
    assert_eq!(l.notch.y + l.notch.height, m.bounds.y + m.bounds.height);
    assert!(l.notch.y + l.notch.height > m.work.y + m.work.height);
    let detail = l.detail.unwrap();
    assert!(detail.y + detail.height <= m.work.y + m.work.height);
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
fn compact_single_row_detail_shrinks_below_100() {
    let m = monitor(1.);
    let l_single = calculate_layout(&m, &NotchPlacement::default(), 2, true, 78.).unwrap();
    assert_eq!(l_single.detail.unwrap().height, 78.);

    let l_double = calculate_layout(&m, &NotchPlacement::default(), 2, true, 130.).unwrap();
    assert_eq!(l_double.detail.unwrap().height, 130.);
    assert!(l_single.detail.unwrap().height < l_double.detail.unwrap().height);
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
    m.bounds.width = DEPTH + GAP;
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
        let l =
            calculate_layout(&monitor(scale), &NotchPlacement::default(), 3, false, 360.).unwrap();
        // Midpoints on each half of the S curve; check both sides of the arc.
        let p1_along = SHOULDER / 2.;
        let p1_boundary = DEPTH - SHOULDER + (SHOULDER.powi(2) - p1_along.powi(2)).sqrt();
        let p2_along = SHOULDER + INNER_RADIUS / 2.;
        let p2_boundary = INNER_RADIUS - (INNER_RADIUS.powi(2) - (p2_along - SHOULDER - INNER_RADIUS).powi(2)).sqrt();
        for (along, boundary) in [(p1_along, p1_boundary), (p2_along, p2_boundary)] {
            for end in [along * scale, l.notch.height - along * scale] {
                assert!(!l.hit(l.notch.x + (boundary - 0.5) * scale, l.notch.y + end));
                assert!(l.hit(l.notch.x + (boundary + 0.5) * scale, l.notch.y + end));
            }
        }
    }
}

fn drag_monitor(scale: f64) -> MonitorArea {
    MonitorArea {
        name: "main".into(),
        bounds: Rect {
            x: 0.,
            y: 0.,
            width: 2560.,
            height: 1440.,
        },
        work: Rect {
            x: 0.,
            y: 0.,
            width: 2560.,
            height: 1440.,
        },
        scale,
    }
}

#[test]
fn drag_threshold_corner_hysteresis_and_all_edges() {
    for scale in [1., 1.25, 1.5, 2.] {
        let m = drag_monitor(scale);
        let l = calculate_layout(&m, &NotchPlacement::default(), 2, false, 200.).unwrap();
        let start = (l.notch.x + 36. * scale, l.notch.y + l.notch.height / 2.);
        let mut d = NotchDrag::new(&l, start.0, start.1, 2);
        d.update(std::slice::from_ref(&m), start.0 - 4. * scale, start.1)
            .unwrap();
        assert!(!d.active);
        d.update(std::slice::from_ref(&m), start.0 - 6. * scale, start.1)
            .unwrap();
        assert!(d.active, "perpendicular motion must start drag");
        d.update(std::slice::from_ref(&m), 2559., 1.).unwrap();
        assert_eq!(d.placement.edge, NotchEdge::Right);
        d.update(std::slice::from_ref(&m), 2500., 1.).unwrap();
        assert_eq!(d.placement.edge, NotchEdge::Top);
        d.update(std::slice::from_ref(&m), 2559., 1.).unwrap();
        assert_eq!(d.placement.edge, NotchEdge::Top, "ties retain edge");
        d.update(std::slice::from_ref(&m), 1., 200.).unwrap();
        assert_eq!(d.placement.edge, NotchEdge::Left);
        d.update(std::slice::from_ref(&m), 500., 1439.).unwrap();
        assert_eq!(d.placement.edge, NotchEdge::Bottom);
        d.update(std::slice::from_ref(&m), 2559., 600.).unwrap();
        assert_eq!(d.placement.edge, NotchEdge::Right);
        d.update(&[m], 1200., 600.).unwrap();
        assert_eq!(d.placement.edge, NotchEdge::Right, "interior retains edge");
    }
}

#[test]
fn drag_allows_bottom_taskbar_and_requires_monitor_inset() {
    let mut m = drag_monitor(1.);
    m.work.height -= 48.;
    let l = calculate_layout(&m, &NotchPlacement::default(), 2, false, 200.).unwrap();
    let mut d = NotchDrag::new(&l, 2524., 600., 2);
    d.update(std::slice::from_ref(&m), 1200., 1439.).unwrap();
    assert_eq!(d.placement.edge, NotchEdge::Bottom);
    let mut other = drag_monitor(1.5);
    other.name = "other".into();
    other.bounds.x = -2560.;
    other.work.x = -2560.;
    let monitors = [m, other];
    d.update(&monitors, -10., 600.).unwrap();
    assert_eq!(d.placement.monitor_hint.as_deref(), Some("main"));
    d.update(&monitors, -40., 600.).unwrap();
    assert_eq!(d.placement.monitor_hint.as_deref(), Some("other"));
    assert_eq!(d.placement.edge, NotchEdge::Right);
    d.update(&monitors, 10., 600.).unwrap();
    assert_eq!(d.placement.monitor_hint.as_deref(), Some("other"));
}

#[test]
fn drag_keeps_grab_ratio_and_handles_zero_travel() {
    let mut m = drag_monitor(1.);
    let l = calculate_layout(&m, &NotchPlacement::default(), 1, false, 200.).unwrap();
    let mut d = NotchDrag::new(&l, 2524., l.notch.y + l.notch.height / 4., 1);
    d.update(&[m.clone()], 1000., 1.).unwrap();
    let moved = calculate_layout(&m, &d.placement, 1, false, 200.).unwrap();
    assert!((moved.notch.x + moved.notch.width / 4. - 1000.).abs() <= 1.);
    m.bounds.width = CELL + END_PADDING * 2.;
    m.work.width = m.bounds.width;
    d.update(&[m], 100., 1.).unwrap();
    assert_eq!(d.placement.offset, 0.);
}

#[test]
fn drag_can_cross_monitor_seams_while_following_an_outer_edge() {
    let m = drag_monitor(1.);
    let mut other = m.clone();
    other.name = "other".into();
    other.bounds.x = -2560.;
    other.work.x = -2560.;
    let l = calculate_layout(
        &m,
        &NotchPlacement {
            edge: NotchEdge::Top,
            ..Default::default()
        },
        2,
        false,
        200.,
    )
    .unwrap();
    let mut d = NotchDrag::new(&l, 1200., 10., 2);
    d.update(&[m.clone(), other], -100., 10.).unwrap();
    assert_eq!(d.placement.monitor_hint.as_deref(), Some("other"));
    assert_eq!(d.placement.edge, NotchEdge::Top);
    let mut above = m.clone();
    above.name = "above".into();
    above.bounds.y = -1440.;
    above.work.y = -1440.;
    let l = calculate_layout(
        &m,
        &NotchPlacement {
            edge: NotchEdge::Left,
            ..Default::default()
        },
        2,
        false,
        200.,
    )
    .unwrap();
    let mut d = NotchDrag::new(&l, 10., 600., 2);
    d.update(&[m, above], 10., -100.).unwrap();
    assert_eq!(d.placement.monitor_hint.as_deref(), Some("above"));
    assert_eq!(d.placement.edge, NotchEdge::Left);
}

#[test]
fn fast_monitor_round_trips_cannot_skip_the_entry_band() {
    for scale in [1., 1.25, 1.5, 2.] {
        let right = drag_monitor(scale);
        let mut left = right.clone();
        left.name = "left".into();
        left.bounds.x = -2560.;
        left.work.x = -2560.;
        let l = calculate_layout(&left, &NotchPlacement::default(), 2, false, 200.).unwrap();
        let mut d = NotchDrag::new(&l, -36. * scale, 600., 2);
        let monitors = [right, left];
        // First crossing is slow and succeeds; reverse motion skips the narrow band.
        d.update(&monitors, 40. * scale, 600.).unwrap();
        assert_eq!(d.placement.monitor_hint.as_deref(), Some("main"));
        for _ in 0..5 {
            d.update(&monitors, -180. * scale, 600.).unwrap();
            assert_eq!(d.placement.monitor_hint.as_deref(), Some("left"));
            assert_eq!(d.placement.edge, NotchEdge::Right);
            d.update(&monitors, 180. * scale, 600.).unwrap();
            assert_eq!(d.placement.monitor_hint.as_deref(), Some("main"));
            assert_eq!(d.placement.edge, NotchEdge::Left);
        }
    }
}

#[test]
fn fast_stacked_monitor_crossing_preserves_entry_hysteresis() {
    let bottom = drag_monitor(1.);
    let mut top = bottom.clone();
    top.name = "top".into();
    top.bounds.y = -1440.;
    top.work.y = -1440.;
    let l = calculate_layout(
        &top,
        &NotchPlacement {
            edge: NotchEdge::Bottom,
            ..Default::default()
        },
        2,
        false,
        200.,
    )
    .unwrap();
    let mut d = NotchDrag::new(&l, 1000., -36., 2);
    let monitors = [bottom, top];
    d.update(&monitors, 1000., 10.).unwrap();
    assert_eq!(d.placement.monitor_hint.as_deref(), Some("top"));
    d.update(&monitors, 1000., 180.).unwrap();
    assert_eq!(d.placement.monitor_hint.as_deref(), Some("main"));
    d.update(&monitors, 1000., -180.).unwrap();
    assert_eq!(d.placement.monitor_hint.as_deref(), Some("top"));
}

#[test]
fn anchor_corner_stays_immobile_on_detail_expansion() {
    let m = monitor(1.);
    for edge in [NotchEdge::Right, NotchEdge::Left, NotchEdge::Top, NotchEdge::Bottom] {
        for offset in [0.1, 0.5, 0.9] {
            let p = NotchPlacement {
                edge,
                offset,
                monitor_hint: None,
            };
            let closed = calculate_layout(&m, &p, 2, false, 350.).unwrap();
            let open = calculate_layout(&m, &p, 2, true, 350.).unwrap();

            assert_eq!(closed.anchor_x, open.anchor_x);
            assert_eq!(closed.anchor_y, open.anchor_y);

            // Anchor corner must be identical between closed and open states.
            match open.anchor_x {
                AnchorX::Left => assert_eq!(closed.window.x, open.window.x),
                AnchorX::Right => {
                    assert_eq!(closed.window.x + closed.window.width, open.window.x + open.window.width)
                }
            }
            match open.anchor_y {
                AnchorY::Top => assert_eq!(closed.window.y, open.window.y),
                AnchorY::Bottom => {
                    assert_eq!(closed.window.y + closed.window.height, open.window.y + open.window.height)
                }
            }
        }
    }
}
#[test]
fn all_current_providers_fit_each_edge() {
    use token_usage_lib::domain::types::ProviderId;
    for edge in [NotchEdge::Left, NotchEdge::Right, NotchEdge::Top, NotchEdge::Bottom] {
        let layout = calculate_layout(&monitor(1.), &NotchPlacement { edge, ..Default::default() }, ProviderId::all().len(), false, 0.).unwrap();
        assert_eq!(layout.edge, edge);
    }
}

#[test]
fn model_detail_window_centers_on_target_cell_across_providers() {
    use token_usage_lib::domain::notch::calculate_layout_target;
    let m = monitor(1.);
    let p = NotchPlacement {
        edge: NotchEdge::Right,
        offset: 0.9,
        monitor_hint: None,
    };
    let l_claude = calculate_layout_target(&m, &p, 4, true, 78., Some(41.)).unwrap();
    let l_codex = calculate_layout_target(&m, &p, 4, true, 130., Some(111.)).unwrap();
    let l_grok = calculate_layout_target(&m, &p, 4, true, 78., Some(181.)).unwrap();
    let l_agy = calculate_layout_target(&m, &p, 4, true, 78., Some(251.)).unwrap();

    let d_claude = l_claude.detail.unwrap();
    let d_codex = l_codex.detail.unwrap();
    let d_grok = l_grok.detail.unwrap();
    let d_agy = l_agy.detail.unwrap();

    assert!(d_claude.y < d_codex.y);
    assert!(d_codex.y < d_grok.y);
    assert!(d_grok.y < d_agy.y);

    assert!(l_claude.hit(l_claude.notch.x - 4., d_claude.y + d_claude.height / 2.));
    assert!(l_codex.hit(l_codex.notch.x - 4., d_codex.y + d_codex.height / 2.));
    assert!(l_grok.hit(l_grok.notch.x - 4., d_grok.y + d_grok.height / 2.));
    assert!(l_agy.hit(l_agy.notch.x - 4., d_agy.y + d_agy.height / 2.));

    assert_eq!(l_claude.window, l_codex.window);
    assert_eq!(l_codex.window, l_grok.window);
    assert_eq!(l_grok.window, l_agy.window);
}

#[test]
fn changing_detail_preserves_native_canvas_and_notch_position() {
    for edge in [NotchEdge::Left, NotchEdge::Right, NotchEdge::Top, NotchEdge::Bottom] {
        for scale in [1., 1.25, 1.5, 2.] {
            for offset in [0., 0.5, 1.] {
                for count in 1..=4 {
                    let m = monitor(scale);
                    let p = NotchPlacement { edge, offset, monitor_hint: None };
                    let closed = calculate_layout(&m, &p, count, false, 0.).unwrap();
                    let last = END_PADDING + CELL * (count as f64 - 0.5);
                    for target in [None, Some(END_PADDING + CELL / 2.), Some(last)] {
                        for height in [78., 150., 234., 400., DETAIL_HEIGHT] {
                            let open = calculate_layout_target(&m, &p, count, true, height, target).unwrap();
                            assert_eq!(closed.window, open.window);
                            assert_eq!(closed.notch, open.notch);
                            let d = open.detail.unwrap();
                            let w = open.window;
                            assert!(d.x >= w.x && d.y >= w.y, "edge={edge:?} scale={scale} offset={offset} count={count} target={target:?} height={height} detail={d:?} window={w:?}");
                            assert!(d.x + d.width <= w.x + w.width);
                            assert!(d.y + d.height <= w.y + w.height);
                            // Reserved pixels must not intercept input with details closed.
                            assert!(!closed.hit(d.x + d.width / 2., d.y + d.height / 2.));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn horizontal_notch_detail_window_centers_on_target_cell() {
    use token_usage_lib::domain::notch::calculate_layout_target;
    let m = monitor(1.);
    let p = NotchPlacement {
        edge: NotchEdge::Top,
        offset: 0.5,
        monitor_hint: None,
    };
    let l_first = calculate_layout_target(&m, &p, 4, true, 78., Some(41.)).unwrap();
    let l_last = calculate_layout_target(&m, &p, 4, true, 78., Some(251.)).unwrap();

    let d_first = l_first.detail.unwrap();
    let d_last = l_last.detail.unwrap();

    assert!(d_first.x < d_last.x);
    assert!(l_first.hit(d_first.x + d_first.width / 2., l_first.notch.y + l_first.notch.height + 4.));
    assert!(l_last.hit(d_last.x + d_last.width / 2., l_last.notch.y + l_last.notch.height + 4.));
}

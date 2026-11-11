use super::*;
fn bounds(x: f64, y: f64, w: f64, h: f64) -> ObjectBounds {
    ObjectBounds {
        min: [x, y],
        max: [x + w, y + h],
    }
}
fn target(id: u128, bounds: ObjectBounds) -> Target {
    Target {
        id: ObjectId::from_u128(id),
        bounds,
    }
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}
#[test]
fn edges_centers_and_cross_edges_snap_independently_at_every_zoom() {
    for scale in [0.5, 1., 2., 4.] {
        for axis in 0..2 {
            for source in 0..3 {
                for destination in 0..3 {
                    if (source == 1) != (destination == 1) {
                        continue;
                    }
                    let t = bounds(100., 200., 100., 120.);
                    let mut b = bounds(450., 550., 20., 30.);
                    let correction =
                        value(t, axis, destination) - value(b, axis, source) - 3. / scale;
                    b.min[axis] += correction;
                    b.max[axis] += correction;
                    let mut snap = SnapSession::new(vec![target(1, t)], SnapSettings::default());
                    let mut active = [None; 2];
                    active[axis] = Some(source);
                    let result = snap.solve(b, scale, false, Some(active));
                    near(result[axis], 3. / scale);
                    near(result[1 - axis], 0.);
                    assert!(snap.guides[axis].is_some());
                }
            }
        }
        let mut snap = SnapSession::new(
            vec![target(1, bounds(100., 200., 20., 20.))],
            SnapSettings::default(),
        );
        let result = snap.solve(
            bounds(100. - 3. / scale, 200. - 2. / scale, 20., 20.),
            scale,
            false,
            None,
        );
        near(result[0], 3. / scale);
        near(result[1], 2. / scale);
    }
}
#[test]
fn threshold_is_screen_space_with_stable_latches_and_natural_release() {
    for sensitivity in [Sensitivity::Low, Sensitivity::Normal, Sensitivity::High] {
        for scale in [0.5, 1., 2., 4.] {
            let settings = SnapSettings {
                sensitivity,
                ..Default::default()
            };
            let t = settings.sensitivity.threshold() / scale;
            let mut snap =
                SnapSession::new(vec![target(1, bounds(100., 500., 200., 20.))], settings);
            near(
                snap.solve(
                    bounds(100. - t - 0.01, 0., 30., 30.),
                    scale,
                    false,
                    Some([Some(0), None]),
                )[0],
                0.,
            );
            near(
                snap.solve(
                    bounds(100. - t + 0.01, 0., 30., 30.),
                    scale,
                    false,
                    Some([Some(0), None]),
                )[0],
                t - 0.01,
            );
            near(
                snap.solve(
                    bounds(100. - t * 1.4, 0., 30., 30.),
                    scale,
                    false,
                    Some([Some(0), None]),
                )[0],
                t * 1.4,
            );
            near(
                snap.solve(
                    bounds(100. - t * 1.6, 0., 30., 30.),
                    scale,
                    false,
                    Some([Some(0), None]),
                )[0],
                0.,
            );
        }
    }
    let mut snap = SnapSession::new(
        vec![
            target(1, bounds(100., 200., 30., 30.)),
            target(2, bounds(106., 400., 30., 30.)),
        ],
        SnapSettings::default(),
    );
    near(
        snap.solve(bounds(98., 0., 10., 10.), 1., false, Some([Some(0), None]))[0],
        2.,
    );
    for _ in 0..20 {
        near(
            snap.solve(bounds(104., 0., 10., 10.), 1., false, Some([Some(0), None]))[0],
            -4.,
        );
    }
    near(
        snap.solve(bounds(113., 0., 10., 10.), 1., false, Some([Some(0), None]))[0],
        -7.,
    );
    near(
        snap.solve(
            bounds(-10000., 0., 10., 10.),
            1.,
            false,
            Some([Some(0), None]),
        )[0],
        0.,
    );
}
#[test]
fn equal_spacing_uses_real_sizes_and_overlapping_rows_without_clutter() {
    for axis in 0..2 {
        let swap = |mut b: ObjectBounds| {
            if axis == 1 {
                b.min.swap(0, 1);
                b.max.swap(0, 1);
            }
            b
        };
        let mut snap = SnapSession::new(
            vec![
                target(1, swap(bounds(0., 0., 20., 50.))),
                target(2, swap(bounds(200., 0., 70., 50.))),
            ],
            SnapSettings::default(),
        );
        let moving = swap(bounds(94., 10., 40., 20.));
        let correction = snap.solve(moving, 1., false, None);
        near(correction[axis], -4.);
        let guide = snap.guides[axis].unwrap();
        let gaps = guide.gaps.unwrap();
        near(gaps[0][1] - gaps[0][0], gaps[1][1] - gaps[1][0]);
        snap.clear();
        let wrong_row = swap(bounds(94., 200., 40., 20.));
        snap.solve(wrong_row, 1., false, None);
        assert!(!snap.guides.iter().flatten().any(|g| g.gaps.is_some()));
    }
    let mut snap = SnapSession::new(
        vec![
            target(1, bounds(0., 0., 20., 30.)),
            target(2, bounds(50., 0., 20., 30.)),
        ],
        SnapSettings::default(),
    );
    snap.solve(bounds(23., 0., 40., 30.), 1., false, None);
    assert!(!snap.guides.iter().flatten().any(|g| g.gaps.is_some()));
}
#[test]
fn grid_modes_hidden_grid_and_temporary_bypass_are_independent() {
    let mut snap = SnapSession::new(
        vec![],
        SnapSettings {
            objects: false,
            grid: true,
            show_grid: false,
            spacing: 20.,
            ..Default::default()
        },
    );
    assert_eq!(
        snap.solve(bounds(18., 39., 10., 10.), 1., false, None),
        [2., 1.]
    );
    assert!(snap.guides.iter().all(Option::is_none));
    assert_eq!(
        snap.solve(bounds(18., 39., 10., 10.), 1., true, None),
        [0.; 2]
    );
    assert!(snap.latch.iter().all(Option::is_none));
    assert_eq!(
        snap.solve(bounds(18., 39., 10., 10.), 1., false, None),
        [2., 1.]
    );
    let mut snap = SnapSession::new(
        vec![target(1, bounds(23., 100., 20., 20.))],
        SnapSettings {
            grid: true,
            spacing: 20.,
            ..Default::default()
        },
    );
    near(
        snap.solve(bounds(21., 0., 10., 10.), 1., false, Some([Some(0), None]))[0],
        2.,
    );
    snap.settings.grid = false;
    snap.clear();
    near(
        snap.solve(bounds(21., 0., 10., 10.), 1., false, Some([Some(0), None]))[0],
        2.,
    );
}
#[test]
fn proportional_resize_snaps_one_realizable_edge_and_keeps_opposite_anchor() {
    let mut snap = SnapSession::new(
        vec![target(1, bounds(100., 200., 30., 30.))],
        SnapSettings::default(),
    );
    let original = bounds(0., 0., 50., 50.);
    let proposed = crate::object::Placement {
        scale: 1.96,
        ..Default::default()
    };
    let result = snap.resize(original, proposed, 2, 1., false);
    near(result.scale, 2.);
    assert_eq!(result.apply([0., 0.]), [0.; 2]);
    near(result.apply([50., 50.])[0], 100.);
    snap.reconcile(original.placed(result));
    assert_eq!(snap.guides.iter().flatten().count(), 1);
    snap.reconcile(bounds(0., 0., 80., 80.));
    assert!(snap.guides.iter().all(Option::is_none));
}
#[test]
fn align_commands_preserve_bounds_dimensions_and_use_collective_reference() {
    for command in [
        Align::Left,
        Align::Center,
        Align::Right,
        Align::Top,
        Align::Middle,
        Align::Bottom,
    ] {
        let targets = vec![
            target(1, bounds(10., 20., 30., 40.)),
            target(2, bounds(100., 200., 70., 80.)),
            target(3, bounds(45., 78., 15., 55.)),
        ];
        let reference = targets
            .iter()
            .map(|t| t.bounds)
            .reduce(ObjectBounds::union)
            .unwrap();
        let axis = usize::from(matches!(
            command,
            Align::Top | Align::Middle | Align::Bottom
        ));
        let feature = match command {
            Align::Left | Align::Top => 0,
            Align::Center | Align::Middle => 1,
            _ => 2,
        };
        for (id, delta) in alignment(&targets, command).unwrap() {
            let original = targets.iter().find(|t| t.id == id).unwrap().bounds;
            let result = original.placed(crate::object::Placement {
                position: delta,
                ..Default::default()
            });
            near(
                value(result, axis, feature),
                value(reference, axis, feature),
            );
            near(delta[1 - axis], 0.);
            for i in 0..2 {
                near(
                    result.max[i] - result.min[i],
                    original.max[i] - original.min[i],
                );
            }
        }
    }
}
#[test]
fn unequal_size_distribution_pins_extremes_and_refuses_insufficient_space() {
    for axis in 0..2 {
        let swap = |mut b: ObjectBounds| {
            if axis == 1 {
                b.min.swap(0, 1);
                b.max.swap(0, 1);
            }
            b
        };
        let mut targets = vec![
            target(1, swap(bounds(0., 0., 20., 20.))),
            target(2, swap(bounds(21., 40., 40., 20.))),
            target(3, swap(bounds(90., 80., 10., 20.))),
            target(4, swap(bounds(200., 120., 50., 20.))),
        ];
        let command = if axis == 0 {
            Align::DistributeHorizontal
        } else {
            Align::DistributeVertical
        };
        let movement = alignment(&targets, command).unwrap();
        assert_eq!(movement.len(), 2);
        for (id, delta) in movement {
            assert!(![ObjectId::from_u128(1), ObjectId::from_u128(4)].contains(&id));
            let t = targets.iter_mut().find(|t| t.id == id).unwrap();
            t.bounds = t.bounds.placed(crate::object::Placement {
                position: delta,
                ..Default::default()
            });
        }
        for pair in targets.windows(2) {
            near(
                pair[1].bounds.min[axis] - pair[0].bounds.max[axis],
                (200. - 20. - 40. - 10.) / 3.,
            );
        }
        let cramped = vec![
            target(1, swap(bounds(0., 0., 40., 20.))),
            target(2, swap(bounds(10., 0., 40., 20.))),
            target(3, swap(bounds(20., 0., 40., 20.))),
        ];
        assert!(alignment(&cramped, command).is_err());
        assert!(alignment(&targets[..2], command).is_err());
    }
}
#[test]
fn far_origin_precision_and_guides_disabled_do_not_change_snapping() {
    let origin = 1e12;
    let mut snap = SnapSession::new(
        vec![target(1, bounds(origin, origin, 100., 100.))],
        SnapSettings {
            guides: false,
            ..Default::default()
        },
    );
    assert_eq!(
        snap.solve(
            bounds(origin - 2., origin - 3., 100., 100.),
            4.,
            false,
            None
        ),
        [2., 0.]
    );
    assert!(snap.guides.iter().all(Option::is_none));
    assert!(
        !SnapSettings {
            spacing: f64::NAN,
            ..Default::default()
        }
        .valid()
    );
    assert!(
        !SnapSettings {
            spacing: 0.,
            ..Default::default()
        }
        .valid()
    );
}
#[test]
#[ignore = "manual release-mode cached snapping candidate benchmark"]
fn cached_snapping_candidate_performance() {
    let targets = (0..10000)
        .map(|i| {
            target(
                i + 1,
                bounds((i % 100) as f64 * 100., (i / 100) as f64 * 100., 40., 40.),
            )
        })
        .collect();
    let mut snap = SnapSession::new(targets, SnapSettings::default());
    let pointers = snap.features.each_ref().map(|v| v.as_ptr());
    let start = std::time::Instant::now();
    for sample in 0..2000 {
        snap.solve(
            bounds(
                (sample % 100) as f64 * 100. + 3.,
                (sample / 100) as f64 * 100. + 2.,
                30.,
                30.,
            ),
            4.,
            false,
            None,
        );
        assert_eq!(snap.features.each_ref().map(|v| v.as_ptr()), pointers);
    }
    eprintln!(
        "10,000 cached targets / 2,000 snap samples: {:?}",
        start.elapsed()
    );
}
#[test]
fn nearly_parallel_resize_rail_does_not_amplify_a_small_snap_into_a_large_jump() {
    let mut snap = SnapSession::new(
        vec![target(1, bounds(100., 500., 40., 40.))],
        SnapSettings::default(),
    );
    let grip = [99., 100.];
    let result = snap.handle(
        ObjectBounds {
            min: grip,
            max: grip,
        },
        grip,
        Some([0.001, 1.]),
        true,
        1.,
        false,
    );
    assert_eq!(result, grip);
    assert!(snap.guides.iter().all(Option::is_none));
}

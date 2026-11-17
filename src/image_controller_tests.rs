use super::*;
use crate::canvas::selection_ui::tests::{
    click, frame, frame_shapes, moved, pointer, screen, setup, t,
};
use crate::object::ObjectBounds;
use crate::{
    document::DocumentKind,
    image_object::test_support,
    object_clipboard::ObjectBundle,
    selection::{LassoRegion, SelectionCommand},
    shape::{Shape, ShapeKind},
};
fn add(c: &mut Canvas, s: Surface, p: Point) -> Uuid {
    let mut image = ImageObject::new(c.drawing.owner(), s, p, test_support::asset());
    image.dimensions = [120., 60.];
    let id = image.id;
    assert!(c.drawing.insert_snapshots(vec![image.into()]));
    id
}
fn get(c: &Canvas, id: Uuid) -> Arc<ImageObject> {
    match c.drawing.snapshot(id).unwrap() {
        ObjectSnapshot::Image(i) => i.clone(),
        _ => panic!(),
    }
}
#[test]
fn image_history_grouping_clipboard_cross_document_and_asset_reuse() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        let (mut c, s) = setup(kind);
        let id = add(&mut c, s, [80., 100.]);
        let original = get(&c, id);
        let ctx = egui::Context::default();
        assert!(c.drawing.dirty());
        c.selection.change([id], Membership::Replace, &c.drawing);
        c.selection_action(SelectionCommand::Duplicate, &ctx);
        let duplicate = *c.selection.ids.iter().next().unwrap();
        assert_ne!(duplicate, id);
        assert!(Arc::ptr_eq(&original.asset, &get(&c, duplicate).asset));
        c.selection
            .change([id, duplicate], Membership::Replace, &c.drawing);
        c.selection_action(SelectionCommand::Group, &ctx);
        let group = *c.selection.ids.iter().next().unwrap();
        let assets_before = get(&c, id).asset.clone();
        c.selection_action(SelectionCommand::RotateRight, &ctx);
        c.selection_action(SelectionCommand::FlipHorizontal, &ctx);
        assert!(get(&c, id).placement.reflected[0]);
        assert!(Arc::ptr_eq(&get(&c, id).asset, &assets_before));
        c.selection_action(SelectionCommand::Copy, &ctx);
        let bundle = ObjectBundle::capture_objects(c.drawing.items().iter().cloned()).unwrap();
        let encoded = bundle.encode().unwrap();
        let json: serde_json::Value = serde_json::from_str(
            encoded
                .strip_prefix(crate::object_clipboard::PREFIX)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["assets"].as_array().unwrap().len(), 1);
        let mut legacy = json;
        for object in legacy["objects"].as_array_mut().unwrap() {
            if object["kind"] == "Image" {
                object["data"]["preserve_aspect"] = true.into();
            }
        }
        let legacy = format!("{}{legacy}", crate::object_clipboard::PREFIX);
        let restored = ObjectBundle::decode(&legacy).unwrap();
        let (mut other, other_surface) = setup(DocumentKind::Whiteboard);
        let pasted = restored.paste(&mut other.drawing, other_surface, [250., 250.]);
        assert_eq!(pasted.len(), 1);
        let images: Vec<_> = other
            .drawing
            .items()
            .iter()
            .filter_map(|o| {
                if let ObjectSnapshot::Image(i) = o {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(images.len(), 2);
        assert!(images.iter().all(|i| i.id != id
            && i.id != duplicate
            && i.document_id == other.drawing.owner()
            && i.asset.bytes() == original.asset.bytes()));
        assert!(Arc::ptr_eq(&images[0].asset, &images[1].asset));
        let snapshot = other.drawing.items().to_vec();
        other.drawing.undo();
        assert!(other.drawing.items().is_empty());
        other.drawing.redo();
        assert_eq!(other.drawing.items(), snapshot);
        c.selection_action(SelectionCommand::Ungroup, &ctx);
        assert_eq!(c.selection.ids.len(), 2);
        assert!(c.drawing.snapshot(group).is_none());
        c.selection.change([id], Membership::Replace, &c.drawing);
        c.selection_action(SelectionCommand::Lock, &ctx);
        assert!(c.drawing.is_locked(id));
        assert!(!c.drawing.replace_image((*get(&c, id)).clone()));
        c.drawing.lock_objects(&HashSet::from([id]), false);
        c.drawing.erase(&HashSet::from([id]));
        assert!(c.drawing.snapshot(id).is_none());
        c.drawing.undo();
        assert!(Arc::ptr_eq(&get(&c, id).asset, &assets_before));
    }
}
#[test]
fn image_opacity_edits_merge_and_undo_without_touching_bytes() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let id = add(&mut c, s, [80., 100.]);
    let original = get(&c, id);
    let mut edited = (*original).clone();
    edited.opacity = 200;
    assert!(c.drawing.edit_image(edited.clone(), false));
    edited.opacity = 80;
    assert!(c.drawing.edit_image(edited, true));
    assert_eq!(get(&c, id).opacity, 80);
    c.drawing.undo();
    assert_eq!(get(&c, id), original);
    c.drawing.redo();
    assert_eq!(get(&c, id).opacity, 80);
    assert!(Arc::ptr_eq(&get(&c, id).asset, &original.asset));
}
#[test]
fn real_image_selection_drag_resize_offscreen_zoom_and_texture_reuse() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.5, 1., 2., 4.] {
            let (mut c, s) = setup(kind);
            c.snapping.objects = false;
            c.snapping.grid = false;
            let ctx = egui::Context::default();
            let id = add(&mut c, s, [180., 160.]);
            let original = get(&c, id);
            c.select(Tool::Select);
            let view = Transform {
                origin: [80., 80.],
                center: [180., 160.],
                scale,
            };
            frame(&ctx, &mut c, s, view, vec![], 0., true);
            click(&ctx, &mut c, s, view, [210., 180.], egui::Modifiers::NONE);
            assert!(c.selection.ids.contains(&id));
            let textures = c.image_cache.len();
            assert_eq!(textures, 1);
            let asset_id = original.asset.id();
            let texture_id = c.image_cache[&asset_id].texture.id();
            let start = [210., 180.];
            let end = [240., 200.];
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, start),
                    pointer(screen(view, start), true, egui::Modifiers::NONE),
                    moved(view, end),
                ],
                1.,
                true,
            );
            assert!(matches!(
                c.selection_gesture,
                Some(SelectionGesture::Transform { corner: None, .. })
            ));
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
                1.1,
                true,
            );
            let moved_image = get(&c, id);
            assert!((moved_image.placement.position[0] - 210.).abs() < 1e-4);
            assert_eq!(c.image_cache[&asset_id].texture.id(), texture_id);
            assert!(Arc::ptr_eq(&moved_image.asset, &original.asset));
            let handle = moved_image.handles()[4];
            let end = [handle[0] + 20., handle[1] + 15.];
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, handle),
                    pointer(screen(view, handle), true, egui::Modifiers::NONE),
                    moved(view, end),
                ],
                2.,
                true,
            );
            assert!(matches!(
                c.selection_gesture,
                Some(SelectionGesture::ImageResize { .. })
            ));
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
                2.1,
                true,
            );
            let resized = get(&c, id);
            assert_eq!(resized.dimensions, [140., 75.]);
            c.drawing.undo();
            assert_eq!(get(&c, id), moved_image);
            c.drawing.redo();
            assert_eq!(get(&c, id), resized);
            assert_eq!(c.image_cache[&asset_id].texture.id(), texture_id);
            // Whiteboard: move the view so most of the image is outside, then body-drag its visible sliver.
            if kind == DocumentKind::Whiteboard {
                let view = Transform {
                    origin: [-110. * scale, 70.],
                    center: resized.placement.position,
                    scale,
                };
                frame(&ctx, &mut c, s, view, vec![], 3., true);
                let p = [
                    resized.placement.position[0] + 118.,
                    resized.placement.position[1] + 30.,
                ];
                let end = [p[0] + 20., p[1] + 10.];
                frame(
                    &ctx,
                    &mut c,
                    s,
                    view,
                    vec![
                        moved(view, p),
                        pointer(screen(view, p), true, egui::Modifiers::NONE),
                        moved(view, end),
                    ],
                    3.1,
                    true,
                );
                frame(
                    &ctx,
                    &mut c,
                    s,
                    view,
                    vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
                    3.2,
                    true,
                );
                assert!(get(&c, id).placement.position[0] > resized.placement.position[0]);
            }
        }
    }
}
#[test]
fn image_selection_modifiers_area_lasso_alignment_and_z_order() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    c.select(Tool::Select);
    let a = add(&mut c, s, [100., 100.]);
    let b = add(&mut c, s, [350., 110.]);
    let ctx = egui::Context::default();
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    click(&ctx, &mut c, s, t(), [140., 130.], egui::Modifiers::NONE);
    assert_eq!(c.selection.ids, HashSet::from([a]));
    click(&ctx, &mut c, s, t(), [390., 140.], egui::Modifiers::SHIFT);
    assert_eq!(c.selection.ids, HashSet::from([a, b]));
    click(&ctx, &mut c, s, t(), [140., 130.], egui::Modifiers::ALT);
    assert_eq!(c.selection.ids, HashSet::from([b]));
    let bounds = get(&c, a).world_bounds().unwrap();
    assert!(get(&c, a).rectangle_matches(bounds, false));
    assert!(get(&c, a).rectangle_matches(
        ObjectBounds {
            min: [210., 150.],
            max: [240., 180.]
        },
        true
    ));
    let lasso = LassoRegion::new(bounds.expand(1.).corners().to_vec()).unwrap();
    assert!(get(&c, a).lasso_matches(&lasso));
    c.selection.change([a, b], Membership::Replace, &c.drawing);
    c.selection_action(SelectionCommand::Align(crate::snapping::Align::Left), &ctx);
    assert_eq!(
        get(&c, a).world_bounds().unwrap().min[0],
        get(&c, b).world_bounds().unwrap().min[0]
    );
    c.selection.change([a], Membership::Replace, &c.drawing);
    c.selection_action(SelectionCommand::BringToFront, &ctx);
    assert_eq!(c.drawing.items().last().unwrap().id(), a);
    let shape = Shape::create(
        c.drawing.owner(),
        s,
        ShapeKind::Rectangle,
        [50., 50.],
        [80., 80.],
        false,
    );
    if let Some(shape) = shape {
        let id = shape.id;
        assert!(c.drawing.insert_snapshots(vec![shape.into()]));
        c.selection.change([a, id], Membership::Replace, &c.drawing);
        c.selection_action(SelectionCommand::Group, &ctx);
        assert_eq!(c.selection.ids.len(), 1);
    }
}

fn drain(c: &mut Canvas, ctx: &egui::Context) {
    let start = std::time::Instant::now();
    while c.image_imports.pending > 0 && start.elapsed() < std::time::Duration::from_secs(5) {
        c.poll_images(ctx);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    c.poll_images(ctx);
    assert_eq!(c.image_imports.pending, 0);
}
#[test]
fn image_import_failures_replacement_and_screenshot_paste_preserve_state() {
    let (mut c, s) = setup(DocumentKind::Page);
    let ctx = egui::Context::default();
    let id = add(&mut c, s, [100., 100.]);
    let mut image = (*get(&c, id)).clone();
    image.opacity = 100;
    image.placement.rotation = 0.2;
    image.placement.reflected = [true, false];
    image.placement.position = [250., 200.];
    assert!(c.drawing.replace_image(image.clone()));
    let target = Target {
        surface: s,
        position: [300., 300.],
        replace: Some(id),
    };
    let revision = c.drawing.revision();
    c.image_imports
        .sender
        .send(crate::image_import::Imported {
            target,
            result: Err("Corrupted image".into()),
        })
        .unwrap();
    c.poll_images(&ctx);
    assert_eq!(c.drawing.revision(), revision);
    assert_eq!(get(&c, id).opacity, 100);
    let original_asset = get(&c, id).asset.clone();
    c.image_imports
        .sender
        .send(crate::image_import::Imported {
            target,
            result: ImageAsset::import(test_support::bytes(image::ImageFormat::Jpeg)),
        })
        .unwrap();
    c.poll_images(&ctx);
    let replaced = get(&c, id);
    assert_eq!(replaced.id, id);
    assert_eq!(replaced.placement, image.placement);
    assert_eq!(replaced.opacity, 100);
    assert_eq!(replaced.dimensions, image.dimensions);
    assert_ne!(replaced.asset.id(), original_asset.id());
    c.drawing.undo();
    assert!(Arc::ptr_eq(&get(&c, id).asset, &original_asset));
    c.drawing.redo();
    assert_eq!(get(&c, id).asset.id(), replaced.asset.id());
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let pixels = Arc::new(egui::ColorImage::filled(
        [64, 32],
        Color32::from_rgba_unmultiplied(10, 20, 30, 128),
    ));
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![Event::PasteImage(pixels)],
        0.1,
        true,
    );
    drain(&mut c, &ctx);
    assert_eq!(c.drawing.items().len(), 2);
    let screenshot = get(&c, *c.selection.ids.iter().next().unwrap());
    assert_eq!(screenshot.asset.dimensions(), [64, 32]);
    let decoded = screenshot.asset.decode().unwrap();
    assert_eq!(decoded.rgba[3], 128);
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_image_drop_uses_pointer_transform_without_moving_previous_selection() {
    #[derive(Debug)]
    struct File(std::path::PathBuf);
    impl egui::DroppedFile for File {
        fn path(&self) -> &std::path::Path {
            &self.0
        }
        fn bytes(&self) -> Result<Vec<u8>, String> {
            std::fs::read(&self.0).map_err(|e| e.to_string())
        }
    }
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("image.png");
    std::fs::write(&path, test_support::bytes(image::ImageFormat::Png)).unwrap();
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let id = add(&mut c, s, [100., 100.]);
    let original = get(&c, id);
    c.select(Tool::Select);
    c.selection.change([id], Membership::Replace, &c.drawing);
    let view = Transform {
        origin: [80., 80.],
        center: [100., 100.],
        scale: 2.,
    };
    frame(&ctx, &mut c, s, view, vec![], 0., true);
    let drop_point = [260., 220.];
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.))),
            events: vec![moved(view, drop_point)],
            dropped_files: vec![Arc::new(File(path))],
            focused: true,
            ..Default::default()
        },
        |ui| {
            c.begin_frame(ui);
            c.surface(ui, ui.clip_rect(), s, view);
            c.end_frame(ui);
        },
    );
    drop(output);
    drain(&mut c, &ctx);
    assert_eq!(get(&c, id), original);
    assert_eq!(c.drawing.items().len(), 2);
    let inserted = get(&c, *c.selection.ids.iter().next().unwrap());
    let center = inserted.world_bounds().unwrap().center();
    assert!((center[0] - 260.).abs() < 1e-9 && (center[1] - 220.).abs() < 1e-9);
    std::fs::remove_file(tmp.path().join("image.png")).unwrap();
    assert!(inserted.asset.decode().is_ok());
}
#[test]
fn image_asset_registry_interns_imports_and_rejects_history_memory_overflow() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let a = add(&mut c, s, [40., 40.]);
    let b = add(&mut c, s, [80., 80.]);
    assert!(Arc::ptr_eq(&get(&c, a).asset, &get(&c, b).asset));
    let mut image = (*get(&c, a)).clone();
    let mut accepted = 1;
    for n in 1..18 {
        let pixels = image::RgbaImage::from_pixel(2, 2, image::Rgba([n, 40, 50, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(pixels)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        image.asset = ImageAsset::import(bytes.into_inner().into()).unwrap().0;
        if c.drawing.replace_image(image.clone()) {
            accepted += 1;
        }
    }
    assert_eq!(accepted, 16);
    for _ in 0..15 {
        c.drawing.undo();
    }
    assert_eq!(get(&c, a).asset.id(), get(&c, b).asset.id());
}
#[test]
#[ignore = "manual release image movement/cache benchmark"]
fn image_overlap_drag_performance() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let asset = test_support::asset();
    let objects: Vec<_> = (0..1000)
        .map(|n| {
            let mut image = ImageObject::new(
                c.drawing.owner(),
                s,
                [100. + f64::from(n % 10), 120. + f64::from(n / 100)],
                asset.clone(),
            );
            image.dimensions = [120., 60.];
            image.into()
        })
        .collect();
    assert!(c.drawing.insert_snapshots(objects));
    c.select(Tool::Select);
    c.selection.change(
        c.drawing.items().iter().map(|o| o.id()).collect::<Vec<_>>(),
        Membership::Replace,
        &c.drawing,
    );
    c.snapping.objects = false;
    c.snapping.grid = false;
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let tex = c.image_cache[&asset.id()].texture.id();
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![
            moved(t(), [170., 150.]),
            pointer([170., 150.], true, egui::Modifiers::NONE),
        ],
        0.1,
        true,
    );
    assert!(matches!(
        c.selection_gesture,
        Some(SelectionGesture::Transform { corner: None, .. })
    ));
    let start = std::time::Instant::now();
    for i in 0..40 {
        frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![moved(t(), [170. + f64::from(i), 150. + f64::from(i)])],
            0.2 + f64::from(i) * 0.02,
            true,
        );
        assert_eq!(c.image_cache[&asset.id()].texture.id(), tex);
    }
    eprintln!(
        "1000 overlapping image objects, 40 changed frames: {:?}",
        start.elapsed()
    );
    assert_eq!(c.image_cache.len(), 1);
    assert!(
        c.drawing
            .items()
            .iter()
            .all(|o| matches!(o,ObjectSnapshot::Image(i) if Arc::ptr_eq(&i.asset,&asset)))
    );
}

fn assert_selection_paint(
    shapes: &[egui::epaint::ClippedShape],
    view: Transform,
    corners: [Point; 4],
    handles: &[Point],
    rotation: Point,
) {
    let near = |pos: Pos2, point: Point| {
        let [x, y] = view.to_screen(point);
        pos.distance(egui::pos2(x as f32, y as f32)) < 0.01
    };
    assert!(
        shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Path(path) if path.closed && path.points.len() == 4
                && path.points.iter().zip(corners).all(|(&pos, point)| near(pos, point))
        )),
        "selection outline did not match the preview"
    );
    let viewport = Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.));
    let visible = |point| {
        let [x, y] = view.to_screen(point);
        viewport.contains(egui::pos2(x as f32, y as f32))
    };
    for &handle in handles.iter().filter(|&&point| visible(point)) {
        assert!(
            shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Rect(rect)
                    if (rect.rect.width() - crate::ui::style::SELECTION_HANDLE).abs() < 0.001
                        && near(rect.rect.center(), handle)
            )),
            "missing resize handle at {handle:?}"
        );
    }
    assert!(
        !visible(rotation)
            || shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Circle(circle)
                    if circle.radius == crate::ui::style::SELECTION_HANDLE * 0.5
                        && near(circle.center, rotation)
            )),
        "missing rotation handle at {rotation:?}"
    );
}

fn assert_image_paint(
    canvas: &Canvas,
    shapes: &[egui::epaint::ClippedShape],
    view: Transform,
    expected: &ImageObject,
) {
    let texture = canvas.image_cache[&expected.asset.id()].texture.id();
    let corners = expected.corners();
    assert!(
        shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Mesh(mesh) if mesh.texture_id == texture
                && mesh.vertices.len() == 4
                && mesh.vertices.iter().zip(corners).all(|(v, p)| {
                    let [x, y] = view.to_screen(p);
                    v.pos.distance(egui::pos2(x as f32, y as f32)) < 0.01
                })
        )),
        "image mesh did not match the preview"
    );
}

fn assert_selected_image_paint(
    canvas: &Canvas,
    shapes: &[egui::epaint::ClippedShape],
    view: Transform,
    expected: &ImageObject,
) {
    let effective = canvas.effective_selected_image().unwrap();
    for (actual, target) in effective.handles().into_iter().zip(expected.handles()) {
        assert!((actual[0] - target[0]).hypot(actual[1] - target[1]) < 0.001);
    }
    assert_image_paint(canvas, shapes, view, expected);
    assert_selection_paint(
        shapes,
        view,
        expected.corners(),
        &expected.handles(),
        crate::selection::SelectionFrame::for_object(expected)
            .unwrap()
            .grip(f64::from(crate::ui::style::ROTATION_OFFSET) / view.scale)
            .1,
    );
}

#[test]
fn image_mesh_and_all_selection_grips_move_in_the_same_held_frame() {
    for scale in [0.5, 1., 2., 4.] {
        for angle in [0., 0.4] {
            let (mut c, s) = setup(DocumentKind::Whiteboard);
            c.snapping.objects = false;
            c.snapping.grid = false;
            c.select(Tool::Select);
            let id = add(&mut c, s, [100., 100.]);
            let mut original = (*get(&c, id)).clone();
            original.placement.rotation = angle;
            original.placement.reflected = [angle != 0., false];
            if scale == 4. {
                original.dimensions[0] = 220.;
            }
            if angle != 0. || scale == 4. {
                assert!(c.drawing.replace_image(original.clone()));
            }
            let ctx = egui::Context::default();
            let view = Transform {
                origin: [if angle == 0. { 195. } else { 320. }, 165.],
                center: [100., 100.],
                scale,
            };
            let start = original.placement.apply([60., 30.]);
            frame(&ctx, &mut c, s, view, vec![], 0., true);
            click(&ctx, &mut c, s, view, start, egui::Modifiers::NONE);
            let revision = c.drawing.revision();
            let held = frame_shapes(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, start),
                    pointer(screen(view, start), true, egui::Modifiers::NONE),
                ],
                1.,
                true,
            );
            assert_selected_image_paint(&c, &held, view, &original);
            let offset = if angle == 0. && scale == 1. {
                [150., 80.]
            } else {
                [76., 29.]
            };
            let end = [start[0] + offset[0], start[1] + offset[1]];
            let mut expected = original.clone();
            for (position, delta) in expected.placement.position.iter_mut().zip(offset) {
                *position += delta;
            }
            let held = frame_shapes(&ctx, &mut c, s, view, vec![moved(view, end)], 1.01, true);
            assert_selected_image_paint(&c, &held, view, &expected);
            let stationary = frame_shapes(&ctx, &mut c, s, view, vec![], 1.2, true);
            assert_selected_image_paint(&c, &stationary, view, &expected);
            assert_eq!(c.drawing.revision(), revision);
            assert_eq!(*get(&c, id), original);

            let escape = frame_shapes(
                &ctx,
                &mut c,
                s,
                view,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                1.3,
                true,
            );
            assert_selected_image_paint(&c, &escape, view, &original);
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
                1.4,
                true,
            );
            assert_eq!(c.drawing.revision(), revision);

            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, start),
                    pointer(screen(view, start), true, egui::Modifiers::NONE),
                    moved(view, end),
                ],
                2.,
                true,
            );
            let released = frame_shapes(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
                2.1,
                true,
            );
            assert_selected_image_paint(&c, &released, view, &expected);
            c.undo();
            assert_eq!(*get(&c, id), original);
            let undone = frame_shapes(&ctx, &mut c, s, view, vec![], 2.2, true);
            assert_selected_image_paint(&c, &undone, view, &original);
            c.redo();
            let redone = frame_shapes(&ctx, &mut c, s, view, vec![], 2.3, true);
            assert_selected_image_paint(&c, &redone, view, &expected);
        }
    }
}

#[test]
fn image_resize_and_rotation_paint_match_before_and_after_release() {
    for index in [Some(4), Some(3), None] {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        c.select(Tool::Select);
        c.snapping.objects = false;
        c.snapping.grid = false;
        let id = add(&mut c, s, [200., 200.]);
        let mut original = (*get(&c, id)).clone();
        original.placement.rotation = 0.35;
        original.placement.reflected = [false, true];
        assert!(c.drawing.replace_image(original.clone()));
        let ctx = egui::Context::default();
        let view = t();
        c.selection.change([id], Membership::Replace, &c.drawing);
        let bounds = original.world_bounds().unwrap();
        let (start, end, expected) = if let Some(index) = index {
            let start = original.handles()[index];
            let end = [start[0] + 38., start[1] - 18.];
            (start, end, original.resized(index, end, false))
        } else {
            let start = crate::selection::SelectionFrame::for_object(&original)
                .unwrap()
                .grip(f64::from(crate::ui::style::ROTATION_OFFSET) / view.scale)
                .1;
            let center = bounds.center();
            let end = [
                center[0] + center[1] - start[1],
                center[1] + start[0] - center[0],
            ];
            (
                start,
                end,
                original.transformed(Placement {
                    origin: center,
                    position: center,
                    rotation: std::f64::consts::FRAC_PI_2,
                    ..Placement::default()
                }),
            )
        };
        let held = frame_shapes(
            &ctx,
            &mut c,
            s,
            view,
            vec![
                moved(view, start),
                pointer(screen(view, start), true, egui::Modifiers::NONE),
                moved(view, end),
            ],
            1.,
            true,
        );
        assert_eq!(*get(&c, id), original);
        if index.is_some() {
            assert_selected_image_paint(&c, &held, view, &expected);
        } else {
            assert_image_paint(&c, &held, view, &expected);
            assert!(c.rotating(s));
        }
        let release = frame_shapes(
            &ctx,
            &mut c,
            s,
            view,
            vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
            1.1,
            true,
        );
        assert_selected_image_paint(&c, &release, view, &expected);
    }
}

#[test]
fn mixed_and_grouped_image_drag_paints_the_collective_preview() {
    for kind in ["image", "rectangle", "text", "sticky note", "group"] {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        c.select(Tool::Select);
        c.snapping.objects = false;
        c.snapping.grid = false;
        let id = add(&mut c, s, [100., 100.]);
        let original = get(&c, id);
        let owner = c.drawing.owner();
        let peer: ObjectSnapshot = match kind {
            "image" => {
                let peer = add(&mut c, s, [300., 140.]);
                get(&c, peer).as_ref().clone().into()
            }
            "text" => crate::text::TextObject::new(owner, s, [300., 140.], 120.).into(),
            "sticky note" => {
                Shape::create_sticky_note(owner, s, [300., 140.], [100., 100.], [255, 240, 100])
                    .unwrap()
                    .into()
            }
            _ => Shape::create(
                owner,
                s,
                ShapeKind::Rectangle,
                [300., 140.],
                [390., 205.],
                false,
            )
            .unwrap()
            .into(),
        };
        let peer_id = peer.id();
        if kind != "image" {
            assert!(c.drawing.insert_snapshots(vec![peer]));
        }
        let ctx = egui::Context::default();
        c.selection
            .change([id, peer_id], Membership::Replace, &c.drawing);
        if kind == "group" {
            c.selection_action(SelectionCommand::Group, &ctx);
        }
        let bounds = c.selection.bounds.unwrap();
        let view = t();
        let start = [140., 130.];
        let end = [290., 210.];
        let delta = Placement {
            position: [150., 80.],
            ..Placement::default()
        };
        let expected_bounds = bounds.placed(delta);
        let expected_frame = c.selection.frame.unwrap().transformed(delta);
        let expected_corners = expected_frame.corners();
        let held = frame_shapes(
            &ctx,
            &mut c,
            s,
            view,
            vec![
                moved(view, start),
                pointer(screen(view, start), true, egui::Modifiers::NONE),
                moved(view, end),
            ],
            1.,
            true,
        );
        assert_eq!(get(&c, id), original);
        assert_image_paint(&c, &held, view, &original.transformed(delta));
        assert_selection_paint(
            &held,
            view,
            expected_corners,
            &expected_corners,
            expected_frame
                .grip(f64::from(crate::ui::style::ROTATION_OFFSET) / view.scale)
                .1,
        );
        let release = frame_shapes(
            &ctx,
            &mut c,
            s,
            view,
            vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
            1.1,
            true,
        );
        assert_eq!(c.selection.bounds, Some(expected_bounds));
        assert_image_paint(&c, &release, view, &original.transformed(delta));
    }
}

#[test]
fn image_resize_reprojects_stationary_shift_changes_from_the_same_anchor() {
    for scale in [0.5, 1., 2., 4.] {
        for index in [3, 4] {
            let (mut c, s) = setup(DocumentKind::Whiteboard);
            c.select(Tool::Select);
            c.snapping.objects = false;
            c.snapping.grid = false;
            let id = add(&mut c, s, [110., 140.]);
            let mut original = (*get(&c, id)).clone();
            original.dimensions = [260., 90.];
            original.opacity = 177;
            original.placement.rotation = 0.27;
            original.placement.reflected = [index == 3, index == 4];
            assert!(c.drawing.replace_image(original.clone()));
            c.selection.change([id], Membership::Replace, &c.drawing);
            let ctx = egui::Context::default();
            let grip = original.handles()[index];
            let view = Transform {
                origin: [390., 295.],
                center: grip,
                scale,
            };
            // Grab off-center and move one fast sample in both local axes.
            let start = view.to_document([392., 296.]);
            let target = original
                .placement
                .apply([300., if index == 3 { 70. } else { 120. }]);
            let end = std::array::from_fn(|axis| start[axis] + (target[axis] - grip[axis]));
            let free = original.resized(index, target, false);
            let proportional = original.resized(index, target, true);
            assert_ne!(free.dimensions, proportional.dimensions);
            let revision = c.drawing.revision();
            let held = frame_shapes(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, start),
                    pointer(screen(view, start), true, egui::Modifiers::NONE),
                    moved(view, end),
                ],
                0.,
                true,
            );
            assert_selected_image_paint(&c, &held, view, &free);
            frame(&ctx, &mut c, s, view, vec![Event::PointerGone], 0.05, true);
            for (frame_index, modifiers) in [
                egui::Modifiers::SHIFT,
                egui::Modifiers::NONE,
                egui::Modifiers::SHIFT,
                egui::Modifiers::NONE,
                egui::Modifiers::SHIFT,
            ]
            .into_iter()
            .enumerate()
            {
                let output = frame_shapes(
                    &ctx,
                    &mut c,
                    s,
                    view,
                    vec![Event::ModifiersChanged(modifiers)],
                    0.1 + frame_index as f64 * 0.01,
                    true,
                );
                let expected = if modifiers.shift {
                    &proportional
                } else {
                    &free
                };
                assert_selected_image_paint(&c, &output, view, expected);
                let candidate = c.effective_selected_image().unwrap();
                if scale == 1.0 {
                    let id = egui::Id::unique("selection-context")
                        .with(c.drawing.owner())
                        .with(s);
                    let controls = ctx.memory(|memory| memory.area_rect(id)).unwrap();
                    let center = screen(view, candidate.world_bounds().unwrap().center());
                    assert!((f64::from(controls.center().x) - center[0]).abs() < 1.0);
                }
                let anchor = (index + 4) % 8;
                let [x, y] = original.handles()[anchor];
                let [a, b] = candidate.handles()[anchor];
                assert!((x - a).hypot(y - b) < 1e-8);
                assert_eq!(candidate.opacity, original.opacity);
                assert!(Arc::ptr_eq(&candidate.asset, &original.asset));
                assert_eq!(c.drawing.revision(), revision);
            }
            let released = frame_shapes(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(screen(view, end), false, egui::Modifiers::SHIFT)],
                1.,
                true,
            );
            assert_selected_image_paint(&c, &released, view, &proportional);
            let committed = get(&c, id);
            assert_eq!(c.drawing.revision(), revision + 1);
            c.undo();
            assert_eq!(*get(&c, id), original);
            c.redo();
            assert_eq!(get(&c, id), committed);
            assert_eq!(committed.asset.bytes(), original.asset.bytes());

            let grip = committed.handles()[index];
            let end = [grip[0] + 16., grip[1] - 8.];
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, grip),
                    pointer(screen(view, grip), true, egui::Modifiers::SHIFT),
                    moved(view, end),
                ],
                2.,
                true,
            );
            assert!(matches!(
                c.selection_gesture,
                Some(SelectionGesture::ImageResize { moved: true, .. })
            ));
            let revision = c.drawing.revision();
            let escaped = frame_shapes(
                &ctx,
                &mut c,
                s,
                view,
                vec![Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                2.1,
                true,
            );
            assert_selected_image_paint(&c, &escaped, view, &committed);
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
                2.2,
                true,
            );
            assert_eq!(c.drawing.revision(), revision);
            assert_eq!(get(&c, id), committed);
        }
    }
}

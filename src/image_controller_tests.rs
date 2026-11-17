use super::*;
use crate::canvas::selection_ui::tests::{click, frame, moved, pointer, screen, setup, t};
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
        let restored = ObjectBundle::decode(&encoded).unwrap();
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
fn image_opacity_and_aspect_edits_merge_and_undo_without_touching_bytes() {
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
    let mut edited = (*get(&c, id)).clone();
    edited.preserve_aspect = false;
    assert!(c.drawing.replace_image(edited));
    c.drawing.undo();
    assert!(get(&c, id).preserve_aspect);
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
            let end = [handle[0] + 20., handle[1] + 10.];
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
            assert!((resized.dimensions[0] / resized.dimensions[1] - 2.).abs() < 1e-8);
            assert!(resized.dimensions[0] > 120.);
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

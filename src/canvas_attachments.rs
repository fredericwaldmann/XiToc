//! Canvas card rendering and commands. Content never enters the render path.
use super::*;
use crate::{
    attachment::{Attachment, valid_name},
    attachment_io::ResultEvent,
    object::Placement,
};
use std::sync::Arc;

impl Canvas {
    pub(crate) fn choose_attachment(&mut self, ctx: &egui::Context, replace: Option<Uuid>) {
        self.finish();
        self.attachment_io.pick(
            ctx.clone(),
            self.image_target(replace),
            self.drawing.editing_group,
        );
    }

    pub(super) fn attachment(&self, id: Uuid) -> Option<Arc<Attachment>> {
        match self.drawing.snapshot(id)? {
            ObjectSnapshot::Attachment(attachment) => Some(attachment.clone()),
            _ => None,
        }
    }

    pub(super) fn selected_attachment(&self) -> Option<Arc<Attachment>> {
        if self.selection.ids.len() != 1 {
            return None;
        }
        self.attachment(*self.selection.ids.iter().next()?)
    }

    pub(super) fn poll_attachments(&mut self) {
        if self.text_session.is_some() || self.formula_session.is_some() {
            return;
        }
        while let Ok(event) = self.attachment_io.rx.try_recv() {
            match event {
                ResultEvent::PickerClosed => self.attachment_io.picker_open = false,
                ResultEvent::Done(message) => self.notice = Some(message),
                ResultEvent::Import(target, scope, result, dropped) => {
                    if dropped {
                        self.attachment_io.pending = self.attachment_io.pending.saturating_sub(1);
                    }
                    if target.replace.is_none() && scope != self.drawing.editing_group {
                        self.notice = Some(
                            "Attachment import cancelled because the group editing scope changed."
                                .into(),
                        );
                        continue;
                    }
                    match result {
                        Ok((name, asset)) => self.insert_attachment(target, name, asset),
                        Err(error) => self.notice = Some(error),
                    }
                }
            }
        }
    }

    fn insert_attachment(
        &mut self,
        target: crate::image_import::Target,
        name: String,
        asset: Arc<crate::asset::BinaryAsset>,
    ) {
        if let Some(id) = target.replace {
            let Some(old) = self.attachment(id) else {
                self.notice = Some("The attachment to replace is no longer available.".into());
                return;
            };
            let mut next = (*old).clone();
            if next.display_name == next.original_name {
                next.display_name = name.clone();
            }
            next.original_name = name;
            next.asset = asset;
            self.notice = Some(
                if next == *old {
                    "Attachment already contains this file."
                } else if self.drawing.replace_attachment(next) {
                    "Embedded attachment replaced."
                } else {
                    "Cannot replace the attachment. Check locking and document asset limits."
                }
                .into(),
            );
            return;
        }
        let mut attachment = Attachment::new(
            self.drawing.owner(),
            target.surface,
            target.position,
            name,
            asset,
        );
        for (axis, position) in attachment.placement.position.iter_mut().enumerate() {
            *position -= attachment.dimensions[axis] / 2.;
            if let Some(page) = self.drawing.bounds(target.surface) {
                *position = position.clamp(0., (page[axis] - attachment.dimensions[axis]).max(0.));
            }
        }
        let id = attachment.id;
        if self.drawing.insert_snapshots(vec![attachment.into()]) {
            self.select(self.selection_mode);
            self.selection.scope(target.surface);
            self.selection
                .change([id], crate::selection::Membership::Replace, &self.drawing);
            self.notice = Some("File attached · unsaved until PDF persistence (M2).".into());
        } else {
            self.notice = Some(
                "Cannot attach file. Check page bounds and the 16-asset / 128 MiB document limit."
                    .into(),
            );
        }
    }

    pub(super) fn attachment_resize_preview(&self) -> Option<&Attachment> {
        match &self.selection_gesture {
            Some(SelectionGesture::AttachmentResize { preview, .. }) => Some(preview),
            _ => None,
        }
    }

    pub(super) fn attachment_handle(
        &self,
        point: [f64; 2],
        scale: f64,
    ) -> Option<(Arc<Attachment>, usize)> {
        let source = self.selected_attachment()?;
        if self.drawing.is_locked(source.id) {
            return None;
        }
        let index = source.handles().iter().position(|p| {
            (p[0] - point[0]).hypot(p[1] - point[1]) * scale
                <= f64::from(crate::ui::style::SELECTION_HIT)
        })?;
        Some((source, index))
    }

    pub(super) fn paint_attachment(
        ui: &egui::Ui,
        painter: &egui::Painter,
        view: Transform,
        attachment: &Attachment,
        delta: Placement,
    ) {
        let style = crate::ui::style::palette(ui);
        let to_screen = |p| {
            let p = view.to_screen(delta.apply(attachment.placement.apply(p)));
            Pos2::new(p[0] as f32, p[1] as f32)
        };
        let [w, h] = attachment.dimensions;
        let corners = [[0., 0.], [w, 0.], [w, h], [0., h]].map(to_screen);
        if !painter.clip_rect().intersects(Rect::from_points(&corners)) {
            return;
        }
        painter.add(egui::Shape::convex_polygon(
            corners.to_vec(),
            style.surface,
            Line::new(1., style.border),
        ));
        let pixel_scale = (view.scale * attachment.placement.scale * delta.scale) as f32;
        let placement = attachment.placement.then(delta, [0.; 2]);
        let bounds = Rect::from_points(&corners).intersect(painter.clip_rect());
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos())
            && bounds.contains(pos)
            && attachment
                .local_bounds()
                .unwrap()
                .contains(placement.inverse(view.to_document([f64::from(pos.x), f64::from(pos.y)])))
        {
            ui.interact(
                bounds,
                egui::Id::unique("attachment-tooltip").with(attachment.id),
                egui::Sense::hover(),
            )
            .on_hover_text(format!(
                "{}\n{} bytes",
                attachment.original_name,
                attachment.asset.bytes().len()
            ));
        }
        let font_size = (13. * pixel_scale).min(128.);
        if font_size < 3. {
            return;
        }
        // Keep names readable on reflected cards. Rotation follows the card.
        let readable = |x, y| {
            to_screen([
                if placement.reflected[0] { w - x } else { x },
                if placement.reflected[1] { h - y } else { y },
            ])
        };
        let index = match attachment.extension().as_str() {
            "xls" | "xlsx" | "ods" | "csv" => 1,
            "zip" | "tar" | "7z" | "gz" | "rar" => 2,
            "png" | "jpg" | "jpeg" | "webp" => 3,
            _ => 0,
        };
        let mark = |a: (f64, f64), b: (f64, f64)| {
            painter.line_segment(
                [readable(a.0, h * 0.5 + a.1), readable(b.0, h * 0.5 + b.1)],
                Line::new(1.2, style.accent),
            )
        };
        for (a, b) in [
            ((9., -7.), (20., -7.)),
            ((20., -7.), (20., 7.)),
            ((20., 7.), (9., 7.)),
            ((9., 7.), (9., -7.)),
        ] {
            mark(a, b);
        }
        match index {
            1 => {
                mark((10., 0.), (19., 0.));
                mark((14., -6.), (14., 6.));
            }
            2 => {
                mark((14., -6.), (14., 2.));
                mark((14., 2.), (17., 2.));
            }
            3 => {
                mark((10., 5.), (13., 0.));
                mark((13., 0.), (19., 5.));
            }
            _ => {
                mark((11., -2.), (18., -2.));
                mark((11., 2.), (17., 2.));
            }
        }
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = ((w - 35.) as f32 * pixel_scale).max(1.);
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.append(
            &attachment.display_name,
            0.,
            egui::TextFormat::simple(egui::FontId::proportional(font_size), style.text),
        );
        let galley = painter.layout_job(job);
        let center_y = (h - f64::from(galley.size().y / pixel_scale)) * 0.5;
        painter.add(
            egui::epaint::TextShape::new(readable(28., center_y), galley, style.text)
                .with_angle(placement.rotation as f32),
        );
    }

    pub(super) fn attachment_menu(&mut self, ui: &mut egui::Ui, attachment: Arc<Attachment>) {
        let label = if cfg!(target_arch = "wasm32") {
            "Download"
        } else {
            "Open"
        };
        if ui.button(label).clicked() {
            self.attachment_io
                .open(ui.ctx().clone(), attachment.clone());
            ui.close();
        }
        if !cfg!(target_arch = "wasm32") && ui.button("Save As…").clicked() {
            self.attachment_io
                .save(ui.ctx().clone(), attachment.clone());
            ui.close();
        }
        ui.add_enabled_ui(!self.drawing.is_locked(attachment.id), |ui| {
            if ui.button("Rename Display Name…").clicked() {
                self.attachment_rename = Some((attachment.id, attachment.display_name.clone()));
                ui.close();
            }
            if ui.button("Replace Attached File…").clicked() {
                self.choose_attachment(ui.ctx(), Some(attachment.id));
                ui.close();
            }
        });
        ui.menu_button("File Information", |ui| {
            ui.label(&attachment.original_name);
            ui.label(format!("{} bytes", attachment.asset.bytes().len()));
        });
    }

    pub(super) fn attachment_rename_dialog(&mut self, ctx: &egui::Context) -> bool {
        let Some((id, name)) = self.attachment_rename.as_mut() else {
            return false;
        };
        let id = *id;
        let mut apply = false;
        let mut cancel = false;
        let response =
            egui::Modal::new(egui::Id::unique("attachment-rename").with(id)).show(ctx, |ui| {
                ui.heading("Rename attachment");
                ui.text_edit_singleline(name);
                ui.horizontal(|ui| {
                    apply = ui
                        .add_enabled(valid_name(name), egui::Button::new("Rename"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if apply {
            let (_, name) = self.attachment_rename.take().unwrap();
            if let Some(mut existing) = self.attachment(id).map(|a| (*a).clone()) {
                existing.display_name = name;
                self.drawing.replace_attachment(existing);
            }
        } else if cancel || response.should_close() {
            self.attachment_rename = None;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        asset::BinaryAsset,
        canvas::selection_ui::tests::{frame, moved, pointer, screen, setup},
        document::DocumentKind,
        image_import::Target,
        object_clipboard::ObjectBundle,
        selection::{Membership, SelectionCommand},
    };

    #[test]
    fn attachments_share_image_bytes_and_round_trip_across_documents_and_history() {
        let (mut c, surface) = setup(DocumentKind::Whiteboard);
        let image_asset = crate::image_object::test_support::asset();
        let image = crate::image_object::ImageObject::new(
            c.drawing.owner(),
            surface,
            [240., 150.],
            image_asset.clone(),
        );
        assert!(c.drawing.insert_snapshots(vec![image.into()]));
        let target = Target {
            surface,
            position: [180., 210.],
            replace: None,
        };
        c.insert_attachment(
            target,
            "original.png".into(),
            BinaryAsset::new(image_asset.bytes().into()).unwrap(),
        );
        let original = c.selected_attachment().unwrap();
        assert!(Arc::ptr_eq(&original.asset, &image_asset.binary));
        c.insert_attachment(
            target,
            "same-content.txt".into(),
            BinaryAsset::new(image_asset.bytes().into()).unwrap(),
        );
        let second = c.selected_attachment().unwrap();
        assert!(Arc::ptr_eq(&second.asset, &original.asset));
        assert_ne!(second.id, original.id);
        assert_ne!(second.original_name, original.original_name);
        let ctx = egui::Context::default();
        c.selection_action(SelectionCommand::SelectAll, &ctx);
        c.selection_action(SelectionCommand::Group, &ctx);
        let group = *c.selection.ids.iter().next().unwrap();
        let bundle = ObjectBundle::capture_objects(c.drawing.items().iter().cloned()).unwrap();
        let encoded = bundle.encode().unwrap();
        let wire: serde_json::Value = serde_json::from_str(
            encoded
                .strip_prefix(crate::object_clipboard::PREFIX)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(wire["version"], 3);
        assert_eq!(wire["assets"].as_array().unwrap().len(), 1);
        let mut corrupt = wire;
        corrupt["assets"][0]["bytes"] = "AA==".into();
        assert!(
            ObjectBundle::decode(&format!("{}{}", crate::object_clipboard::PREFIX, corrupt))
                .is_err()
        );
        let (mut other, target_surface) = setup(DocumentKind::Page);
        let new_roots = ObjectBundle::decode(&encoded).unwrap().paste(
            &mut other.drawing,
            target_surface,
            [250., 250.],
        );
        assert_eq!(new_roots.len(), 1);
        let pasted = other
            .drawing
            .items()
            .iter()
            .find_map(|o| match o {
                ObjectSnapshot::Attachment(a) if a.original_name == original.original_name => {
                    Some(a.clone())
                }
                _ => None,
            })
            .unwrap();
        assert_ne!(pasted.id, original.id);
        assert_ne!(other.drawing.parent(pasted.id), Some(group));
        assert_eq!(pasted.document_id, other.drawing.owner());
        assert_eq!(pasted.surface, target_surface);
        assert_eq!(pasted.asset.bytes(), original.asset.bytes());
        assert!(other.drawing.items().iter().any(
            |o| matches!(o, ObjectSnapshot::Image(i) if Arc::ptr_eq(&i.asset.binary, &pasted.asset))
        ));
        drop(c);
        other.drawing.undo();
        assert!(other.drawing.items().is_empty());
        other.drawing.redo();
        assert!(Arc::ptr_eq(
            &pasted.asset,
            &other.attachment(pasted.id).unwrap().asset
        ));
        other.insert_attachment(
            Target {
                surface: target_surface,
                position: [0.; 2],
                replace: Some(pasted.id),
            },
            "update.dat".into(),
            BinaryAsset::new(b"new bytes".as_slice().into()).unwrap(),
        );
        assert_eq!(
            other.attachment(pasted.id).unwrap().asset.bytes(),
            b"new bytes"
        );
        other.drawing.undo();
        assert_eq!(other.attachment(pasted.id).unwrap(), pasted);
    }

    #[test]
    fn oriented_attachment_resize_and_move_preserve_binary_and_history() {
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            for scale in [0.5, 1., 2., 4.] {
                let (mut c, surface) = setup(kind);
                let asset = BinaryAsset::new(b"immutable".as_slice().into()).unwrap();
                let mut source = Attachment::new(
                    c.drawing.owner(),
                    surface,
                    [150., 220.],
                    "a.txt".into(),
                    asset.clone(),
                );
                source.placement.rotation = 0.45;
                let id = source.id;
                assert!(c.drawing.insert_snapshots(vec![source.clone().into()]));
                c.select(Tool::Select);
                c.selection.change([id], Membership::Replace, &c.drawing);
                c.snapping.grid = false;
                let view = Transform {
                    origin: [400., 265.],
                    center: source.handles()[3],
                    scale,
                };
                let ctx = egui::Context::default();
                frame(&ctx, &mut c, surface, view, vec![], 0., true);
                let start = source.handles()[3];
                let target = source.placement.apply([210., 18.]);
                frame(
                    &ctx,
                    &mut c,
                    surface,
                    view,
                    vec![
                        moved(view, start),
                        pointer(screen(view, start), true, egui::Modifiers::NONE),
                        moved(view, target),
                    ],
                    1.,
                    true,
                );
                let preview = c.attachment_resize_preview().unwrap().clone();
                assert!((preview.dimensions[0] - 210.).abs() < 1e-4);
                assert_eq!(preview.dimensions[1], source.dimensions[1]);
                assert_eq!(preview.handles()[7], source.handles()[7]);
                assert!(Arc::ptr_eq(&preview.asset, &asset));
                frame(
                    &ctx,
                    &mut c,
                    surface,
                    view,
                    vec![pointer(screen(view, target), false, egui::Modifiers::NONE)],
                    1.1,
                    true,
                );
                assert_eq!(*c.attachment(id).unwrap(), preview);
                c.undo();
                assert_eq!(*c.attachment(id).unwrap(), source);
                c.redo();
                assert_eq!(*c.attachment(id).unwrap(), preview);
                let center = preview.world_bounds().unwrap().center();
                let moved_to = [center[0] + 10., center[1] - 8.];
                frame(
                    &ctx,
                    &mut c,
                    surface,
                    view,
                    vec![
                        moved(view, center),
                        pointer(screen(view, center), true, egui::Modifiers::NONE),
                        moved(view, moved_to),
                        pointer(screen(view, moved_to), false, egui::Modifiers::NONE),
                    ],
                    2.,
                    true,
                );
                let after_move = c.attachment(id).unwrap();
                let position = preview.placement.position;
                assert!((after_move.placement.position[0] - (position[0] + 10.)).abs() < 1e-4);
                assert!((after_move.placement.position[1] - (position[1] - 8.)).abs() < 1e-4);
                assert!(Arc::ptr_eq(&after_move.asset, &asset));
                c.selection_action(SelectionCommand::RotateRight, &ctx);
                let rotated = c.attachment(id).unwrap();
                assert_ne!(rotated.placement.rotation, preview.placement.rotation);
                assert!(Arc::ptr_eq(&rotated.asset, &asset));
                assert!(c.drawing.lock_objects(&HashSet::from([id]), true));
                assert!(!c.drawing.replace_attachment(source));
            }
        }
    }
}

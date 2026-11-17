//! Image import integration and cached texture painting in the shared canvas.
use super::*;
use crate::{
    image_import::Target,
    image_object::{AssetId, DecodedImage, ImageAsset, ImageObject},
    object::{Placement, Point},
    selection::Membership,
};

pub(super) struct ImageTexture {
    texture: egui::TextureHandle,
    asset: Arc<ImageAsset>,
}
impl ImageTexture {
    fn new(ctx: &egui::Context, asset: Arc<ImageAsset>, mut decoded: DecodedImage) -> Self {
        let limit = ctx.input(|i| i.max_texture_side).clamp(1, 2048);
        if decoded.size.iter().any(|n| *n > limit) {
            let rgba = image::RgbaImage::from_raw(
                decoded.size[0] as u32,
                decoded.size[1] as u32,
                decoded.rgba,
            )
            .unwrap();
            let resized = image::DynamicImage::ImageRgba8(rgba)
                .resize(
                    limit as u32,
                    limit as u32,
                    image::imageops::FilterType::Triangle,
                )
                .into_rgba8();
            decoded = DecodedImage {
                size: [resized.width() as usize, resized.height() as usize],
                rgba: resized.into_raw(),
            };
        }
        let image = egui::ColorImage::from_rgba_unmultiplied(decoded.size, &decoded.rgba);
        Self {
            texture: ctx.load_texture(
                format!("image-{:x?}", asset.id()),
                image,
                egui::TextureOptions::LINEAR,
            ),
            asset,
        }
    }
}
impl Canvas {
    pub(super) fn image_target(&self, replace: Option<Uuid>) -> Target {
        let (surface, position) = self
            .selection_view
            .map(|(s, t, b)| (s, t.to_document([b.center().x as f64, b.center().y as f64])))
            .unwrap_or((self.selection.surface, [100., 100.]));
        Target {
            surface,
            position,
            replace,
        }
    }
    pub(crate) fn choose_image(&mut self, ctx: &egui::Context, replace: Option<Uuid>) {
        self.finish();
        let target = self.image_target(replace);
        self.image_imports.picker(ctx.clone(), target);
        self.notice = Some("Choose PNG, JPEG or WebP · images remain unsaved until M2.".into());
    }
    fn image_budget(&self, asset: &ImageAsset) -> bool {
        let mut assets = HashMap::new();
        for object in self.drawing.items() {
            if let ObjectSnapshot::Image(i) = object {
                assets.entry(i.asset.id()).or_insert(i.asset.bytes().len());
            }
        }
        if assets.contains_key(&asset.id()) {
            return true;
        }
        assets.len() < 16
            && assets.values().sum::<usize>() + asset.bytes().len() <= 128 * 1024 * 1024
    }
    pub(super) fn poll_images(&mut self, ctx: &egui::Context) {
        if self.text_session.is_some() || self.formula_session.is_some() {
            return;
        }
        while let Ok(imported) = self.image_imports.receiver.try_recv() {
            self.image_imports.pending = self.image_imports.pending.saturating_sub(1);
            match imported.result {
                Err(error) => self.notice = Some(error),
                Ok((mut asset, decoded)) => {
                    if !self.image_budget(&asset) {
                        self.notice = Some(
                            "Image limit reached: 16 unique assets / 128 MiB per document.".into(),
                        );
                        continue;
                    }
                    // Reuse the immutable asset as well as its texture on repeated imports.
                    if let Some(existing) = self.image_cache.get(&asset.id()) {
                        asset = existing.asset.clone();
                    }
                    let target = imported.target;
                    let mut image = if let Some(id) = target.replace {
                        let Some(ObjectSnapshot::Image(old)) = self.drawing.snapshot(id) else {
                            continue;
                        };
                        if self.drawing.is_locked(id) {
                            continue;
                        }
                        let mut next = (**old).clone();
                        next.asset = asset.clone();
                        if next.preserve_aspect {
                            let d = asset.dimensions();
                            next.dimensions[1] =
                                next.dimensions[0] * f64::from(d[1]) / f64::from(d[0]);
                        }
                        next
                    } else {
                        let mut next = ImageObject::new(
                            self.drawing.owner(),
                            target.surface,
                            [0.; 2],
                            asset.clone(),
                        );
                        next.placement.position =
                            std::array::from_fn(|i| target.position[i] - next.dimensions[i] * 0.5);
                        next
                    };
                    if target.replace.is_none() {
                        fit_image(&mut image, self.drawing.bounds(target.surface));
                    }
                    let id = image.id;
                    let changed = if target.replace.is_some() {
                        self.drawing.replace_image(image)
                    } else {
                        self.drawing.insert_snapshots(vec![image.into()])
                    };
                    if changed {
                        self.image_cache
                            .entry(asset.id())
                            .or_insert_with(|| ImageTexture::new(ctx, asset, decoded));
                        self.finish();
                        self.selection.scope(target.surface);
                        self.selection
                            .change([id], Membership::Replace, &self.drawing);
                        self.tool = self.selection_mode;
                        self.notice =
                            Some("Image inserted · memory-only until PDF persistence (M2).".into());
                    } else {
                        self.notice = Some("Cannot insert/replace image at this location.".into());
                    }
                }
            }
        }
        if self.image_cache_revision == Some(self.drawing.revision()) {
            return;
        }
        self.image_cache_revision = Some(self.drawing.revision());
        let live: HashSet<_> = self
            .drawing
            .items()
            .iter()
            .filter_map(|o| {
                if let ObjectSnapshot::Image(i) = o {
                    Some(i.asset.id())
                } else {
                    None
                }
            })
            .collect();
        self.image_cache.retain(|id, _| live.contains(id));
    }
    pub(super) fn image_surface(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        view: Transform,
    ) -> bool {
        if !ui.is_enabled() || self.text_session.is_some() || self.formula_session.is_some() {
            return false;
        }
        let rect = bounds.intersect(ui.clip_rect());
        if !ui.input(|i| i.raw.hovered_files.is_empty()) {
            let p = crate::ui::style::palette(ui);
            ui.painter().rect_stroke(
                rect.shrink(4.),
                4.,
                Line::new(2., p.accent),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop PNG, JPEG or WebP to insert an image",
                egui::FontId::proportional(14.),
                p.text,
            );
        }
        #[cfg(target_arch = "wasm32")]
        if !ui.ctx().text_edit_focused() {
            let target = Target {
                surface,
                position: view.to_document([rect.center().x as f64, rect.center().y as f64]),
                replace: None,
            };
            ui.ctx().data_mut(|d| {
                d.insert_temp(
                    crate::image_import::paste_id(),
                    crate::image_import::BrowserPasteDestination {
                        sender: self.image_imports.sender.clone(),
                        target,
                    },
                )
            });
        }
        let pos = ui.input(|i| i.pointer.hover_pos().or(i.pointer.latest_pos()));
        let Some(pos) = pos.filter(|p| rect.contains(*p)) else {
            return false;
        };
        if ui.ctx().layer_id_at(pos) != Some(ui.layer_id()) {
            return false;
        }
        let files = ui.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        if files.is_empty() {
            if !ui.input(|i| i.raw.hovered_files.is_empty()) {
                self.finish();
                return true;
            }
            return false;
        }
        self.finish();
        let position = view.to_document([pos.x as f64, pos.y as f64]);
        for (index, file) in files.into_iter().take(4).enumerate() {
            self.image_imports.drop_file(
                ui.ctx().clone(),
                Target {
                    surface,
                    position: position.map(|v| v + index as f64 * 16.),
                    replace: None,
                },
                file,
            );
        }
        true
    }
    pub(super) fn paint_image(
        cache: &mut HashMap<AssetId, ImageTexture>,
        painter: &egui::Painter,
        view: Transform,
        image: &ImageObject,
    ) {
        let corners = image.corners();
        let screen = corners.map(|p| {
            let p = view.to_screen(p);
            Pos2::new(p[0] as f32, p[1] as f32)
        });
        let rect = Rect::from_points(&screen);
        if !painter.clip_rect().intersects(rect) {
            return;
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = cache.entry(image.asset.id()) {
            let Ok(decoded) = image.asset.decode() else {
                return;
            };
            entry.insert(ImageTexture::new(
                painter.ctx(),
                image.asset.clone(),
                decoded,
            ));
        }
        let texture = &cache[&image.asset.id()];
        let mut mesh = egui::Mesh::with_texture(texture.texture.id());
        for (pos, uv) in screen
            .into_iter()
            .zip([[0., 0.], [1., 0.], [1., 1.], [0., 1.]])
        {
            mesh.vertices.push(egui::epaint::Vertex {
                pos,
                uv: Pos2::new(uv[0], uv[1]),
                color: Color32::from_white_alpha(image.opacity),
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
    }
    pub(crate) fn selected_image(&self) -> Option<Arc<ImageObject>> {
        if self.selection.ids.len() != 1 {
            return None;
        }
        match self.drawing.snapshot(*self.selection.ids.iter().next()?) {
            Some(ObjectSnapshot::Image(i)) => Some(i.clone()),
            _ => None,
        }
    }
    pub(crate) fn image_controls(&mut self, ui: &mut egui::Ui) {
        let Some(source) = self.selected_image() else {
            return;
        };
        ui.add_enabled_ui(!self.drawing.is_locked(source.id),|ui| {
            let mut image=(*source).clone();let mut changed=false;
            let mut opacity=f32::from(image.opacity)/255.;
            let response=ui.add(egui::Slider::new(&mut opacity,0.0..=1.0).text("Opacity"));
            if response.changed(){image.opacity=(opacity*255.).round() as u8;changed=true;}
            changed|=ui.checkbox(&mut image.preserve_aspect,"Preserve aspect ratio").changed();
            if ui.button("Reset dimensions").clicked(){
                let natural=ImageObject::new(image.document_id,image.surface,[0.;2],image.asset.clone());image.dimensions=natural.dimensions;image.placement.scale=1.;changed=true;
            }
            if changed {let page=self.drawing.bounds(image.surface);fit_image(&mut image,page);let merge=self.style_gesture==Some((response.id,self.drawing.revision()));
                if self.drawing.edit_image(image,merge) {self.style_gesture=ui.input(|i|i.pointer.primary_down()).then_some((response.id,self.drawing.revision()));}
                self.selection.refresh(&self.drawing);}
            if ui.button("Replace image source…").clicked(){self.choose_image(ui.ctx(),Some(source.id));ui.close();}
            ui.weak("Images are memory-only until M2. Corners preserve aspect ratio; unlock above for independent edges.");
        });
    }
    pub(super) fn image_handle(&self, p: Point, scale: f64) -> Option<(Arc<ImageObject>, usize)> {
        let source = self.selected_image()?;
        if self.drawing.is_locked(source.id) {
            return None;
        }
        self.effective_selected_image()?
            .handles()
            .iter()
            .position(|h| {
                (h[0] - p[0]).hypot(h[1] - p[1]) * scale
                    <= f64::from(crate::ui::style::SELECTION_HIT)
            })
            .map(|index| (source, index))
    }
    pub(super) fn image_resize_preview(&self) -> Option<&ImageObject> {
        match &self.selection_gesture {
            Some(SelectionGesture::ImageResize { preview, .. }) => Some(preview),
            _ => None,
        }
    }
    // Resolve the same original geometry and live placement for both the mesh and
    // selection UI. Intrinsic resize already carries its effective placement.
    pub(super) fn effective_image(
        source: &ImageObject,
        resize: Option<&ImageObject>,
        placement: Option<Placement>,
    ) -> ImageObject {
        if let Some(preview) = resize.filter(|preview| preview.id == source.id) {
            preview.clone()
        } else if let Some(delta) = placement {
            source.transformed(delta)
        } else {
            source.clone()
        }
    }
    pub(super) fn effective_selected_image(&self) -> Option<ImageObject> {
        let source = self.selected_image()?;
        let placement = self.selection_preview(source.surface).filter(|_| {
            selection_ui::selection_preview_ids(&self.selection_gesture, &self.selection)
                .contains(&source.id)
        });
        Some(Self::effective_image(
            &source,
            self.image_resize_preview(),
            placement,
        ))
    }
    pub(super) fn image_rotation_geometry(image: &ImageObject, view_scale: f64) -> (Point, Point) {
        let center = image
            .placement
            .apply(image.dimensions.map(|size| size * 0.5));
        let (sin, cos) = image.placement.rotation.sin_cos();
        let outward = [sin, -cos];
        let half_height = image.dimensions[1] * image.placement.scale * 0.5;
        let stem = std::array::from_fn(|i| center[i] + outward[i] * half_height);
        let offset = f64::from(crate::ui::style::ROTATION_OFFSET) / view_scale;
        let handle = std::array::from_fn(|i| stem[i] + outward[i] * offset);
        (stem, handle)
    }
    pub(super) fn paint_image_handles(
        &self,
        painter: &egui::Painter,
        view: Transform,
        image: &ImageObject,
        fill: Color32,
        outline: Color32,
    ) {
        if self.drawing.is_locked(image.id) {
            return;
        }

        for p in image.handles() {
            let s = view.to_screen(p);
            painter.rect(
                Rect::from_center_size(
                    Pos2::new(s[0] as f32, s[1] as f32),
                    egui::Vec2::splat(crate::ui::style::SELECTION_HANDLE),
                ),
                1.,
                fill,
                Line::new(1., outline),
                egui::StrokeKind::Inside,
            );
        }
    }
}
fn fit_image(image: &mut ImageObject, page: Option<Point>) {
    let Some(page) = page else {
        return;
    };
    let b = image.world_bounds().unwrap();
    let factor = (page[0] / (b.max[0] - b.min[0]))
        .min(page[1] / (b.max[1] - b.min[1]))
        .min(1.0)
        * 0.98;
    if factor < 0.98 {
        image.placement.scale *= factor;
    }
    let b = image.world_bounds().unwrap();
    for (axis, size) in page.into_iter().enumerate() {
        image.placement.position[axis] += if b.min[axis] < 0. {
            -b.min[axis]
        } else if b.max[axis] > size {
            size - b.max[axis]
        } else {
            0.
        };
    }
}

#[cfg(test)]
#[path = "image_controller_tests.rs"]
mod tests;

//! Explicit local file picker/drop adapters. No workspace scans or retained source paths.
use crate::{
    drawing::Surface,
    image_object::{DecodedImage, ImageAsset, MAX_IMAGE_BYTES},
    object::Point,
};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, Sender},
};
use uuid::Uuid;
#[derive(Clone, Copy)]
pub struct Target {
    pub surface: Surface,
    pub position: Point,
    pub replace: Option<Uuid>,
}
pub struct Imported {
    pub target: Target,
    pub result: Result<(Arc<ImageAsset>, DecodedImage), String>,
}
pub struct Imports {
    pub sender: Sender<Imported>,
    pub receiver: Receiver<Imported>,
    pub pending: usize,
}
impl Default for Imports {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver,
            pending: 0,
        }
    }
}
fn deliver(
    sender: Sender<Imported>,
    ctx: egui::Context,
    target: Target,
    bytes: Result<Arc<[u8]>, String>,
) {
    let result = bytes.and_then(ImageAsset::import);
    let _ = sender.send(Imported { target, result });
    ctx.request_repaint();
}
impl Imports {
    pub fn picker(&mut self, ctx: egui::Context, target: Target) {
        if self.pending >= 4 {
            return;
        }
        self.pending += 1;
        let sender = self.sender.clone();
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let result = rfd::FileDialog::new()
                .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                .pick_file()
                .map_or_else(
                    || Err("Image selection cancelled.".into()),
                    |path| read_path(&path),
                );
            deliver(sender, ctx, target, result);
        });
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let bytes = if let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                .pick_file()
                .await
            {
                read_file(file.inner()).await
            } else {
                Err("Image selection cancelled.".into())
            };
            deliver(sender, ctx, target, bytes);
        });
    }
    pub fn drop_file(&mut self, ctx: egui::Context, target: Target, file: egui::DroppedFileHandle) {
        if self.pending >= 4 {
            return;
        }
        self.pending += 1;
        let sender = self.sender.clone();
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let bytes = read_path(file.path());
            deliver(sender, ctx, target, bytes);
        });
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let bytes = if let Some(file) = file.web_file() {
                read_file(file).await
            } else {
                Err("Browser did not supply image data. Use Image to choose a file.".into())
            };
            deliver(sender, ctx, target, bytes);
        });
    }
    pub fn paste_pixels(
        &mut self,
        ctx: egui::Context,
        target: Target,
        pixels: Arc<egui::ColorImage>,
    ) {
        if self.pending >= 4 {
            return;
        }
        self.pending += 1;
        let sender = self.sender.clone();
        let work = move || {
            let result = (|| {
                if pixels.size[0] == 0
                    || pixels.size[1] == 0
                    || pixels.size[0] as u64 * pixels.size[1] as u64
                        > crate::image_object::MAX_IMAGE_PIXELS
                    || pixels.size[0] > 16384
                    || pixels.size[1] > 16384
                {
                    return Err("Clipboard image exceeds import limits.".into());
                }
                let bytes: Vec<_> = pixels
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_srgba_unmultiplied())
                    .collect();
                let rgba =
                    image::RgbaImage::from_raw(pixels.size[0] as u32, pixels.size[1] as u32, bytes)
                        .ok_or("Invalid clipboard image.")?;
                let mut output = std::io::Cursor::new(Vec::new());
                image::DynamicImage::ImageRgba8(rgba)
                    .write_to(&mut output, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                Ok(output.into_inner().into())
            })();
            deliver(sender, ctx, target, result);
        };
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(work);
        #[cfg(target_arch = "wasm32")]
        work();
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn read_path(path: &std::path::Path) -> Result<Arc<[u8]>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_IMAGE_BYTES as u64 {
        return Err("Image exceeds 16 MiB.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes.into())
}
#[cfg(target_arch = "wasm32")]
async fn read_file(file: &web_sys::File) -> Result<Arc<[u8]>, String> {
    if file.size() > MAX_IMAGE_BYTES as f64 {
        return Err("Image exceeds 16 MiB.".into());
    }
    let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
        .await
        .map_err(|_| "Browser could not read image.")?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec().into())
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
pub struct BrowserPasteDestination {
    pub sender: Sender<Imported>,
    pub target: Target,
}
#[cfg(target_arch = "wasm32")]
pub fn paste_id() -> egui::Id {
    egui::Id::unique("image-paste-destination")
}

/// eframe handles text paste; this adds local image clipboard files and accurate drop coordinates.
#[cfg(target_arch = "wasm32")]
pub mod browser {
    use super::*;
    use wasm_bindgen::{JsCast, prelude::*};
    pub fn install(ctx: egui::Context) {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let Some(canvas) = document.get_element_by_id("xitoc-canvas") else {
            return;
        };
        for name in ["dragover", "drop"] {
            let ctx = ctx.clone();
            let canvas = canvas.clone();
            let listener =
                Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |event: web_sys::DragEvent| {
                    let rect = canvas.get_bounding_client_rect();
                    let pos = egui::pos2(
                        event.client_x() as f32 - rect.left() as f32,
                        event.client_y() as f32 - rect.top() as f32,
                    ) / ctx.zoom_factor();
                    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("image-drop-position"), pos));
                    ctx.request_repaint();
                });
            let _ = document
                .get_element_by_id("xitoc-canvas")
                .unwrap()
                .add_event_listener_with_callback_and_bool(
                    name,
                    listener.as_ref().unchecked_ref(),
                    true,
                );
            listener.forget();
        }
        let listener = Closure::<dyn FnMut(web_sys::ClipboardEvent)>::new(
            move |event: web_sys::ClipboardEvent| {
                if ctx.text_edit_focused() {
                    return;
                }
                let Some(destination) =
                    ctx.data(|d| d.get_temp::<BrowserPasteDestination>(paste_id()))
                else {
                    return;
                };
                let Some(files) = event.clipboard_data().and_then(|data| data.files()) else {
                    return;
                };
                let Some(file) = (0..files.length())
                    .filter_map(|i| files.get(i))
                    .find(|file| file.type_().starts_with("image/"))
                else {
                    return;
                };
                event.prevent_default();
                event.stop_immediate_propagation();
                let ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let bytes = read_file(&file).await;
                    deliver(destination.sender, ctx, destination.target, bytes);
                });
            },
        );
        let _ = document.add_event_listener_with_callback_and_bool(
            "paste",
            listener.as_ref().unchecked_ref(),
            true,
        );
        listener.forget();
    }
    pub fn inject(ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(pos) =
            ctx.data_mut(|d| d.remove_temp::<egui::Pos2>(egui::Id::unique("image-drop-position")))
        {
            raw.events.push(egui::Event::PointerMoved(pos));
        }
    }
}

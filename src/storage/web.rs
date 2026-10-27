//! Browser fallback only: explicit import and export, never arbitrary disk access.
use super::Loaded;
use crate::{document::Location, pdf};
use std::sync::mpsc::Sender;

pub enum Event {
    Imported(Result<Loaded, String>),
    Export(Result<(), String>),
}

pub fn import(sender: Sender<Event>, ctx: egui::Context) {
    wasm_bindgen_futures::spawn_local(async move {
        if let Some(file) = rfd::AsyncFileDialog::new()
            .add_filter("PDF", &["pdf"])
            .pick_file()
            .await
        {
            let result = if file.inner().size() > pdf::MAX_PDF_BYTES as f64 {
                Err("PDF is larger than the 16 MiB import limit".into())
            } else {
                match wasm_bindgen_futures::JsFuture::from(file.inner().array_buffer()).await {
                    Ok(buffer) => {
                        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                        pdf::decode(&bytes, Location::new(file.file_name())).map(|document| {
                            Loaded {
                                document,
                                bytes: bytes.into(),
                            }
                        })
                    }
                    Err(_) => Err("Browser could not read the selected file".into()),
                }
            };
            let _ = sender.send(Event::Imported(result));
            ctx.request_repaint();
        }
    });
}

pub fn export(
    name: String,
    bytes: std::sync::Arc<[u8]>,
    sender: Sender<Event>,
    ctx: egui::Context,
) {
    wasm_bindgen_futures::spawn_local(async move {
        if let Some(file) = rfd::AsyncFileDialog::new()
            .set_file_name(name)
            .save_file()
            .await
        {
            let result = file.write(&bytes).await.map_err(|e| e.to_string());
            let _ = sender.send(Event::Export(result));
            ctx.request_repaint();
        }
    });
}

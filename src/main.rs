mod app;
mod canvas;
mod document;
mod drawing;
mod editor;
mod pdf;
mod storage;
mod ui;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    eframe::run_native(
        "XiToc",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([1100.0, 760.0])
                .with_min_inner_size([480.0, 360.0]),
            ..Default::default()
        },
        Box::new(|_cc| Ok(Box::new(app::XiTocApp::default()))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast as _;

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document
            .get_element_by_id("xitoc-canvas")
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|_cc| Ok(Box::new(app::XiTocApp::default()))),
            )
            .await;
        if let Some(status) = document.get_element_by_id("loading") {
            match result {
                Ok(()) => status.remove(),
                Err(error) => status.set_text_content(Some(&format!(
                    "XiToc could not start: {error:?}. Check browser WebGL support."
                ))),
            }
        }
    });
}

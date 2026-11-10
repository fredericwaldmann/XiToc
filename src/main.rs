mod app;
mod canvas;
mod connector;
mod connector_route;
mod document;
mod drawing;
mod editor;
mod eraser_index;
mod object;
mod object_clipboard;
mod partial_erase;
mod pdf;
mod pen;
mod selection;
mod settings;
mod shape;
mod shape_border;
#[cfg(test)]
mod shape_style_tests;
mod smoothing;
mod storage;
mod stroke_pattern;
mod text;
mod text_fonts;
mod text_layout;
#[cfg(test)]
mod text_tests;
mod ui;
#[cfg(target_arch = "wasm32")]
mod web_pressure;

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
        Box::new(|cc| {
            text_layout::install(&cc.egui_ctx);
            Ok(Box::new(app::XiTocApp::new(cc.storage)))
        }),
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
                Box::new(|cc| {
                    text_layout::install(&cc.egui_ctx);
                    web_pressure::install(cc.egui_ctx.clone());
                    Ok(Box::new(app::XiTocApp::new(cc.storage)))
                }),
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

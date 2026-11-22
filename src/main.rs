mod app;
mod asset;
mod attachment;
mod attachment_io;
mod canvas;
mod code_listing;
mod connector;
mod connector_route;
mod coordinate;
mod coordinate_expression;
mod coordinate_geometry;
mod coordinate_tikz;
mod document;
mod drawing;
mod editor;
mod eraser_index;
mod formula;
mod formula_render;
#[cfg(test)]
mod formula_tests;
mod group;
mod image_import;
mod image_object;
mod latex_config;
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
mod snapping;
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
    const APP_NAME: &str = "XiToc";
    eframe::run_native(
        APP_NAME,
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_title(APP_NAME)
                .with_app_id(APP_NAME)
                .with_inner_size([1100.0, 760.0])
                .with_min_inner_size([480.0, 360.0])
                .with_decorations(true)
                .with_resizable(true)
                .with_icon(native_icon()),
            ..Default::default()
        },
        Box::new(|cc| {
            text_layout::install(&cc.egui_ctx);
            Ok(Box::new(app::XiTocApp::new(cc.storage)))
        }),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn native_icon() -> eframe::egui::IconData {
    const SIZE: u32 = 128;
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(include_str!("../assets/xitoc-icon.svg"), &options)
        .expect("the bundled XiToc application icon must be valid SVG");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(SIZE, SIZE)
        .expect("the fixed-size XiToc application icon must fit in memory");
    let scale = resvg::tiny_skia::Transform::from_scale(
        SIZE as f32 / tree.size().width(),
        SIZE as f32 / tree.size().height(),
    );
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let alpha = pixel.alpha();
            let unpremultiply = |channel: u8| {
                if alpha == 0 {
                    0
                } else {
                    ((u16::from(channel) * 255 + u16::from(alpha) / 2) / u16::from(alpha)).min(255)
                        as u8
                }
            };
            [
                unpremultiply(pixel.red()),
                unpremultiply(pixel.green()),
                unpremultiply(pixel.blue()),
                alpha,
            ]
        })
        .collect();
    eframe::egui::IconData {
        rgba,
        width: SIZE,
        height: SIZE,
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod native_window_tests {
    #[test]
    fn bundled_application_icon_is_valid_and_has_transparent_corners() {
        let icon = super::native_icon();
        assert_eq!((icon.width, icon.height), (128, 128));
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
        assert_eq!(&icon.rgba[..4], &[0, 0, 0, 0]);
        assert!(
            icon.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] == 255)
        );
    }
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
                    image_import::browser::install(cc.egui_ctx.clone());
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

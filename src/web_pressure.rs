//! Browser pen bridge and explicit contact cancellation. Mouse/touch motion
//! and release remain owned by eframe.
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export function installPenBridge(wake) {
    const canvas = document.getElementById('xitoc-canvas');
    const queue = [];
    window.xitocPenQueue = queue;
    let active = null;
    const handle = (event) => {
        if (event.pointerType !== 'pen' || !event.isPrimary) return;
        if (event.type === 'pointerdown') {
            if (event.button !== 0 || event.target !== canvas || active !== null) return;
            active = event.pointerId;
            canvas.setPointerCapture(active);
            canvas.focus();
        }
        if (event.pointerId !== active) return;
        const rect = canvas.getBoundingClientRect();
        const phase = {pointerdown: 0, pointermove: 1, pointerup: 2, pointercancel: 3}[event.type];
        queue.push([phase, event.clientX - rect.left, event.clientY - rect.top, event.pressure]);
        if (phase >= 2) active = null;
        event.preventDefault();
        event.stopImmediatePropagation();
        wake();
    };
    for (const name of ['pointerdown','pointermove','pointerup','pointercancel']) {
        document.addEventListener(name, handle, {capture:true, passive:false});
    }
    // eframe owns mouse/touch motion and release on document, but has no
    // mouse pointercancel listener. Forward explicit cancellation without
    // synthesizing movement/release or competing for pointer ownership.
    document.addEventListener('pointercancel', (event) => {
        if (event.pointerType !== 'pen' && event.isPrimary) {
            queue.push([3,0,0,0]);
            wake();
        }
    }, {capture:true});
    window.addEventListener('blur', () => {
        if (active !== null) { queue.push([3,0,0,0]); active = null; wake(); }
    });
}
export function takePenSamples() {
    return window.xitocPenQueue ? window.xitocPenQueue.splice(0) : [];
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = installPenBridge)]
    fn install_bridge(wake: &js_sys::Function);
    #[wasm_bindgen(js_name = takePenSamples)]
    fn take_samples() -> js_sys::Array;
}

pub fn install(ctx: egui::Context) {
    use wasm_bindgen::JsCast as _;
    // The page-lifetime event listeners own this wake callback.
    let wake = Closure::<dyn Fn()>::new(move || ctx.request_repaint());
    install_bridge(wake.as_ref().unchecked_ref());
    wake.forget();
}

pub fn inject(ctx: &egui::Context, raw: &mut egui::RawInput) {
    let samples = take_samples();
    if samples.length() == 0 {
        return;
    }
    for sample in samples.iter() {
        let sample = js_sys::Array::from(&sample);
        let number = |i| sample.get(i).as_f64().unwrap_or(0.0);
        let phase = number(0) as u8;
        let pos = egui::pos2(number(1) as f32, number(2) as f32) / ctx.zoom_factor();
        raw.events.extend(super::canvas::pen_events(
            phase,
            pos,
            number(3) as f32,
            ctx.input(|i| i.modifiers),
        ));
    }
}

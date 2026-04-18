#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions::default();
    if let Err(err) = eframe::run_native(
        "digital garden",
        native_options,
        Box::new(|cc| Ok(Box::new(digital_garden::TemplateApp::new(cc)))),
    ) {
        eprintln!("digital garden exited with error: {err}");
    }
}

// when compiling to web using trunk. eframe 0.31 replaced the sync
// `start_web` API with `WebRunner` + an async `start` that attaches to
// an `HtmlCanvasElement`. Spawn it and log any startup failure to the
// browser console.
#[cfg(target_arch = "wasm32")]
fn main() {
    // Make sure panics are logged using `console.error`.
    console_error_panic_hook::set_once();

    // Redirect tracing to console.log and friends:
    tracing_wasm::set_as_global_default();

    use wasm_bindgen::JsCast;

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("no document");
        let canvas = document
            .get_element_by_id("the_canvas_id")
            .and_then(|el| el.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("canvas #the_canvas_id not found");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(digital_garden::TemplateApp::new(cc)))),
            )
            .await;
        if let Err(err) = start_result {
            web_sys::console::error_1(
                &format!("Failed to start digital garden: {err:?}").into(),
            );
        }
    });
}

use wasm_bindgen::prelude::*;

/// Phase 1 pipeline check: returns a dummy value so the web app can verify
/// the WASM module loaded and is callable.
#[wasm_bindgen]
pub fn pipeline_check() -> u32 {
    42
}

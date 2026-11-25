//! Calling the backend: `window.__TAURI__.core.invoke`, with the shared `ipc` types.

use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

fn to_js<A: Serialize>(args: &A) -> Result<JsValue, String> {
    // Plain JS objects (not Maps), which is what Tauri expects.
    args.serialize(&serde_wasm_bindgen::Serializer::json_compatible()).map_err(|e| e.to_string())
}

/// Call a backend command. Errors come back as the backend's message.
pub async fn invoke<T: DeserializeOwned, A: Serialize>(cmd: &str, args: &A) -> Result<T, String> {
    let args = to_js(args)?;
    match tauri_invoke(cmd, args).await {
        Ok(v) => serde_wasm_bindgen::from_value(v).map_err(|e| format!("unexpected reply from {cmd}: {e}")),
        Err(e) => Err(e.as_string().unwrap_or_else(|| format!("{e:?}"))),
    }
}

#[derive(Serialize)]
pub struct NoArgs {}

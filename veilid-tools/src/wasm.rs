use super::*;
use core::sync::atomic::{AtomicI8, Ordering};
use js_sys::{global, Reflect};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn console_log(s: &str);

    #[wasm_bindgen]
    pub fn alert(s: &str);
}

pub fn is_browser() -> bool {
    static CACHE: AtomicI8 = AtomicI8::new(-1);
    let cache = CACHE.load(Ordering::Relaxed);
    if cache != -1 {
        return cache != 0;
    }

    let res = Reflect::has(&global().as_ref(), &"navigator".into()).unwrap_or_default();

    CACHE.store(res as i8, Ordering::Relaxed);

    res
}

pub fn is_browser_https() -> bool {
    static CACHE: AtomicI8 = AtomicI8::new(-1);
    let cache = CACHE.load(Ordering::Relaxed);
    if cache != -1 {
        return cache != 0;
    }

    let res = js_sys::eval("self.location.protocol === 'https'")
        .map(|res| res.is_truthy())
        .unwrap_or_default();

    CACHE.store(res as i8, Ordering::Relaxed);

    res
}

pub fn get_wasm_global_string_value<K: AsRef<str>>(key: K) -> Option<String> {
    let Ok(v) = Reflect::get(&global().as_ref(), &JsValue::from_str(key.as_ref())) else {
        return None;
    };
    v.as_string()
}

#[derive(ThisError, Debug, Clone, Eq, PartialEq)]
#[error("JsValue error")]
pub struct JsValueError(String);

pub fn map_jsvalue_error(x: JsValue) -> JsValueError {
    JsValueError(x.as_string().unwrap_or_default())
}
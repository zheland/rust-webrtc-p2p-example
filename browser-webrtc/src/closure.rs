use wasm_bindgen::closure::Closure;
use wasm_bindgen::convert::{FromWasmAbi, ReturnWasmAbi};

pub fn closure_0<F, R>(func: F) -> Closure<dyn FnMut() -> R>
where
    F: 'static + FnMut() -> R,
    R: 'static + ReturnWasmAbi,
{
    let handler: Box<dyn FnMut() -> R> = Box::new(func);
    Closure::wrap(handler)
}

pub fn closure_1<F, R, T1>(func: F) -> Closure<dyn FnMut(T1) -> R>
where
    F: 'static + FnMut(T1) -> R,
    R: 'static + ReturnWasmAbi,
    T1: 'static + FromWasmAbi,
{
    let handler: Box<dyn FnMut(T1) -> R> = Box::new(func);
    Closure::wrap(handler)
}

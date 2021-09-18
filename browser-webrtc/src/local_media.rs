use web_sys::{MediaStream, MediaStreamConstraints};

#[derive(Clone, Debug)]
pub struct LocalMedia {
    js_media_stream: MediaStream,
}

impl LocalMedia {
    pub async fn new(constraints: MediaStreamConstraints) -> Self {
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::JsFuture;
        use web_sys::window;

        let window = window().unwrap();
        let navigator = window.navigator();
        let media_devices = navigator.media_devices().unwrap();
        let media_stream_promise = media_devices
            .get_user_media_with_constraints(&constraints)
            .unwrap();
        let js_media_stream: MediaStream = JsFuture::from(media_stream_promise)
            .await
            .unwrap()
            .dyn_into()
            .unwrap();

        Self { js_media_stream }
    }

    pub async fn with_video() -> Self {
        use wasm_bindgen::JsValue;

        let mut constraints = MediaStreamConstraints::new();
        let _: &mut _ = constraints.video(&JsValue::TRUE);
        Self::new(constraints).await
    }

    pub async fn with_audio() -> Self {
        use wasm_bindgen::JsValue;

        let mut constraints = MediaStreamConstraints::new();
        let _: &mut _ = constraints.audio(&JsValue::TRUE);
        Self::new(constraints).await
    }

    pub async fn with_video_and_audio() -> Self {
        use wasm_bindgen::JsValue;

        let mut constraints = MediaStreamConstraints::new();
        let _: &mut _ = constraints.video(&JsValue::TRUE);
        let _: &mut _ = constraints.audio(&JsValue::TRUE);
        Self::new(constraints).await
    }

    pub fn media_stream(&self) -> &MediaStream {
        &self.js_media_stream
    }
}

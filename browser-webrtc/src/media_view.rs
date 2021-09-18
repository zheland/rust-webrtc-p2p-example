use async_std::sync::Arc;
use thiserror::Error;
use wasm_bindgen::JsValue;
use web_sys::{HtmlVideoElement, MediaStream};

#[derive(Debug)]
pub struct MediaView {
    pub video: HtmlVideoElement,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MediaViewAudio {
    Disable,
    Enable,
}

impl MediaView {
    pub fn new(
        media_stream: MediaStream,
        audio: MediaViewAudio,
    ) -> Result<Arc<Self>, NewMediaViewError> {
        use wasm_bindgen::JsCast;
        use web_sys::window;

        let window = window().ok_or(NewMediaViewError::WindowIsUndefined)?;
        let document = window
            .document()
            .ok_or(NewMediaViewError::DocumentIsUndefined)?;
        let video: HtmlVideoElement = document
            .create_element("video")
            .map_err(|err| NewMediaViewError::VideoElementCreateError(err))?
            .dyn_into()
            .unwrap();

        video.set_autoplay(true);
        let _: Option<_> = video.set_attribute("playsinline", "").ok();

        video.set_src_object(Some(&media_stream));

        match audio {
            MediaViewAudio::Enable => video.set_muted(false),
            MediaViewAudio::Disable => video.set_muted(true),
        }

        Ok(Arc::new(Self { video }))
    }

    pub fn view(&self) -> &HtmlVideoElement {
        &self.video
    }
}

#[derive(Error, Debug)]
pub enum NewMediaViewError {
    #[error("JavaScript window is undefined")]
    WindowIsUndefined,
    #[error("JavaScript document is undefined")]
    DocumentIsUndefined,
    #[error("failed to create video element: {0:?}")]
    VideoElementCreateError(JsValue),
}

use core::cell::RefCell;

use async_std::sync::Arc;
use thiserror::Error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use web_sys::{MediaStream, MediaStreamTrack, TrackEvent};

use crate::{BoxAsyncFn2, BoxAsyncFn2Wrapper, Receiver};

#[derive(Debug)]
pub struct MediaReceiverBuilder {
    receiver: Arc<Receiver>,
    js_media_stream: MediaStream,
}

impl MediaReceiverBuilder {
    pub fn new(receiver: Arc<Receiver>, js_media_stream: MediaStream) -> Self {
        Self {
            receiver,
            js_media_stream,
        }
    }

    pub fn build_with_handler(
        self,
        handler: BoxAsyncFn2<Arc<MediaReceiver>, MediaReceiverEvent, ()>,
    ) -> Arc<MediaReceiver> {
        MediaReceiver::new(self.receiver, self.js_media_stream, handler)
    }
}

#[derive(Debug)]
pub struct MediaReceiver {
    receiver: Arc<Receiver>,
    handler: BoxAsyncFn2Wrapper<Arc<MediaReceiver>, MediaReceiverEvent, ()>,
    js_media_stream: MediaStream,
    js_add_track_handler: RefCell<Option<Closure<dyn FnMut(TrackEvent)>>>,
    js_remove_track_handler: RefCell<Option<Closure<dyn FnMut(TrackEvent)>>>,
}

impl MediaReceiver {
    pub fn new(
        receiver: Arc<Receiver>,
        js_media_stream: MediaStream,
        handler: BoxAsyncFn2<Arc<Self>, MediaReceiverEvent, ()>,
    ) -> Arc<Self> {
        log::trace!("browser_webrtc::MediaReceiver::new");

        let data_channel = Arc::new(Self {
            receiver,
            handler: BoxAsyncFn2Wrapper(handler),
            js_media_stream,
            js_add_track_handler: RefCell::new(None),
            js_remove_track_handler: RefCell::new(None),
        });

        data_channel.init_add_track_handler();
        data_channel.init_remove_track_handler();

        data_channel
    }

    pub fn media_stream(&self) -> &MediaStream {
        &self.js_media_stream
    }

    fn init_add_track_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_add_track_handler = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |ev: TrackEvent| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_add_track_event(ev).await })
            })
        };
        self.js_media_stream
            .set_onaddtrack(Some(js_add_track_handler.as_ref().unchecked_ref()));
        let prev_handler = self
            .js_add_track_handler
            .replace(Some(js_add_track_handler));
        debug_assert!(prev_handler.is_none());
    }

    fn init_remove_track_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_remove_track_handler = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |ev: TrackEvent| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_remove_track_event(ev).await })
            })
        };
        self.js_media_stream
            .set_onremovetrack(Some(js_remove_track_handler.as_ref().unchecked_ref()));
        let prev_handler = self
            .js_remove_track_handler
            .replace(Some(js_remove_track_handler));
        debug_assert!(prev_handler.is_none());
    }

    async fn handler(self: &Arc<Self>, ev: MediaReceiverEvent) {
        self.handler.0(Arc::clone(self), ev).await
    }

    async fn error(self: &Arc<Self>, err: MediaReceiverError) {
        self.handler(MediaReceiverEvent::Error(err)).await
    }

    async fn on_add_track_event(self: &Arc<Self>, ev: TrackEvent) {
        match self.handle_add_track_event(ev).await {
            Ok(()) => {}
            Err(err) => self.error(err).await,
        }
    }

    async fn handle_add_track_event(
        self: &Arc<Self>,
        ev: TrackEvent,
    ) -> Result<(), MediaReceiverError> {
        use wasm_bindgen::JsCast;

        let track = ev
            .track()
            .and_then(|track| track.dyn_into().ok())
            .ok_or_else(|| MediaReceiverError::InvalidAddTrackValue(ev.track().map(Into::into)))?;
        self.handler(MediaReceiverEvent::AddTrack(track)).await;
        Ok(())
    }

    async fn on_remove_track_event(self: &Arc<Self>, ev: TrackEvent) {
        match self.handle_remove_track_event(ev).await {
            Ok(()) => {}
            Err(err) => self.error(err).await,
        }
    }

    async fn handle_remove_track_event(
        self: &Arc<Self>,
        ev: TrackEvent,
    ) -> Result<(), MediaReceiverError> {
        use wasm_bindgen::JsCast;

        let track = ev
            .track()
            .and_then(|track| track.dyn_into().ok())
            .ok_or_else(|| {
                MediaReceiverError::InvalidRemoveTrackValue(ev.track().map(Into::into))
            })?;
        self.handler(MediaReceiverEvent::RemoveTrack(track)).await;
        Ok(())
    }
}

impl Drop for MediaReceiver {
    fn drop(&mut self) {
        log::trace!("browser_webrtc::MediaReceiver::drop");
    }
}

#[derive(Debug)]
pub enum MediaReceiverEvent {
    AddTrack(MediaStreamTrack),
    RemoveTrack(MediaStreamTrack),
    Error(MediaReceiverError),
}

#[derive(Error, Debug)]
pub enum MediaReceiverError {
    #[error("add track event called without MediaStreamTrack: {0:?}")]
    InvalidAddTrackValue(Option<JsValue>),
    #[error("add track event called without MediaStreamTrack: {0:?}")]
    InvalidRemoveTrackValue(Option<JsValue>),
}

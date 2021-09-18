use core::cell::RefCell;

use async_std::sync::Arc;
use thiserror::Error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use web_sys::{Event, RtcDataChannel, RtcPeerConnection};

use crate::{BoxAsyncFn2, BoxAsyncFn2Wrapper, Sender};

#[derive(Debug)]
pub struct DataSender {
    sender: Arc<Sender>,
    handler: BoxAsyncFn2Wrapper<Arc<DataSender>, DataSenderEvent, ()>,
    js_channel: RtcDataChannel,
    js_open_handler: RefCell<Option<Closure<dyn FnMut(Event)>>>,
    js_error_handler: RefCell<Option<Closure<dyn FnMut(Event)>>>,
}

impl DataSender {
    pub fn new<T: AsRef<str>>(
        sender: Arc<Sender>,
        js_connection: RtcPeerConnection,
        name: T,
        handler: BoxAsyncFn2<Arc<Self>, DataSenderEvent, ()>,
    ) -> Arc<Self> {
        log::trace!("browser_webrtc::DataSender::new");

        use web_sys::RtcDataChannelType;

        let js_channel = js_connection.create_data_channel(name.as_ref());
        js_channel.set_binary_type(RtcDataChannelType::Arraybuffer);

        let data_channel = Arc::new(Self {
            sender,
            handler: BoxAsyncFn2Wrapper(handler),
            js_channel: js_channel,
            js_open_handler: RefCell::new(None),
            js_error_handler: RefCell::new(None),
        });

        data_channel.init_open_handler();
        data_channel.init_error_handler();

        data_channel
    }

    fn init_open_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_open_handler = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |_: Event| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_open_event().await })
            })
        };
        self.js_channel
            .set_onopen(Some(js_open_handler.as_ref().unchecked_ref()));
        let prev_handler = self.js_open_handler.replace(Some(js_open_handler));
        debug_assert!(prev_handler.is_none());
    }

    fn init_error_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_error_handler = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |ev: Event| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_error_event(ev).await })
            })
        };
        self.js_channel
            .set_onopen(Some(js_error_handler.as_ref().unchecked_ref()));
        let prev_handler = self.js_error_handler.replace(Some(js_error_handler));
        debug_assert!(prev_handler.is_none());
    }

    async fn handler(self: &Arc<Self>, ev: DataSenderEvent) {
        self.handler.0(Arc::clone(self), ev).await
    }

    async fn error(self: &Arc<Self>, err: DataSenderError) {
        self.handler(DataSenderEvent::Error(err)).await
    }

    async fn on_open_event(self: &Arc<Self>) {
        self.handler(DataSenderEvent::Open).await;
    }

    async fn on_error_event(self: &Arc<Self>, ev: Event) {
        use js_sys::Reflect;
        let error = Reflect::get(&ev, &JsValue::from_str("error")).unwrap();
        self.error(DataSenderError::RtcDataChannelError(error))
            .await;
    }

    pub fn send(&self, data: &[u8]) -> Result<(), DataSenderSendError> {
        self.js_channel
            .send_with_u8_array(data)
            .map_err(DataSenderSendError::RtcDataChannelSendError)
    }
}

impl Drop for DataSender {
    fn drop(&mut self) {
        log::trace!("browser_webrtc::DataSender::drop");

        self.js_channel.set_onopen(None);
        self.js_channel.set_onerror(None);
        self.js_channel.close();
    }
}

#[derive(Debug)]
pub enum DataSenderEvent {
    Open,
    Error(DataSenderError),
}

#[derive(Error, Debug)]
pub enum DataSenderError {
    #[error("RtcDataChannel error: {0:?}")]
    RtcDataChannelError(JsValue),
}

#[derive(Error, Debug)]
pub enum DataSenderSendError {
    #[error("RtcDataChannel send error: {0:?}")]
    RtcDataChannelSendError(JsValue),
}

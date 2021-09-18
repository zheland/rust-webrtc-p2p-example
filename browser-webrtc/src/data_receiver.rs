use core::cell::RefCell;

use async_std::sync::Arc;
use thiserror::Error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use web_sys::{MessageEvent, RtcDataChannel};

use crate::{BoxAsyncFn2, BoxAsyncFn2Wrapper, Receiver};

#[derive(Debug)]
pub struct DataReceiverBuilder {
    receiver: Arc<Receiver>,
    js_channel: RtcDataChannel,
}

impl DataReceiverBuilder {
    pub fn new(receiver: Arc<Receiver>, js_channel: RtcDataChannel) -> Self {
        Self {
            receiver,
            js_channel,
        }
    }

    pub fn build_with_handler(
        self,
        handler: BoxAsyncFn2<Arc<DataReceiver>, DataReceiverEvent, ()>,
    ) -> Arc<DataReceiver> {
        DataReceiver::new(self.receiver, self.js_channel, handler)
    }
}

#[derive(Debug)]
pub struct DataReceiver {
    receiver: Arc<Receiver>,
    handler: BoxAsyncFn2Wrapper<Arc<DataReceiver>, DataReceiverEvent, ()>,
    js_channel: RtcDataChannel,
    js_message_handler: RefCell<Option<Closure<dyn FnMut(MessageEvent)>>>,
}

impl DataReceiver {
    pub fn new(
        receiver: Arc<Receiver>,
        js_channel: RtcDataChannel,
        handler: BoxAsyncFn2<Arc<Self>, DataReceiverEvent, ()>,
    ) -> Arc<Self> {
        log::trace!("browser_webrtc::DataReceiver::new");

        let data_channel = Arc::new(Self {
            receiver,
            handler: BoxAsyncFn2Wrapper(handler),
            js_channel: js_channel,
            js_message_handler: RefCell::new(None),
        });

        data_channel.init_message_handler();

        data_channel
    }

    fn init_message_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_message_handler = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |ev: MessageEvent| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_message_event(ev).await })
            })
        };
        self.js_channel
            .set_onmessage(Some(js_message_handler.as_ref().unchecked_ref()));
        let prev_handler = self.js_message_handler.replace(Some(js_message_handler));
        debug_assert!(prev_handler.is_none());
    }

    async fn handler(self: &Arc<Self>, ev: DataReceiverEvent) {
        self.handler.0(Arc::clone(self), ev).await
    }

    async fn error(self: &Arc<Self>, err: DataReceiverError) {
        self.handler(DataReceiverEvent::Error(err)).await
    }

    async fn on_message_event(self: &Arc<Self>, ev: MessageEvent) {
        match self.clone().handle_message_event(ev).await {
            Ok(()) => {}
            Err(err) => self.error(err).await,
        }
    }

    async fn handle_message_event(
        self: &Arc<Self>,
        ev: MessageEvent,
    ) -> Result<(), DataReceiverError> {
        use js_sys::{ArrayBuffer, Uint8Array};
        use wasm_bindgen::JsCast;

        let array_buffer: ArrayBuffer = ev
            .data()
            .dyn_into()
            .map_err(DataReceiverError::NonArrayData)?;
        let data = Uint8Array::new(&array_buffer).to_vec();

        self.handler(DataReceiverEvent::Message(data)).await;
        Ok(())
    }
}

impl Drop for DataReceiver {
    fn drop(&mut self) {
        log::trace!("browser_webrtc::DataReceiver::drop");

        self.js_channel.set_onmessage(None);
        self.js_channel.close();
    }
}

#[derive(Debug)]
pub enum DataReceiverEvent {
    Message(Vec<u8>),
    Error(DataReceiverError),
}

#[derive(Error, Debug)]
pub enum DataReceiverError {
    #[error("non-array data received: {0:?}")]
    NonArrayData(JsValue),
}

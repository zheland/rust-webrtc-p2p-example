use core::cell::RefCell;
use core::sync::atomic::AtomicU32;
use std::collections::HashMap;

use async_std::sync::{Arc, RwLock, Weak};
use signaling_protocol::{
    ChannelId, NetworkMode, ServerMessage, SessionReceiverId, SessionSenderId,
};
use thiserror::Error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use web_sys::{MessageEvent, RtcConfiguration, WebSocket};

use crate::{
    parse_websocket_server_message, BoxAsyncFn2, BoxAsyncFn2Wrapper, NewReceiverError,
    NewSenderError, Receiver, ReceiverEvent, Sender, SenderEvent, WebSocketServerMessageParseError,
};

#[derive(Debug)]
pub struct Server {
    senders: RwLock<HashMap<SessionSenderId, Weak<Sender>>>,
    receivers: RwLock<HashMap<SessionReceiverId, Weak<Receiver>>>,
    handler: BoxAsyncFn2Wrapper<Arc<Server>, ServerEvent, ()>,
    next_sender_id: AtomicU32,
    next_receiver_id: AtomicU32,
    js_websocket: WebSocket,
    js_message_handler: RefCell<Option<Closure<dyn FnMut(MessageEvent)>>>,
    //js_close_handler: RefCell<Option<Closure<dyn FnMut(CloseEvent)>>>,
}

impl Server {
    pub async fn new<Url>(
        url: Url,
        handler: BoxAsyncFn2<Arc<Self>, ServerEvent, ()>,
    ) -> Result<Arc<Self>, NewServerError>
    where
        Url: AsRef<str>,
    {
        log::trace!("browser_webrtc::Server::new");

        use js_sys::Promise;
        use wasm_bindgen_futures::JsFuture;
        use web_sys::BinaryType;

        let js_websocket =
            WebSocket::new(url.as_ref()).map_err(NewServerError::NewWebSocketError)?;
        js_websocket.set_binary_type(BinaryType::Arraybuffer);

        let server = Arc::new(Self {
            senders: RwLock::new(HashMap::new()),
            receivers: RwLock::new(HashMap::new()),
            handler: BoxAsyncFn2Wrapper(handler),
            next_sender_id: AtomicU32::new(0),
            next_receiver_id: AtomicU32::new(0),
            js_websocket: js_websocket.clone(),
            js_message_handler: RefCell::new(None),
            //js_close_handler: RefCell::new(None),
        });

        server.init_message_handler();

        /*let js_close_handler = {
            let server = Arc::clone(&server);
            closure_1(move |ev: CloseEvent| {
                let server = Arc::clone(&server);
                spawn_local(async move { server.on_close_event(ev).await })
            })
        };
        js_websocket.set_onmessage(Some(js_close_handler.as_ref().unchecked_ref()));
        let prev_handler = server.js_close_handler.replace(Some(js_close_handler));
        debug_assert!(prev_handler.is_none());*/

        let web_socket_opened = Promise::new(&mut |resolve, reject| {
            js_websocket.set_onopen(Some(&resolve));
            js_websocket.set_onerror(Some(&reject));
        });
        let _: JsValue = JsFuture::from(web_socket_opened)
            .await
            .map_err(NewServerError::WebSocketError)?;

        Ok(server)
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
        self.js_websocket
            .set_onmessage(Some(js_message_handler.as_ref().unchecked_ref()));
        let prev_handler = self.js_message_handler.replace(Some(js_message_handler));
        debug_assert!(prev_handler.is_none());
    }

    pub async fn open_channel(
        self: &Arc<Self>,
        channel_id: ChannelId,
        network_mode: NetworkMode,
        rtc_configuration: Option<RtcConfiguration>,
        handler: BoxAsyncFn2<Arc<Sender>, SenderEvent, ()>,
    ) -> Result<Arc<Sender>, ServerOpenChannelError> {
        use core::sync::atomic::Ordering;

        let sender_id = SessionSenderId(self.next_sender_id.fetch_add(1, Ordering::Relaxed));
        let sender = Sender::new(
            self.js_websocket.clone(),
            Arc::clone(self),
            sender_id,
            channel_id,
            network_mode,
            handler,
            rtc_configuration,
        )?;

        let mut senders = self.senders.write().await;
        let prev_sender = senders.insert(sender_id, Arc::downgrade(&sender));
        debug_assert!(prev_sender.is_none());

        Ok(sender)
    }

    pub async fn join_channel(
        self: &Arc<Self>,
        channel_id: ChannelId,
        rtc_configuration: Option<RtcConfiguration>,
        handler: BoxAsyncFn2<Arc<Receiver>, ReceiverEvent, ()>,
    ) -> Result<Arc<Receiver>, ServerJoinChannelError> {
        use core::sync::atomic::Ordering;

        let receiver_id = SessionReceiverId(self.next_receiver_id.fetch_add(1, Ordering::Relaxed));
        let receiver = Receiver::new(
            self.js_websocket.clone(),
            Arc::clone(self),
            receiver_id,
            channel_id,
            handler,
            rtc_configuration,
        )?;

        let mut receivers = self.receivers.write().await;
        let prev_receiver = receivers.insert(receiver_id, Arc::downgrade(&receiver));
        debug_assert!(prev_receiver.is_none());

        Ok(receiver)
    }

    pub(crate) async fn on_sender_dropped(self: &Arc<Self>, sender_id: SessionSenderId) {
        let mut senders = self.senders.write().await;
        let sender = senders.remove(&sender_id);
        if sender.is_none() {
            self.error(ServerError::SenderWasAlreadyRemoved(sender_id))
                .await
        }
    }

    pub(crate) async fn on_receiver_dropped(self: &Arc<Self>, receiver_id: SessionReceiverId) {
        let mut receivers = self.receivers.write().await;
        let receiver = receivers.remove(&receiver_id);
        if receiver.is_none() {
            self.error(ServerError::ReceiverWasAlreadyRemoved(receiver_id))
                .await
        }
    }

    async fn handler(self: &Arc<Self>, ev: ServerEvent) {
        self.handler.0(Arc::clone(self), ev).await
    }

    async fn error(self: &Arc<Self>, err: ServerError) {
        self.handler(ServerEvent::Error(err)).await
    }

    async fn on_message_event(self: &Arc<Self>, ev: MessageEvent) {
        match self.handle_socket_message(ev).await {
            Ok(()) => {}
            Err(err) => self.error(err).await,
        }
    }

    async fn handle_socket_message(self: &Arc<Self>, ev: MessageEvent) -> Result<(), ServerError> {
        match parse_websocket_server_message(ev) {
            Ok(msg) => match msg {
                ServerMessage::OpenChannelIdsChanged(ids) => {
                    self.handler(ServerEvent::OpenChannelIdsChanged(ids)).await;
                    Ok(())
                }
                ServerMessage::SenderMessage { sender_id, message } => {
                    let senders = self.senders.read().await;
                    match senders.get(&sender_id) {
                        Some(sender) => match sender.upgrade() {
                            Some(sender) => {
                                drop(senders);
                                sender.on_server_message(message).await;
                                Ok(())
                            }
                            None => Err(ServerError::SenderWasDropped(sender_id)),
                        },
                        None => Err(ServerError::SenderDoesNotExist(sender_id)),
                    }
                }
                ServerMessage::ReceiverMessage {
                    receiver_id,
                    message,
                } => {
                    let receivers = self.receivers.read().await;
                    match receivers.get(&receiver_id) {
                        Some(receiver) => match receiver.upgrade() {
                            Some(receiver) => {
                                drop(receivers);
                                receiver.on_server_message(message).await;
                                Ok(())
                            }
                            None => Err(ServerError::ReceiverWasDropped(receiver_id)),
                        },
                        None => Err(ServerError::ReceiverDoesNotExist(receiver_id)),
                    }
                }
            },
            Err(err) => Err(ServerError::ParseError(err.into())),
        }
    }

    /*async fn on_close_event(self: &Arc<Self>, ev: CloseEvent) {
        match self.handle_close_event(ev).await {
            Ok(()) => {}
            Err(err) => self.error(err).await,
        }
    }

    async fn handle_close_event(self: &Arc<Self>, ev: CloseEvent) -> Result<(), ServerError> {
        /*match ev.code() {
            1000 => {
                self.handler(ServerEvent::WebSocketClosed);
                Ok(())
            }
            code => Err(NewServerError {
                WebSocketCloseError,
            }),
        }*/
        Ok(())
    }*/
}

impl Drop for Server {
    fn drop(&mut self) {
        log::trace!("browser_webrtc::Server::drop");

        self.js_websocket.set_onmessage(None);
        let _: Option<_> = self.js_websocket.close().ok();
    }
}

#[derive(Error, Debug)]
pub enum NewServerError {
    #[error("new WebSocket error: {0:?}")]
    NewWebSocketError(JsValue),
    #[error("WebSocket error: {0:?}")]
    WebSocketError(JsValue),
    #[error("WebSocket close error: {0:?}")]
    WebSocketCloseError(JsValue),
    /*#[error("WebSocket close error with code {code}, reason: {reason}, was_clean: {was_clean}")]
    WebSocketCloseError {
        code: u16,
        reason: String,
        was_clean: bool,
    },*/
}

#[derive(Error, Debug)]
pub enum ServerOpenChannelError {
    #[error(transparent)]
    NewSenderError(#[from] NewSenderError),
}

#[derive(Error, Debug)]
pub enum ServerJoinChannelError {
    #[error(transparent)]
    NewReceiverError(#[from] NewReceiverError),
}

#[derive(Debug)]
pub enum ServerEvent {
    OpenChannelIdsChanged(Vec<ChannelId>),
    WebSocketClosed,
    Error(ServerError),
}

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("server message parse error: {0}")]
    ParseError(#[from] WebSocketServerMessageParseError),
    #[error("sender `{}` does not exist", 0.0)]
    SenderDoesNotExist(SessionSenderId),
    #[error("sender `{}` was dropped", 0.0)]
    SenderWasDropped(SessionSenderId),
    #[error("sender `{}` was already removed", 0.0)]
    SenderWasAlreadyRemoved(SessionSenderId),
    #[error("receiver `{}` does not exist", 0.0)]
    ReceiverDoesNotExist(SessionReceiverId),
    #[error("receiver `{}` was dropped", 0.0)]
    ReceiverWasDropped(SessionReceiverId),
    #[error("receiver `{}` was already removed", 0.0)]
    ReceiverWasAlreadyRemoved(SessionReceiverId),
}

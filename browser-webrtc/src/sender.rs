use core::cell::RefCell;
use core::sync::atomic::AtomicBool;

use async_std::sync::Arc;
use signaling_protocol::{
    ChannelId, ClientMessage, ClientSenderMessage, NetworkMode, ServerSenderErrorMessage,
    ServerSenderMessage, SessionDescription, SessionSenderId,
};
use thiserror::Error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use web_sys::{
    Event, MediaStream, RtcConfiguration, RtcIceCandidate, RtcIceCandidateInit,
    RtcIceConnectionState, RtcIceGatheringState, RtcPeerConnection, RtcPeerConnectionIceEvent,
    RtcSignalingState, WebSocket,
};

use crate::{
    send_websocket_client_message, BoxAsyncFn2, BoxAsyncFn2Wrapper, DataSender, DataSenderEvent,
    MediaSender, Server, WebSocketClientMessageSendError,
};

#[derive(Debug)]
pub struct Sender {
    server: Arc<Server>,
    sender_id: SessionSenderId,
    handler: BoxAsyncFn2Wrapper<Arc<Sender>, SenderEvent, ()>,
    js_connection: RtcPeerConnection,
    js_websocket: WebSocket,
    js_ice_candidate_handler: RefCell<Option<Closure<dyn FnMut(RtcPeerConnectionIceEvent)>>>,
    js_negotiation_needed_handler: RefCell<Option<Closure<dyn FnMut(Event)>>>,
    js_ice_connection_state_change_handler: RefCell<Option<Closure<dyn FnMut(Event)>>>,
    js_ice_gathering_state_change: RefCell<Option<Closure<dyn FnMut(Event)>>>,
    js_signaling_state_change_change: RefCell<Option<Closure<dyn FnMut(Event)>>>,
    is_started: AtomicBool,
}

impl Sender {
    pub fn new(
        js_websocket: WebSocket,
        server: Arc<Server>,
        sender_id: SessionSenderId,
        channel_id: ChannelId,
        network_mode: NetworkMode,
        handler: BoxAsyncFn2<Arc<Self>, SenderEvent, ()>,
        rtc_configuration: Option<RtcConfiguration>,
    ) -> Result<Arc<Self>, NewSenderError> {
        log::trace!("browser_webrtc::Sender::new");

        let message = ClientMessage::SenderMessage {
            sender_id,
            message: ClientSenderMessage::OpenChannel {
                channel_id,
                network_mode,
            },
        };
        send_websocket_client_message(&js_websocket, message)?;

        let js_connection = match rtc_configuration {
            Some(config) => RtcPeerConnection::new_with_configuration(&config),
            None => RtcPeerConnection::new(),
        }
        .map_err(NewSenderError::NewRtcPeerConnectionError)?;

        let sender = Arc::new(Self {
            server,
            sender_id,
            handler: BoxAsyncFn2Wrapper(handler),
            js_connection: js_connection.clone(),
            js_websocket,
            js_ice_candidate_handler: RefCell::new(None),
            js_negotiation_needed_handler: RefCell::new(None),
            js_ice_connection_state_change_handler: RefCell::new(None),
            js_ice_gathering_state_change: RefCell::new(None),
            js_signaling_state_change_change: RefCell::new(None),
            is_started: AtomicBool::new(false),
        });

        sender.init_icecandidate_handler();
        sender.init_ice_connection_state_change_handler();
        sender.init_ice_gathering_state_change_handler();
        sender.init_signaling_state_change_handler();

        Ok(sender)
    }

    fn init_icecandidate_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_ice_candidate_handler = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |ev: RtcPeerConnectionIceEvent| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_ice_candidate_event(ev).await });
            })
        };
        self.js_connection
            .set_onicecandidate(Some(js_ice_candidate_handler.as_ref().unchecked_ref()));
        let prev_handler = self
            .js_ice_candidate_handler
            .replace(Some(js_ice_candidate_handler));
        debug_assert!(prev_handler.is_none());
    }

    #[must_use]
    pub fn add_data_channel<T: AsRef<str>>(
        self: &Arc<Self>,
        name: T,
        handler: BoxAsyncFn2<Arc<DataSender>, DataSenderEvent, ()>,
    ) -> Arc<DataSender> {
        DataSender::new(Arc::clone(self), self.js_connection.clone(), name, handler)
    }

    #[must_use]
    pub fn add_media_stream(self: &Arc<Self>, media_stream: MediaStream) -> Arc<MediaSender> {
        MediaSender::new(Arc::clone(self), self.js_connection.clone(), media_stream)
    }

    pub async fn start(self: &Arc<Self>) -> Result<(), SenderStartError> {
        use core::sync::atomic::Ordering;

        if self.is_started.swap(true, Ordering::Relaxed) {
            Err(SenderStartError::AlreadyStarted)
        } else {
            self.send_offer().await?;
            self.init_negotiation_needed_handler();
            Ok(())
        }
    }

    fn init_negotiation_needed_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_negotiation_needed_handler = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |ev: Event| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_negotiation_needed_event(ev).await });
            })
        };
        self.js_connection
            .set_onnegotiationneeded(Some(js_negotiation_needed_handler.as_ref().unchecked_ref()));
        let prev_handler = self
            .js_negotiation_needed_handler
            .replace(Some(js_negotiation_needed_handler));
        debug_assert!(prev_handler.is_none());
    }

    fn init_ice_connection_state_change_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_ice_connection_state_change_handler = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |ev: Event| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_ice_connection_state_change(ev).await });
            })
        };
        self.js_connection.set_oniceconnectionstatechange(Some(
            js_ice_connection_state_change_handler
                .as_ref()
                .unchecked_ref(),
        ));
        let prev_handler = self
            .js_ice_connection_state_change_handler
            .replace(Some(js_ice_connection_state_change_handler));
        debug_assert!(prev_handler.is_none());
    }

    fn init_ice_gathering_state_change_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_ice_gathering_state_change = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |ev: Event| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_ice_gathering_state_change(ev).await });
            })
        };
        self.js_connection.set_onicegatheringstatechange(Some(
            js_ice_gathering_state_change.as_ref().unchecked_ref(),
        ));
        let prev_handler = self
            .js_ice_gathering_state_change
            .replace(Some(js_ice_gathering_state_change));
        debug_assert!(prev_handler.is_none());
    }

    fn init_signaling_state_change_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_signaling_state_change_change = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |ev: Event| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_signaling_state_change(ev).await });
            })
        };
        self.js_connection.set_onsignalingstatechange(Some(
            js_signaling_state_change_change.as_ref().unchecked_ref(),
        ));
        let prev_handler = self
            .js_signaling_state_change_change
            .replace(Some(js_signaling_state_change_change));
        debug_assert!(prev_handler.is_none());
    }

    fn send_message(&self, message: ClientSenderMessage) -> Result<(), SenderSendError> {
        let message = ClientMessage::SenderMessage {
            sender_id: self.sender_id,
            message,
        };
        send_websocket_client_message(&self.js_websocket, message)?;
        Ok(())
    }

    async fn handler(self: &Arc<Self>, ev: SenderEvent) {
        self.handler.0(Arc::clone(self), ev).await
    }

    async fn error(self: &Arc<Self>, err: SenderError) {
        self.handler(SenderEvent::Error(err)).await
    }

    pub(crate) async fn on_server_message(self: &Arc<Self>, message: ServerSenderMessage) {
        match self.clone().handle_server_message(message).await {
            Ok(()) => {}
            Err(err) => self.error(err).await,
        }
    }

    async fn handle_server_message(
        self: &Arc<Self>,
        message: ServerSenderMessage,
    ) -> Result<(), SenderError> {
        use wasm_bindgen_futures::JsFuture;
        use ServerSenderMessage as Msg;

        match message {
            Msg::OpenChannelSuccess => {
                self.handler(SenderEvent::OpenChannelSuccess).await;
                Ok(())
            }
            Msg::ChannelAnswer(sdp) => {
                self.receive_answer(sdp).await?;
                Ok(())
            }
            Msg::IceCandidate(ice_candidate) => {
                let mut candidate = RtcIceCandidateInit::new(&ice_candidate.candidate);
                let _: &mut _ = candidate
                    .sdp_mid(ice_candidate.sdp_mid.as_deref())
                    .sdp_m_line_index(ice_candidate.sdp_m_line_index);
                let candidate = RtcIceCandidate::new(&candidate)
                    .map_err(SenderError::NewRtcIceCandidateError)?;

                let ice_candidate_result = JsFuture::from(
                    self.js_connection
                        .add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate)),
                )
                .await;
                match ice_candidate_result {
                    Ok(_) => {}
                    Err(err) => self.error(SenderError::AddIceCandidateError(err)).await,
                };

                Ok(())
            }
            Msg::AllIceCandidatesSent => Ok(()),
            Msg::Error(err) => match err {
                ServerSenderErrorMessage::ChannelIdIsAlreadyUsed(channel_id) => {
                    Err(SenderError::ChannelIdIsAlreadyUsed(channel_id))
                }
                _ => panic!("invalid SessionSenderId used"),
            },
        }
    }

    async fn on_ice_candidate_event(self: &Arc<Self>, ev: RtcPeerConnectionIceEvent) {
        log::trace!("browser_webrtc::Sender::on_ice_candidate_event");

        match self.handle_ice_candidate_event(ev).await {
            Ok(()) => {}
            Err(err) => self.error(err).await,
        }
    }

    async fn handle_ice_candidate_event(
        &self,
        ev: RtcPeerConnectionIceEvent,
    ) -> Result<(), SenderError> {
        use signaling_protocol::IceCandidate;

        if let Some(candidate) = ev.candidate() {
            let candidate_str = candidate.candidate();
            let message = match candidate_str.as_ref() {
                "" => ClientSenderMessage::AllIceCandidatesSent,
                _ => {
                    let ice_candidate = IceCandidate {
                        candidate: candidate_str,
                        sdp_mid: candidate.sdp_mid(),
                        sdp_m_line_index: candidate.sdp_m_line_index(),
                    };
                    ClientSenderMessage::IceCandidate(ice_candidate)
                }
            };
            let message = ClientMessage::SenderMessage {
                sender_id: self.sender_id,
                message,
            };
            send_websocket_client_message(&self.js_websocket, message)
                .map_err(SenderError::IceCandidateSendError)?;
        }
        Ok(())
    }

    async fn on_negotiation_needed_event(self: &Arc<Self>, ev: Event) {
        log::trace!("browser_webrtc::Sender::on_negotiation_needed_event");

        match self.handle_negotiation_needed_event(ev).await {
            Ok(()) => {}
            Err(err) => self.error(err).await,
        }
    }

    async fn handle_negotiation_needed_event(&self, _: Event) -> Result<(), SenderError> {
        self.send_offer().await?;
        Ok(())
    }

    async fn on_ice_connection_state_change(self: &Arc<Self>, _: Event) {
        log::trace!("browser_webrtc::Receiver::on_ice_connection_state_change");

        self.handler(SenderEvent::IceConnectionStateChange(
            self.ice_connection_state(),
        ))
        .await
    }

    async fn on_ice_gathering_state_change(self: &Arc<Self>, _: Event) {
        log::trace!("browser_webrtc::Receiver::on_ice_gathering_state_change");

        self.handler(SenderEvent::IceGatheringStateChange(
            self.ice_gathering_state(),
        ))
        .await
    }

    async fn on_signaling_state_change(self: &Arc<Self>, _: Event) {
        log::trace!("browser_webrtc::Receiver::on_signaling_state_change");

        self.handler(SenderEvent::RtcSignalingStateChange(self.signaling_state()))
            .await
    }

    pub fn ice_connection_state(&self) -> RtcIceConnectionState {
        self.js_connection.ice_connection_state()
    }

    pub fn ice_gathering_state(&self) -> RtcIceGatheringState {
        self.js_connection.ice_gathering_state()
    }

    pub fn signaling_state(&self) -> RtcSignalingState {
        self.js_connection.signaling_state()
    }

    async fn send_offer(&self) -> Result<(), SenderSendOfferError> {
        log::trace!("browser_webrtc::Sender::send_offer");

        use js_sys::Reflect;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::JsFuture;
        use web_sys::RtcSessionDescriptionInit;

        let offer = JsFuture::from(self.js_connection.create_offer())
            .await
            .map_err(SenderSendOfferError::CreateOfferError)?;

        let offer: &RtcSessionDescriptionInit = offer.as_ref().unchecked_ref();

        let _: JsValue = JsFuture::from(self.js_connection.set_local_description(offer))
            .await
            .map_err(SenderSendOfferError::SetLocalDescriptionError)?;

        let sdp = Reflect::get(&offer, &JsValue::from_str("sdp"))
            .unwrap()
            .as_string()
            .unwrap();

        self.send_message(ClientSenderMessage::SendOffer(SessionDescription(sdp)))?;

        Ok(())
    }

    async fn receive_answer(
        &self,
        remote_sdp: SessionDescription,
    ) -> Result<(), SenderReceiveAnswerError> {
        use wasm_bindgen_futures::JsFuture;
        use web_sys::{RtcSdpType, RtcSessionDescriptionInit};

        let mut remote_description = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
        let _: &mut _ = remote_description.sdp(&remote_sdp.0);

        let _: JsValue = JsFuture::from(
            self.js_connection
                .set_remote_description(&remote_description),
        )
        .await
        .map_err(SenderReceiveAnswerError::SetRemoteDescriptionError)?;

        Ok(())
    }

    pub fn send_binary_data(&self, data: Vec<u8>) -> Result<(), SenderSendError> {
        self.send_message(ClientSenderMessage::SendBinaryData(data))
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        use wasm_bindgen_futures::spawn_local;

        log::trace!("browser_webrtc::Sender::drop");

        self.js_connection.set_onnegotiationneeded(None);
        self.js_connection.set_onicecandidate(None);
        self.js_connection.close();

        let server = Arc::clone(&self.server);
        let sender_id = self.sender_id;
        let _: Option<()> = self.send_message(ClientSenderMessage::CloseChannel).ok();
        spawn_local(async move { server.on_sender_dropped(sender_id).await });
    }
}

#[derive(Debug)]
pub enum SenderEvent {
    OpenChannelSuccess,
    IceConnectionStateChange(RtcIceConnectionState),
    IceGatheringStateChange(RtcIceGatheringState),
    RtcSignalingStateChange(RtcSignalingState),
    Error(SenderError),
}

#[derive(Error, Debug)]
pub enum SenderError {
    //#[error("client message send error: {0}")]
    //SendError(#[from] WebSocketClientMessageSendError),
    #[error("client message send error: {0}")]
    IceCandidateSendError(WebSocketClientMessageSendError),
    #[error("channel id is already used: {0:?}")]
    ChannelIdIsAlreadyUsed(ChannelId),
    #[error("new RtcIceCandidate error: {}", 0.0)]
    NewRtcIceCandidateError(JsValue),
    #[error("add ice candidate error: {}", 0.0)]
    AddIceCandidateError(JsValue),
    #[error(transparent)]
    SendOfferError(#[from] SenderSendOfferError),
    #[error(transparent)]
    ReceiveAnswerError(#[from] SenderReceiveAnswerError),
}

#[derive(Error, Debug)]
pub enum NewSenderError {
    #[error(transparent)]
    SendError(#[from] WebSocketClientMessageSendError),
    #[error("new RtcPeerConnection error: {0:?}")]
    NewRtcPeerConnectionError(JsValue),
}

#[derive(Error, Debug)]
pub enum SenderStartError {
    #[error("sender is already started")]
    AlreadyStarted,
    #[error(transparent)]
    SendOfferError(#[from] SenderSendOfferError),
}

#[derive(Error, Debug)]
pub enum SenderSendOfferError {
    #[error("create_offer error: {0:?}")]
    CreateOfferError(JsValue),
    #[error("set_local_description error: {0:?}")]
    SetLocalDescriptionError(JsValue),
    #[error("offer send error: {0}")]
    SendError(#[from] SenderSendError),
}

#[derive(Error, Debug)]
pub enum SenderReceiveAnswerError {
    #[error("set_remote_description error: {0:?}")]
    SetRemoteDescriptionError(JsValue),
}

#[derive(Error, Debug)]
pub enum SenderSendError {
    #[error(transparent)]
    SendError(#[from] WebSocketClientMessageSendError),
}

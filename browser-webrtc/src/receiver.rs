use core::cell::RefCell;
use core::sync::atomic::AtomicBool;

use async_std::sync::Arc;
use js_sys::Set;
use signaling_protocol::{
    ChannelId, ClientMessage, ClientReceiverMessage, ServerReceiverErrorMessage,
    ServerReceiverMessage, SessionDescription, SessionReceiverId,
};
use thiserror::Error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use web_sys::{
    Event, MediaStream, RtcConfiguration, RtcDataChannelEvent, RtcIceCandidate,
    RtcIceCandidateInit, RtcIceConnectionState, RtcIceGatheringState, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcSignalingState, RtcTrackEvent, WebSocket,
};

use crate::{
    send_websocket_client_message, BoxAsyncFn2, BoxAsyncFn2Wrapper, DataReceiverBuilder,
    MediaReceiverBuilder, Server, WebSocketClientMessageSendError,
};

#[derive(Debug)]
pub struct Receiver {
    server: Arc<Server>,
    receiver_id: SessionReceiverId,
    handler: BoxAsyncFn2Wrapper<Arc<Receiver>, ReceiverEvent, ()>,
    js_connection: RtcPeerConnection,
    js_websocket: WebSocket,
    js_ice_candidate_handler: RefCell<Option<Closure<dyn FnMut(RtcPeerConnectionIceEvent)>>>,
    js_negotiation_needed_handler: RefCell<Option<Closure<dyn FnMut(Event)>>>,
    js_data_channel_handler: RefCell<Option<Closure<dyn FnMut(RtcDataChannelEvent)>>>,
    js_track_handler: RefCell<Option<Closure<dyn FnMut(RtcTrackEvent)>>>,
    js_ice_connection_state_change_handler: RefCell<Option<Closure<dyn FnMut(Event)>>>,
    js_ice_gathering_state_change: RefCell<Option<Closure<dyn FnMut(Event)>>>,
    js_signaling_state_change_change: RefCell<Option<Closure<dyn FnMut(Event)>>>,
    js_media_streams: Set,
    js_media_tracks: Set,
    is_started: AtomicBool,
}

impl Receiver {
    pub fn new(
        js_websocket: WebSocket,
        server: Arc<Server>,
        receiver_id: SessionReceiverId,
        channel_id: ChannelId,
        handler: BoxAsyncFn2<Arc<Self>, ReceiverEvent, ()>,
        rtc_configuration: Option<RtcConfiguration>,
    ) -> Result<Arc<Self>, NewReceiverError> {
        log::trace!("browser_webrtc::Receiver::new");

        let message = ClientMessage::ReceiverMessage {
            receiver_id,
            message: ClientReceiverMessage::JoinChannel { channel_id },
        };
        send_websocket_client_message(&js_websocket, message)?;

        let js_connection = match rtc_configuration {
            Some(config) => RtcPeerConnection::new_with_configuration(&config),
            None => RtcPeerConnection::new(),
        }
        .map_err(NewReceiverError::NewRtcPeerConnectionError)?;

        let receiver = Arc::new(Self {
            server,
            receiver_id,
            handler: BoxAsyncFn2Wrapper(handler),
            js_connection: js_connection.clone(),
            js_websocket,
            js_ice_candidate_handler: RefCell::new(None),
            js_negotiation_needed_handler: RefCell::new(None),
            js_data_channel_handler: RefCell::new(None),
            js_track_handler: RefCell::new(None),
            js_ice_connection_state_change_handler: RefCell::new(None),
            js_ice_gathering_state_change: RefCell::new(None),
            js_signaling_state_change_change: RefCell::new(None),
            js_media_streams: Set::new(&JsValue::UNDEFINED),
            js_media_tracks: Set::new(&JsValue::UNDEFINED),
            is_started: AtomicBool::new(false),
        });

        receiver.init_icecandidate_handler();
        receiver.init_data_channel_handler();
        receiver.init_track_handler();
        receiver.init_ice_connection_state_change_handler();
        receiver.init_ice_gathering_state_change_handler();
        receiver.init_signaling_state_change_handler();

        Ok(receiver)
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

    fn init_data_channel_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_data_channel_handler = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |ev: RtcDataChannelEvent| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_data_channel_event(ev).await });
            })
        };
        self.js_connection
            .set_ondatachannel(Some(js_data_channel_handler.as_ref().unchecked_ref()));
        let prev_handler = self
            .js_data_channel_handler
            .replace(Some(js_data_channel_handler));
        debug_assert!(prev_handler.is_none());
    }

    fn init_track_handler(self: &Arc<Self>) {
        use crate::closure_1;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::spawn_local;

        let js_track_handler = {
            let self_weak = Arc::downgrade(&self);
            closure_1(move |ev: RtcTrackEvent| {
                let self_arc = self_weak.upgrade().unwrap();
                spawn_local(async move { self_arc.on_track_event(ev).await });
            })
        };
        self.js_connection
            .set_ontrack(Some(js_track_handler.as_ref().unchecked_ref()));
        let prev_handler = self.js_track_handler.replace(Some(js_track_handler));
        debug_assert!(prev_handler.is_none());
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

    fn send_message(&self, message: ClientReceiverMessage) -> Result<(), ReceiverSendError> {
        let message = ClientMessage::ReceiverMessage {
            receiver_id: self.receiver_id,
            message,
        };
        send_websocket_client_message(&self.js_websocket, message)?;
        Ok(())
    }

    async fn handler(self: &Arc<Self>, ev: ReceiverEvent) {
        self.handler.0(Arc::clone(self), ev).await
    }

    async fn error(self: &Arc<Self>, err: ReceiverError) {
        self.handler(ReceiverEvent::Error(err)).await
    }

    pub(crate) async fn on_server_message(self: &Arc<Self>, message: ServerReceiverMessage) {
        match self.clone().handle_server_message(message).await {
            Ok(()) => {}
            Err(err) => self.error(err).await,
        }
    }

    async fn handle_server_message(
        self: &Arc<Self>,
        message: ServerReceiverMessage,
    ) -> Result<(), ReceiverError> {
        use wasm_bindgen_futures::JsFuture;
        use ServerReceiverMessage as Msg;

        match message {
            Msg::JoinChannelSuccess => {
                self.handler(ReceiverEvent::JoinChannelSuccess).await;
                Ok(())
            }
            Msg::ChannelOffer(sdp) => {
                self.receive_offer_and_send_answer(sdp).await?;
                Ok(())
            }
            Msg::IceCandidate(ice_candidate) => {
                let mut candidate = RtcIceCandidateInit::new(&ice_candidate.candidate);
                let _: &mut _ = candidate
                    .sdp_mid(ice_candidate.sdp_mid.as_deref())
                    .sdp_m_line_index(ice_candidate.sdp_m_line_index);
                let candidate = RtcIceCandidate::new(&candidate)
                    .map_err(ReceiverError::NewRtcIceCandidateError)?;

                let ice_candidate_result = JsFuture::from(
                    self.js_connection
                        .add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate)),
                )
                .await;
                match ice_candidate_result {
                    Ok(_) => {}
                    Err(err) => self.error(ReceiverError::AddIceCandidateError(err)).await,
                };

                Ok(())
            }
            Msg::AllIceCandidatesSent => Ok(()),
            Msg::BinaryData(data) => {
                self.handler(ReceiverEvent::BinaryData(data)).await;
                Ok(())
            }
            Msg::Error(err) => match err {
                ServerReceiverErrorMessage::ChannelIsNotExist(channel_id) => {
                    Err(ReceiverError::ChannelIsNotExist(channel_id))
                }
                ServerReceiverErrorMessage::ChannelIsAlreadyOccupied(channel_id) => {
                    Err(ReceiverError::ChannelIsAlreadyOccupied(channel_id))
                }
                _ => panic!("invalid SessionReceiverId used"),
            },
        }
    }

    async fn on_ice_candidate_event(self: &Arc<Self>, ev: RtcPeerConnectionIceEvent) {
        log::trace!("browser_webrtc::Receiver::on_ice_candidate_event");

        match self.handle_ice_candidate_event(ev).await {
            Ok(()) => {}
            Err(err) => self.error(err).await,
        }
    }

    async fn handle_ice_candidate_event(
        &self,
        ev: RtcPeerConnectionIceEvent,
    ) -> Result<(), ReceiverError> {
        use signaling_protocol::IceCandidate;

        if let Some(candidate) = ev.candidate() {
            let candidate_str = candidate.candidate();
            let message = match candidate_str.as_ref() {
                "" => ClientReceiverMessage::AllIceCandidatesSent,
                _ => {
                    let ice_candidate = IceCandidate {
                        candidate: candidate_str,
                        sdp_mid: candidate.sdp_mid(),
                        sdp_m_line_index: candidate.sdp_m_line_index(),
                    };
                    ClientReceiverMessage::IceCandidate(ice_candidate)
                }
            };
            let message = ClientMessage::ReceiverMessage {
                receiver_id: self.receiver_id,
                message,
            };
            send_websocket_client_message(&self.js_websocket, message)
                .map_err(ReceiverError::IceCandidateSendError)?;
        }
        Ok(())
    }

    async fn on_data_channel_event(self: &Arc<Self>, ev: RtcDataChannelEvent) {
        log::trace!("browser_webrtc::Receiver::on_data_channel_event");

        let data_receiver = DataReceiverBuilder::new(Arc::clone(&self), ev.channel());
        self.handler(ReceiverEvent::DataReceiver(data_receiver))
            .await
    }

    async fn on_track_event(self: &Arc<Self>, ev: RtcTrackEvent) {
        log::trace!("browser_webrtc::Receiver::on_track_event");

        match self.handle_track_event(ev).await {
            Ok(()) => {}
            Err(err) => self.error(err).await,
        }
    }

    async fn handle_track_event(self: &Arc<Self>, ev: RtcTrackEvent) -> Result<(), ReceiverError> {
        use wasm_bindgen::JsCast;

        if ev.streams().iter().count() == 0 {
            if self.js_media_tracks.has(&ev.track()) {
                return Ok(());
            }
            let track = ev.track();
            let stream = MediaStream::new().map_err(ReceiverError::NewMediaStreamFailed)?;
            stream.add_track(&track);
            let _: Set = self.js_media_streams.add(&stream);
            let _: Set = self.js_media_tracks.add(&track);

            let media_receiver = MediaReceiverBuilder::new(Arc::clone(&self), stream);
            self.handler(ReceiverEvent::MediaReceiver(media_receiver))
                .await;
        } else {
            for stream in ev.streams().iter() {
                if self.js_media_streams.has(&stream) {
                    continue;
                }
                let stream: Result<MediaStream, _> = stream.dyn_into();
                match stream {
                    Ok(stream) => {
                        let _: Set = self.js_media_streams.add(&stream);
                        for track in stream.get_tracks().iter() {
                            let _: Set = self.js_media_tracks.add(&track);
                        }

                        let media_receiver = MediaReceiverBuilder::new(Arc::clone(&self), stream);
                        self.handler(ReceiverEvent::MediaReceiver(media_receiver))
                            .await;
                    }
                    Err(err) => {
                        self.error(ReceiverError::InvalidTrackEventMediaStream(err))
                            .await
                    }
                }
            }
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

    async fn handle_negotiation_needed_event(&self, _: Event) -> Result<(), ReceiverError> {
        self.send_answer().await?;
        Ok(())
    }

    async fn on_ice_connection_state_change(self: &Arc<Self>, _: Event) {
        log::trace!("browser_webrtc::Receiver::on_ice_connection_state_change");

        self.handler(ReceiverEvent::IceConnectionStateChange(
            self.ice_connection_state(),
        ))
        .await
    }

    async fn on_ice_gathering_state_change(self: &Arc<Self>, _: Event) {
        log::trace!("browser_webrtc::Receiver::on_ice_gathering_state_change");

        self.handler(ReceiverEvent::IceGatheringStateChange(
            self.ice_gathering_state(),
        ))
        .await
    }

    async fn on_signaling_state_change(self: &Arc<Self>, _: Event) {
        log::trace!("browser_webrtc::Receiver::on_signaling_state_change");

        self.handler(ReceiverEvent::RtcSignalingStateChange(
            self.signaling_state(),
        ))
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

    async fn receive_offer_and_send_answer(
        self: &Arc<Self>,
        remote_sdp: SessionDescription,
    ) -> Result<(), ReceiveReceiveOfferAndSendAnswerError> {
        log::trace!("browser_webrtc::Receiver::receive_offer_and_send_answer");

        use wasm_bindgen_futures::JsFuture;
        use web_sys::{RtcSdpType, RtcSessionDescriptionInit};

        use ReceiveReceiveOfferAndSendAnswerError as Event;

        let mut remote_description = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
        let _: &mut _ = remote_description.sdp(&remote_sdp.0);

        let _: JsValue = JsFuture::from(
            self.js_connection
                .set_remote_description(&remote_description),
        )
        .await
        .map_err(Event::SetRemoteDescriptionError)?;

        self.send_answer().await?;
        self.init_negotiation_needed_handler();

        Ok(())
    }

    async fn send_answer(&self) -> Result<(), ReceiveReceiveOfferAndSendAnswerError> {
        log::trace!("browser_webrtc::Receiver::send_answer");

        use js_sys::Reflect;
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::JsFuture;
        use web_sys::RtcSessionDescriptionInit;

        use ReceiveReceiveOfferAndSendAnswerError as Event;

        let offer = JsFuture::from(self.js_connection.create_answer())
            .await
            .map_err(Event::CreateAnswerError)?;

        let offer: &RtcSessionDescriptionInit = offer.as_ref().unchecked_ref();

        let _: JsValue = JsFuture::from(self.js_connection.set_local_description(&offer))
            .await
            .map_err(Event::SetLocalDescriptionError)?;

        let local_sdp = Reflect::get(&offer, &JsValue::from_str("sdp"))
            .unwrap()
            .as_string()
            .unwrap();

        self.send_message(ClientReceiverMessage::SendAnswer(SessionDescription(
            local_sdp,
        )))?;

        Ok(())
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        use wasm_bindgen_futures::spawn_local;

        log::trace!("browser_webrtc::Receiver::drop");

        self.js_connection.set_onicecandidate(None);
        self.js_connection.close();

        let server = Arc::clone(&self.server);
        let receiver_id = self.receiver_id;
        let _: Option<()> = self.send_message(ClientReceiverMessage::ExitChannel).ok();
        spawn_local(async move { server.on_receiver_dropped(receiver_id).await });
    }
}

#[derive(Debug)]
pub enum ReceiverEvent {
    ServerMessage(ServerReceiverMessage),
    DataReceiver(DataReceiverBuilder),
    MediaReceiver(MediaReceiverBuilder),
    IceConnectionStateChange(RtcIceConnectionState),
    IceGatheringStateChange(RtcIceGatheringState),
    RtcSignalingStateChange(RtcSignalingState),
    JoinChannelSuccess,
    BinaryData(Vec<u8>),
    Error(ReceiverError),
}

#[derive(Error, Debug)]
pub enum ReceiverError {
    //#[error("client message send error: {0}")]
    //SendError(#[from] WebSocketClientMessageSendError),
    #[error("client message send error: {0}")]
    IceCandidateSendError(WebSocketClientMessageSendError),
    #[error("channel id is not exist: {0:?}")]
    ChannelIsNotExist(ChannelId),
    #[error("channel id is already occupied: {0:?}")]
    ChannelIsAlreadyOccupied(ChannelId),
    #[error("new RtcIceCandidate error: {}", 0.0)]
    NewRtcIceCandidateError(JsValue),
    #[error("add ice candidate error: {}", 0.0)]
    AddIceCandidateError(JsValue),
    #[error(transparent)]
    ReceiveReceiveOfferAndSendAnswer(#[from] ReceiveReceiveOfferAndSendAnswerError),
    #[error("inalid MediaStream received in track event: {}", 0.0)]
    InvalidTrackEventMediaStream(JsValue),
    #[error("new MediaStream error: {}", 0.0)]
    NewMediaStreamFailed(JsValue),
}

#[derive(Error, Debug)]
pub enum NewReceiverError {
    #[error(transparent)]
    SendError(#[from] WebSocketClientMessageSendError),
    #[error("new RtcPeerConnection error: {0:?}")]
    NewRtcPeerConnectionError(JsValue),
}

#[derive(Error, Debug)]
pub enum ReceiveReceiveOfferAndSendAnswerError {
    #[error("set_remote_description error: {0:?}")]
    SetRemoteDescriptionError(JsValue),
    #[error("create_answer error: {0:?}")]
    CreateAnswerError(JsValue),
    #[error("set_local_description error: {0:?}")]
    SetLocalDescriptionError(JsValue),
    #[error("answer send error: {0}")]
    SendError(#[from] ReceiverSendError),
}

#[derive(Error, Debug)]
pub enum ReceiverSendError {
    #[error(transparent)]
    SendError(#[from] WebSocketClientMessageSendError),
}

#[derive(Error, Debug)]
pub enum ReceiverIceCandidateError {
    #[error("client message send error: {0}")]
    SendError(#[from] WebSocketClientMessageSendError),
}

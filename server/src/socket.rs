use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures::stream::SplitStream;
use signaling_protocol::{
    ChannelId, ClientReceiverMessage, ClientSenderMessage, IceCandidate, NetworkMode,
    ServerReceiverErrorMessage, ServerSenderErrorMessage, SessionDescription, SessionReceiverId,
    SessionSenderId,
};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::WebSocketStream;

use crate::{Channel, ChannelReceiver, ServerData, SocketSender};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SocketId(pub u32);

#[derive(Debug)]
pub struct Socket {
    socket_id: SocketId,
    server_data: Arc<ServerData>,
    socket_sender: Arc<SocketSender>,
    socket_receiver: SplitStream<WebSocketStream<TcpStream>>,
    channel_senders: HashMap<SessionSenderId, Arc<Channel>>,
    channel_receivers: HashMap<SessionReceiverId, Arc<ChannelReceiver>>,
    addr: SocketAddr,
}

impl Socket {
    pub async fn new(
        socket_id: SocketId,
        server_data: Arc<ServerData>,
        stream: TcpStream,
        addr: SocketAddr,
    ) -> Result<Self, NewSessionError> {
        use futures::StreamExt;
        use log::info;
        use tokio_tungstenite::accept_async;

        let websocket = accept_async(stream).await.unwrap();
        let (socket_sender, socket_receiver) = websocket.split();
        let socket_sender = Arc::new(SocketSender::new(socket_sender));
        info!("new session: {}", addr);

        let prev_sender = server_data
            .senders()
            .write()
            .await
            .insert(socket_id, Arc::downgrade(&socket_sender));
        assert!(prev_sender.is_none());

        server_data.update_open_channel_ids().await;

        Ok(Self {
            socket_id,
            server_data,
            socket_sender,
            socket_receiver,
            channel_senders: HashMap::new(),
            channel_receivers: HashMap::new(),
            addr,
        })
    }

    pub async fn run(mut self) {
        use bincode::deserialize;
        use futures::stream::StreamExt;
        use log::{debug, error, info};
        use signaling_protocol::ClientMessage;

        loop {
            let message = self.socket_receiver.next().await.unwrap().unwrap();
            match message {
                Message::Binary(data) => {
                    let message: Result<ClientMessage, _> = deserialize(&data[..]);
                    debug!("client message: {}, {:?}", self.addr, message);
                    match message {
                        Ok(ClientMessage::SenderMessage { sender_id, message }) => match message {
                            ClientSenderMessage::OpenChannel {
                                channel_id,
                                network_mode,
                            } => self.open_channel(sender_id, channel_id, network_mode).await,
                            ClientSenderMessage::CloseChannel => {
                                self.close_channel(sender_id).await
                            }
                            ClientSenderMessage::SendOffer(sdp) => {
                                self.send_offer(sender_id, sdp).await
                            }
                            ClientSenderMessage::IceCandidate(ice_candidate) => {
                                self.sender_ice_candidate(sender_id, ice_candidate).await
                            }
                            ClientSenderMessage::AllIceCandidatesSent => {
                                self.sender_all_ice_candidate_sent(sender_id).await
                            }
                            ClientSenderMessage::SendBinaryData(data) => {
                                self.send_binary_data(sender_id, data).await
                            }
                        },
                        Ok(ClientMessage::ReceiverMessage {
                            receiver_id,
                            message,
                        }) => match message {
                            ClientReceiverMessage::JoinChannel { channel_id } => {
                                self.join_channel(receiver_id, channel_id).await
                            }
                            ClientReceiverMessage::ExitChannel => {
                                self.exit_channel(receiver_id).await
                            }
                            ClientReceiverMessage::SendAnswer(sdp) => {
                                self.send_answer(receiver_id, sdp).await
                            }
                            ClientReceiverMessage::IceCandidate(ice_candidate) => {
                                self.receiver_ice_candidate(receiver_id, ice_candidate)
                                    .await
                            }
                            ClientReceiverMessage::AllIceCandidatesSent => {
                                self.receiver_all_ice_candidate_sent(receiver_id).await
                            }
                        },
                        Err(err) => {
                            error!("ClientMessage deserialization error {}", err);
                        }
                    }
                }
                Message::Close(_) => {
                    info!("session closed: {}", self.addr);
                    break;
                }
                _ => {
                    info!(
                        "invalid client message: {:?}, session closed: {}",
                        message, self.addr
                    );
                    break;
                }
            }
        }
        self.clear().await;
    }

    pub async fn clear(mut self) {
        use core::mem::take;

        let senders = take(&mut self.channel_senders);
        let channel_ids = senders
            .into_iter()
            .filter_map(|(_, channel)| channel.channel_id.upgrade());
        self.server_data.remove_channels(channel_ids).await;

        let prev_sender = self
            .server_data
            .senders()
            .write()
            .await
            .remove(&self.socket_id);
        assert!(prev_sender.is_some());
    }

    pub async fn open_channel(
        &mut self,
        session_sender_id: SessionSenderId,
        channel_id: ChannelId,
        network_mode: NetworkMode,
    ) {
        use crate::{ChannelIceCandidates, ChannelKind, ChannelSender};
        use std::collections::hash_map::Entry;
        use tokio::sync::RwLock;

        let session_channel_entry = match self.channel_senders.entry(session_sender_id) {
            Entry::Occupied(_) => {
                self.socket_sender
                    .send_sender_error(
                        session_sender_id,
                        ServerSenderErrorMessage::SessionSenderIdIsAlreadyUsed,
                    )
                    .await;
                return;
            }
            Entry::Vacant(entry) => entry,
        };

        let channel_id = Arc::new(channel_id);
        let mut channels = self.server_data.channels().write().await;
        let server_channel_entry = match channels.entry(Arc::clone(&channel_id)) {
            Entry::Occupied(_) => {
                self.socket_sender
                    .send_sender_error(
                        session_sender_id,
                        ServerSenderErrorMessage::ChannelIdIsAlreadyUsed(
                            channel_id.as_ref().to_owned(),
                        ),
                    )
                    .await;
                return;
            }
            Entry::Vacant(entry) => entry,
        };

        let channel = match network_mode {
            NetworkMode::PeerToPeer => Channel {
                channel_id: Arc::downgrade(&channel_id),
                sender: ChannelSender {
                    socket_sender: Arc::downgrade(&self.socket_sender),
                    session_sender_id,
                    session_description: RwLock::new(None),
                    ice_candidates: RwLock::new(ChannelIceCandidates::new()),
                },
                kind: ChannelKind::PeerToPeer {
                    receiver: RwLock::new(None),
                },
            },
            NetworkMode::ClientServer => {
                log::error!("not implemented"); // TODO
                return;
            }
        };

        let channel = Arc::new(channel);
        let _: &mut _ = server_channel_entry.insert(Arc::downgrade(&channel));
        let _: &mut _ = session_channel_entry.insert(channel);
        drop(channels);

        self.server_data.update_open_channel_ids().await;
    }

    pub async fn join_channel(
        &mut self,
        session_receiver_id: SessionReceiverId,
        channel_id: ChannelId,
    ) {
        use crate::{ChannelIceCandidates, ChannelKind};
        use std::collections::hash_map::Entry;
        use tokio::sync::RwLock;

        let session_channel_entry = match self.channel_receivers.entry(session_receiver_id) {
            Entry::Occupied(_) => {
                self.socket_sender
                    .send_receiver_error(
                        session_receiver_id,
                        ServerReceiverErrorMessage::SessionReceiverIdIsAlreadyUsed,
                    )
                    .await;
                return;
            }
            Entry::Vacant(entry) => entry,
        };

        let channel_id = Arc::new(channel_id);
        let channels = self.server_data.channels().write().await;
        let channel = channels
            .get(&channel_id)
            .and_then(|channel| channel.upgrade());
        let channel = match channel {
            Some(channel) => channel,
            None => {
                self.socket_sender
                    .send_receiver_error(
                        session_receiver_id,
                        ServerReceiverErrorMessage::ChannelIsNotExist(
                            channel_id.as_ref().to_owned(),
                        ),
                    )
                    .await;
                return;
            }
        };

        let channel_receiver = Arc::new(ChannelReceiver {
            channel: Arc::downgrade(&channel),
            socket_sender: Arc::downgrade(&self.socket_sender),
            session_receiver_id,
            session_description: RwLock::new(None),
            ice_candidates: RwLock::new(ChannelIceCandidates::new()),
        });

        let session_description = channel.sender.session_description.read().await;
        let ice_candidates = channel.sender.ice_candidates.read().await;

        match &channel.kind {
            ChannelKind::PeerToPeer { receiver } => {
                let mut receiver = receiver.write().await;
                if let Some(_) = receiver.as_ref().and_then(|receiver| receiver.upgrade()) {
                    self.socket_sender
                        .send_receiver_error(
                            session_receiver_id,
                            ServerReceiverErrorMessage::ChannelIsAlreadyOccupied(
                                channel_id.as_ref().to_owned(),
                            ),
                        )
                        .await;
                    return;
                }
                let _: Option<_> = receiver.replace(Arc::downgrade(&channel_receiver));
                channel_receiver
                    .send_offer_and_ice_candidates(session_description.as_ref(), &ice_candidates)
                    .await
            }
            ChannelKind::ClientServer { .. } => {
                log::error!("not implemented"); // TODO
                return;
            }
        }

        drop(session_description);
        drop(ice_candidates);

        let _: &mut _ = session_channel_entry.insert(channel_receiver);
        drop(channels);

        match &channel.kind {
            ChannelKind::PeerToPeer { .. } => {
                self.server_data.update_open_channel_ids().await;
            }
            ChannelKind::ClientServer { .. } => {}
        }
    }

    pub async fn get_channel(&mut self, sender_id: SessionSenderId) -> Option<&Arc<Channel>> {
        match self.channel_senders.get(&sender_id) {
            Some(channel) => Some(channel),
            None => {
                self.socket_sender
                    .send_sender_error(
                        sender_id,
                        ServerSenderErrorMessage::SessionSenderIdIsNotExist,
                    )
                    .await;
                None
            }
        }
    }

    pub async fn get_receiver(
        &mut self,
        receiver_id: SessionReceiverId,
    ) -> Option<&Arc<ChannelReceiver>> {
        match self.channel_receivers.get(&receiver_id) {
            Some(channel) => Some(channel),
            None => {
                self.socket_sender
                    .send_receiver_error(
                        receiver_id,
                        ServerReceiverErrorMessage::SessionReceiverIdIsNotExist,
                    )
                    .await;
                None
            }
        }
    }

    pub async fn close_channel(&mut self, sender_id: SessionSenderId) {
        let channel = self.channel_senders.remove(&sender_id);
        if channel.is_some() {
            drop(channel);
            self.server_data.update_open_channel_ids().await;
        } else {
            self.socket_sender
                .send_sender_error(
                    sender_id,
                    ServerSenderErrorMessage::SessionSenderIdIsNotExist,
                )
                .await;
        }
    }

    pub async fn exit_channel(&mut self, receiver_id: SessionReceiverId) {
        let receiver = self.channel_receivers.remove(&receiver_id);
        // TODO: reopen channel for join: set receiver from Some(Weak(null)) to None
        // TODO: or close channel when receiver disconnected
        if receiver.is_none() {
            self.socket_sender
                .send_receiver_error(
                    receiver_id,
                    ServerReceiverErrorMessage::SessionReceiverIdIsNotExist,
                )
                .await;
        }
    }

    pub async fn send_offer(&mut self, sender_id: SessionSenderId, sdp: SessionDescription) {
        use crate::ChannelKind;

        let channel = match self.get_channel(sender_id).await {
            Some(channel) => channel,
            None => return,
        };

        let mut var = channel.sender.session_description.write().await;
        let _: Option<_> = var.replace(sdp.clone());
        drop(var);

        match &channel.kind {
            ChannelKind::PeerToPeer { receiver } => {
                let receiver = receiver.read().await;
                let receiver = receiver.as_ref().and_then(|receiver| receiver.upgrade());
                if let Some(receiver) = receiver {
                    receiver.send_offer(sdp).await;
                }
            }
            ChannelKind::ClientServer { .. } => {
                log::error!("not implemented"); // TODO
            }
        }
    }

    pub async fn send_answer(&mut self, receiver_id: SessionReceiverId, sdp: SessionDescription) {
        use crate::ChannelKind;

        let receiver = match self.get_receiver(receiver_id).await {
            Some(receiver) => receiver,
            None => return,
        };

        let mut var = receiver.session_description.write().await;
        let _: Option<_> = var.replace(sdp.clone());
        drop(var);

        let channel = match receiver.channel.upgrade() {
            Some(channel) => channel,
            None => return,
        };

        match &channel.kind {
            ChannelKind::PeerToPeer { .. } => {
                channel.sender.send_answer(sdp).await;
            }
            ChannelKind::ClientServer { .. } => {
                log::error!("not implemented"); // TODO
            }
        }
    }

    pub async fn sender_ice_candidate(
        &mut self,
        sender_id: SessionSenderId,
        ice_candidate: IceCandidate,
    ) {
        use crate::ChannelKind;

        let channel = match self.get_channel(sender_id).await {
            Some(channel) => channel,
            None => return,
        };

        let mut var = channel.sender.ice_candidates.write().await;
        var.candidates.push(ice_candidate.clone());
        var.all_sent = false;
        drop(var);

        match &channel.kind {
            ChannelKind::PeerToPeer { receiver } => {
                let receiver = receiver.read().await;
                let receiver = receiver.as_ref().and_then(|receiver| receiver.upgrade());
                if let Some(receiver) = receiver {
                    receiver.send_ice_candidate(ice_candidate).await;
                }
            }
            ChannelKind::ClientServer { .. } => {
                log::error!("not implemented"); // TODO
            }
        }
    }

    pub async fn receiver_ice_candidate(
        &mut self,
        receiver_id: SessionReceiverId,
        ice_candidate: IceCandidate,
    ) {
        use crate::ChannelKind;

        let receiver = match self.get_receiver(receiver_id).await {
            Some(receiver) => receiver,
            None => return,
        };

        let mut var = receiver.ice_candidates.write().await;
        var.candidates.push(ice_candidate.clone());
        var.all_sent = false;
        drop(var);

        let channel = match receiver.channel.upgrade() {
            Some(channel) => channel,
            None => return,
        };

        match &channel.kind {
            ChannelKind::PeerToPeer { .. } => {
                channel.sender.send_ice_candidate(ice_candidate).await;
            }
            ChannelKind::ClientServer { .. } => {
                log::error!("not implemented"); // TODO
            }
        }
    }

    pub async fn sender_all_ice_candidate_sent(&mut self, sender_id: SessionSenderId) {
        use crate::ChannelKind;

        let channel = match self.get_channel(sender_id).await {
            Some(channel) => channel,
            None => return,
        };

        let mut var = channel.sender.ice_candidates.write().await;
        var.all_sent = true;
        drop(var);

        match &channel.kind {
            ChannelKind::PeerToPeer { receiver } => {
                let receiver = receiver.read().await;
                let receiver = receiver.as_ref().and_then(|receiver| receiver.upgrade());
                if let Some(receiver) = receiver {
                    receiver.send_all_ice_candidate_sent().await;
                }
            }
            ChannelKind::ClientServer { .. } => {
                log::error!("not implemented"); // TODO
            }
        }
    }

    pub async fn receiver_all_ice_candidate_sent(&mut self, receiver_id: SessionReceiverId) {
        use crate::ChannelKind;

        let receiver = match self.get_receiver(receiver_id).await {
            Some(receiver) => receiver,
            None => return,
        };

        let mut var = receiver.ice_candidates.write().await;
        var.all_sent = true;
        drop(var);

        let channel = match receiver.channel.upgrade() {
            Some(channel) => channel,
            None => return,
        };

        match &channel.kind {
            ChannelKind::PeerToPeer { .. } => {
                channel.sender.send_all_ice_candidate_sent().await;
            }
            ChannelKind::ClientServer { .. } => {
                log::error!("not implemented"); // TODO
            }
        }
    }

    pub async fn send_binary_data(&mut self, sender_id: SessionSenderId, data: Vec<u8>) {
        use crate::ChannelKind;

        let channel = match self.get_channel(sender_id).await {
            Some(channel) => channel,
            None => return,
        };

        match &channel.kind {
            ChannelKind::PeerToPeer { receiver } => {
                let receiver = receiver.read().await;
                let receiver = receiver.as_ref().and_then(|receiver| receiver.upgrade());
                if let Some(receiver) = receiver {
                    receiver.send_binary_data(data).await;
                }
            }
            ChannelKind::ClientServer { .. } => {
                log::error!("not implemented"); // TODO
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum NewSessionError {}

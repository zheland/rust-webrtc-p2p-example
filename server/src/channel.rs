use std::sync::Weak;

use signaling_protocol::{
    ChannelId, IceCandidate, ServerReceiverMessage, ServerSenderMessage, SessionDescription,
    SessionReceiverId, SessionSenderId,
};
use tokio::sync::RwLock;

use crate::SocketSender;

#[derive(Debug)]
pub struct Channel {
    pub channel_id: Weak<ChannelId>,
    pub sender: ChannelSender,
    pub kind: ChannelKind,
}

#[allow(dead_code)] // TODO: ClientServer implementation
#[derive(Debug)]
pub enum ChannelKind {
    PeerToPeer {
        receiver: RwLock<Option<Weak<ChannelReceiver>>>,
    },
    ClientServer {
        receivers: RwLock<Vec<Weak<ChannelReceiver>>>,
    },
}

#[derive(Debug)]
pub struct ChannelSender {
    pub socket_sender: Weak<SocketSender>,
    pub session_sender_id: SessionSenderId,
    pub session_description: RwLock<Option<SessionDescription>>,
    pub ice_candidates: RwLock<ChannelIceCandidates>,
}

#[derive(Debug)]
pub struct ChannelReceiver {
    pub channel: Weak<Channel>,
    pub socket_sender: Weak<SocketSender>,
    pub session_receiver_id: SessionReceiverId,
    pub session_description: RwLock<Option<SessionDescription>>,
    pub ice_candidates: RwLock<ChannelIceCandidates>,
}

#[derive(Debug)]
pub struct ChannelIceCandidates {
    pub candidates: Vec<IceCandidate>,
    pub all_sent: bool,
}

impl ChannelSender {
    pub async fn send_answer(&self, sdp: SessionDescription) {
        if let Some(socket_sender) = self.socket_sender.upgrade() {
            socket_sender
                .send_sender_message(
                    self.session_sender_id,
                    ServerSenderMessage::ChannelAnswer(sdp),
                )
                .await;
        }
    }

    pub async fn send_ice_candidate(&self, ice: IceCandidate) {
        if let Some(socket_sender) = self.socket_sender.upgrade() {
            socket_sender
                .send_sender_message(
                    self.session_sender_id,
                    ServerSenderMessage::IceCandidate(ice),
                )
                .await;
        }
    }

    pub async fn send_all_ice_candidate_sent(&self) {
        if let Some(socket_sender) = self.socket_sender.upgrade() {
            socket_sender
                .send_sender_message(
                    self.session_sender_id,
                    ServerSenderMessage::AllIceCandidatesSent,
                )
                .await;
        }
    }
}

impl ChannelReceiver {
    pub async fn send_offer(&self, sdp: SessionDescription) {
        if let Some(socket_sender) = self.socket_sender.upgrade() {
            socket_sender
                .send_receiver_message(
                    self.session_receiver_id,
                    ServerReceiverMessage::ChannelOffer(sdp),
                )
                .await;
        }
    }

    pub async fn send_ice_candidate(&self, ice: IceCandidate) {
        if let Some(socket_sender) = self.socket_sender.upgrade() {
            socket_sender
                .send_receiver_message(
                    self.session_receiver_id,
                    ServerReceiverMessage::IceCandidate(ice),
                )
                .await;
        }
    }

    pub async fn send_all_ice_candidate_sent(&self) {
        if let Some(socket_sender) = self.socket_sender.upgrade() {
            socket_sender
                .send_receiver_message(
                    self.session_receiver_id,
                    ServerReceiverMessage::AllIceCandidatesSent,
                )
                .await;
        }
    }

    pub async fn send_offer_and_ice_candidates(
        &self,
        sdp: Option<&SessionDescription>,
        ice_candidates: &ChannelIceCandidates,
    ) {
        if let Some(socket_sender) = self.socket_sender.upgrade() {
            if let Some(sdp) = sdp {
                socket_sender
                    .send_receiver_message(
                        self.session_receiver_id,
                        ServerReceiverMessage::ChannelOffer(sdp.clone()),
                    )
                    .await;
            }
            for ice in &ice_candidates.candidates {
                socket_sender
                    .send_receiver_message(
                        self.session_receiver_id,
                        ServerReceiverMessage::IceCandidate(ice.clone()),
                    )
                    .await;
            }
            if ice_candidates.all_sent {
                socket_sender
                    .send_receiver_message(
                        self.session_receiver_id,
                        ServerReceiverMessage::AllIceCandidatesSent,
                    )
                    .await;
            }
        }
    }

    pub async fn send_binary_data(&self, data: Vec<u8>) {
        if let Some(socket_sender) = self.socket_sender.upgrade() {
            socket_sender
                .send_receiver_message(
                    self.session_receiver_id,
                    ServerReceiverMessage::BinaryData(data),
                )
                .await;
        }
    }
}

impl ChannelIceCandidates {
    pub fn new() -> Self {
        Self {
            candidates: Vec::new(),
            all_sent: false,
        }
    }
}

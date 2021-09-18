#![warn(
    clippy::all,
    rust_2018_idioms,
    missing_copy_implementations,
    missing_debug_implementations,
    single_use_lifetimes,
    trivial_casts,
    unused_import_braces,
    unused_qualifications,
    unused_results
)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SessionSenderId(pub u32);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SessionReceiverId(pub u32);

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ChannelId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SessionDescription(pub String);

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_m_line_index: Option<u16>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum NetworkMode {
    PeerToPeer,
    ClientServer,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ClientMessage {
    SenderMessage {
        sender_id: SessionSenderId,
        message: ClientSenderMessage,
    },
    ReceiverMessage {
        receiver_id: SessionReceiverId,
        message: ClientReceiverMessage,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ClientSenderMessage {
    OpenChannel {
        channel_id: ChannelId,
        network_mode: NetworkMode,
    },
    CloseChannel,
    SendOffer(SessionDescription),
    IceCandidate(IceCandidate),
    AllIceCandidatesSent,
    SendBinaryData(Vec<u8>),
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ClientReceiverMessage {
    JoinChannel { channel_id: ChannelId },
    ExitChannel,
    SendAnswer(SessionDescription),
    IceCandidate(IceCandidate),
    AllIceCandidatesSent,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ServerMessage {
    OpenChannelIdsChanged(Vec<ChannelId>),
    SenderMessage {
        sender_id: SessionSenderId,
        message: ServerSenderMessage,
    },
    ReceiverMessage {
        receiver_id: SessionReceiverId,
        message: ServerReceiverMessage,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ServerSenderMessage {
    OpenChannelSuccess,
    ChannelAnswer(SessionDescription),
    IceCandidate(IceCandidate),
    AllIceCandidatesSent,
    Error(ServerSenderErrorMessage),
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ServerReceiverMessage {
    JoinChannelSuccess,
    ChannelOffer(SessionDescription),
    IceCandidate(IceCandidate),
    AllIceCandidatesSent,
    BinaryData(Vec<u8>),
    Error(ServerReceiverErrorMessage),
}

#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, Deserialize, Eq, Error, Hash, PartialEq, Serialize)]
pub enum ServerSenderErrorMessage {
    #[error("session sender id `{}` is already used", 0.0)]
    SessionSenderIdIsAlreadyUsed,
    #[error("session sender id `{}` is not exist", 0.0)]
    SessionSenderIdIsNotExist,
    #[error("channel `{}` is already used", 0.0)]
    ChannelIdIsAlreadyUsed(ChannelId),
}

#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, Deserialize, Eq, Error, Hash, PartialEq, Serialize)]
pub enum ServerReceiverErrorMessage {
    #[error("session receiver id `{}` is already used", 0.0)]
    SessionReceiverIdIsAlreadyUsed,
    #[error("session receiver id `{}` is not exist", 0.0)]
    SessionReceiverIdIsNotExist,
    #[error("channel `{}` is not exist", 0.0)]
    ChannelIsNotExist(ChannelId),
    #[error("channel `{}` is already occupied", 0.0)]
    ChannelIsAlreadyOccupied(ChannelId),
}

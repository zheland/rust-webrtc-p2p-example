use std::collections::HashMap;
use std::sync::{Arc, Weak};

use signaling_protocol::ChannelId;
use tokio::sync::RwLock;

use crate::{Channel, SocketId, SocketSender};

#[derive(Debug)]
pub struct ServerData {
    channels: RwLock<HashMap<Arc<ChannelId>, Weak<Channel>>>,
    senders: RwLock<HashMap<SocketId, Weak<SocketSender>>>,
}

impl ServerData {
    pub fn new() -> Self {
        let channels = RwLock::new(HashMap::new());
        let senders = RwLock::new(HashMap::new());
        Self { channels, senders }
    }

    pub fn channels(&self) -> &RwLock<HashMap<Arc<ChannelId>, Weak<Channel>>> {
        &self.channels
    }

    pub fn senders(&self) -> &RwLock<HashMap<SocketId, Weak<SocketSender>>> {
        &self.senders
    }

    pub async fn remove_channels<T: AsRef<ChannelId>, I: IntoIterator<Item = T>>(&self, iter: I) {
        let mut channels = self.channels.write().await;
        for channel_id in iter.into_iter() {
            drop(channels.remove(channel_id.as_ref()));
        }
    }

    pub async fn update_open_channel_ids(&self) {
        use crate::ChannelKind;
        use signaling_protocol::ServerMessage;

        let channels = self.channels.read().await;
        let mut channel_ids = Vec::new();
        for (channel_id, channel) in channels.iter() {
            if let Some(channel) = channel.upgrade() {
                match &channel.kind {
                    ChannelKind::PeerToPeer { receiver } => {
                        if receiver.read().await.is_none() {
                            channel_ids.push(channel_id.as_ref().to_owned())
                        }
                    }
                    ChannelKind::ClientServer { .. } => {
                        channel_ids.push(channel_id.as_ref().to_owned())
                    }
                }
            }
        }
        drop(channels);

        let senders = self.senders.read().await;
        for sender in senders.values() {
            if let Some(sender) = sender.upgrade() {
                sender
                    .send(ServerMessage::OpenChannelIdsChanged(channel_ids.clone()))
                    .await;
            }
        }
    }
}

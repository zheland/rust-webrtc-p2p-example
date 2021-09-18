use async_std::sync::Arc;
use browser_webrtc::signaling_protocol::ChannelId;
use browser_webrtc::Server;
use sycamore::prelude::*;

use crate::{ReceiversListView, SendersListView};

#[derive(Debug)]
pub struct ServerView {
    server: Arc<Server>,
    channels_var: Signal<Vec<ChannelId>>,
    senders: Arc<SendersListView>,
    receivers: Arc<ReceiversListView>,
}

impl ServerView {
    pub fn new(server: Arc<Server>, channels_var: Signal<Vec<ChannelId>>) -> Arc<Self> {
        log::trace!("client::ServerView::new");

        let senders = SendersListView::new(Arc::clone(&server));
        let receivers = ReceiversListView::new(Arc::clone(&server), channels_var.clone());

        Arc::new(Self {
            server,
            channels_var,
            senders,
            receivers,
        })
    }

    pub fn view(self: &Arc<Self>) -> Template<DomNode> {
        let senders = Arc::clone(&self.senders);
        let receivers = Arc::clone(&self.receivers);

        template! {
            (senders.view())
            (receivers.view())
        }
    }
}

impl Drop for ServerView {
    fn drop(&mut self) {
        log::trace!("client::ServerView::drop");
    }
}

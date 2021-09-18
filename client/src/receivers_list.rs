use async_std::sync::{Arc, Weak};
use browser_webrtc::signaling_protocol::ChannelId;
use browser_webrtc::Server;
use core::cell::RefCell;
use sycamore::prelude::*;

use crate::ReceiverBuilderView;

#[derive(Debug)]
pub struct ReceiversListView {
    server: Weak<Server>,
    channels_var: Signal<Vec<ChannelId>>,
    receivers_var: Signal<RefCell<Vec<Arc<ReceiverBuilderView>>>>,
}

impl ReceiversListView {
    pub fn new(server: Arc<Server>, channels_var: Signal<Vec<ChannelId>>) -> Arc<Self> {
        log::trace!("client::ReceiversListView::new");

        let receivers_var = Signal::new(RefCell::new(Vec::new()));

        Arc::new(Self {
            server: Arc::downgrade(&server),
            channels_var,
            receivers_var,
        })
    }

    pub fn add_receiver(self: &Arc<Self>, channel_id: ChannelId) {
        use crate::SignalVecPush;
        let receiver =
            ReceiverBuilderView::new(Arc::clone(self), self.server.upgrade().unwrap(), channel_id);
        self.receivers_var.push(receiver);
    }

    pub fn remove_receiver(self: &Arc<Self>, receiver: &Arc<ReceiverBuilderView>) {
        use crate::SignalVecRemoveByPtrEq;
        self.receivers_var.remove_by_ptr_eq(receiver);
    }

    pub fn view(self: &Arc<Self>) -> Template<DomNode> {
        let channels_var = self.channels_var.clone();
        let receivers_var = self.receivers_var.clone();

        let on_add_receiver_click = {
            let self_arc = Arc::clone(self);
            move |channel_id: ChannelId| {
                let self_arc = Arc::clone(&self_arc);
                move |_| self_arc.add_receiver(channel_id.clone())
            }
        };

        template! {
            div(class = "component") {
                h1() {
                    ("Receivers")
                }
                div() {
                    ({
                        let channels = channels_var.get();
                        if channels.is_empty() {
                            template! {
                                div() {
                                    ("No open channels found")
                                }
                            }
                        } else {
                            let fragment = Template::new_fragment(
                                channels
                                    .iter()
                                    .cloned()
                                    .map(|channel| template! {
                                        button(
                                            on:click = on_add_receiver_click(channel.clone())
                                        ) {
                                            ("Join channel: ")
                                            (channel.0)
                                        }
                                    })
                                    .collect(),
                            );
                            fragment
                        }
                    })
                }
                div() {
                    ({
                        Template::new_fragment(
                            receivers_var
                                .get()
                                .borrow()
                                .iter()
                                .map(|receiver| receiver.view())
                                .collect(),
                        )
                    })
                }
            }
        }
    }
}

impl Drop for ReceiversListView {
    fn drop(&mut self) {
        log::trace!("client::ReceiversListView::drop");
    }
}

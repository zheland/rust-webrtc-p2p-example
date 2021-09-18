use core::cell::RefCell;

use async_std::sync::{Arc, Weak};
use browser_webrtc::signaling_protocol::{ChannelId, NetworkMode};
use browser_webrtc::Server;
use sycamore::prelude::*;

use crate::SenderBuilderView;

const DEFAULT_NETWORK_MODE: NetworkMode = NetworkMode::PeerToPeer;

#[derive(Debug)]
pub struct SendersListView {
    server: Weak<Server>,
    channel_name_var: Signal<ChannelId>,
    network_mode_var: Signal<NetworkMode>,
    should_use_video_var: Signal<bool>,
    should_use_audio_var: Signal<bool>,
    should_use_data_channel_var: Signal<bool>,
    senders_var: Signal<RefCell<Vec<Arc<SenderBuilderView>>>>,
}

impl SendersListView {
    pub fn new(server: Arc<Server>) -> Arc<Self> {
        log::trace!("client::SendersListView::new");

        let channel_name_var = Signal::new(ChannelId(Self::rand_channel_name()));
        let network_mode_var = Signal::new(DEFAULT_NETWORK_MODE);
        let senders_var = Signal::new(RefCell::new(Vec::new()));
        let should_use_video_var = Signal::new(true);
        let should_use_audio_var = Signal::new(true);
        let should_use_data_channel_var = Signal::new(true);

        Arc::new(Self {
            server: Arc::downgrade(&server),
            channel_name_var,
            network_mode_var,
            senders_var,
            should_use_video_var,
            should_use_audio_var,
            should_use_data_channel_var,
        })
    }

    pub fn rand_channel_name() -> String {
        let rand_letter = || b'a' + (js_sys::Math::random() * 26.0).floor() as u8;
        let channel_name = [rand_letter(), rand_letter(), rand_letter(), rand_letter()];
        let channel_name = std::str::from_utf8(&channel_name).unwrap();
        channel_name.to_owned()
    }

    pub fn add_sender(self: &Arc<Self>) {
        use crate::SignalVecPush;
        let sender = SenderBuilderView::new(
            Arc::clone(self),
            self.server.upgrade().unwrap(),
            self.channel_name_var.get().as_ref().clone(),
            *self.network_mode_var.get().as_ref(),
            *self.should_use_video_var.get().as_ref(),
            *self.should_use_audio_var.get().as_ref(),
            *self.should_use_data_channel_var.get().as_ref(),
        );
        self.senders_var.push(sender);
        self.channel_name_var
            .set(ChannelId(Self::rand_channel_name()));
    }

    pub fn remove_sender(self: &Arc<Self>, sender: &Arc<SenderBuilderView>) {
        use crate::SignalVecRemoveByPtrEq;
        self.senders_var.remove_by_ptr_eq(sender);
    }

    pub fn view(self: &Arc<Self>) -> Template<DomNode> {
        use wasm_bindgen::JsCast;
        use web_sys::{Event, HtmlInputElement};

        let on_add_sender_click = {
            let self_arc = Arc::clone(self);
            move |_| self_arc.add_sender()
        };

        let on_channel_name_change = {
            let self_arc = Arc::clone(self);
            move |ev: Event| {
                let target: HtmlInputElement = ev.target().unwrap().dyn_into().unwrap();
                self_arc.channel_name_var.set(ChannelId(target.value()))
            }
        };

        let self_arc = Arc::clone(self);
        let channel_name_var = self.channel_name_var.clone();
        let network_mode_var = self.network_mode_var.clone();
        let should_use_video_var = self.should_use_video_var.clone();
        let should_use_audio_var = self.should_use_audio_var.clone();
        let should_use_data_channel_var = self.should_use_data_channel_var.clone();
        let senders_var = self.senders_var.clone();

        template! {
            div(class = "component") {
                h1() {
                    ("Senders")
                }
                div() {
                    label() {
                        ("channel name: ")
                        input(
                            type = "text",
                            value = (channel_name_var.get().0),
                            on:input = on_channel_name_change,
                        )
                    }
                }
                div() {
                    ({
                        let network_mode = network_mode_var.get().as_ref().clone();

                        let on_set_network_mode_peer_to_peer = {
                            let self_arc = Arc::clone(&self_arc);
                            move |_| self_arc.network_mode_var.set(NetworkMode::PeerToPeer)
                        };

                        let on_set_network_mode_client_server = {
                            let self_arc = Arc::clone(&self_arc);
                            move |_| self_arc.network_mode_var.set(NetworkMode::ClientServer)
                        };


                        template! {
                            label() {
                                input(
                                    type = "checkbox",
                                    checked = network_mode == NetworkMode::PeerToPeer,
                                    on:change = on_set_network_mode_peer_to_peer,
                                )
                                ("PeerToPeer")
                            }
                            label() {
                                input(
                                    type = "checkbox",
                                    checked = network_mode == NetworkMode::ClientServer,
                                    on: change = on_set_network_mode_client_server,
                                )
                                ("ClientServer")
                            }
                        }
                    })
                }
                div() {
                    label() {
                        input(
                            type = "checkbox",
                            bind:checked = should_use_video_var
                        )
                        ("Use Video")
                    }
                    label() {
                        input(
                            type = "checkbox",
                            bind:checked = should_use_audio_var
                        )
                        ("Use Audio")
                    }
                    label() {
                        input(
                            type = "checkbox",
                            bind:checked = should_use_data_channel_var
                        )
                        ("Use DataChannel")
                    }
                }
                button(on:click = on_add_sender_click) {
                    ("Open channel")
                }
                div() {
                    ({
                        Template::new_fragment(
                            senders_var
                                .get()
                                .borrow()
                                .iter()
                                .map(|sender| sender.view())
                                .collect(),
                        )
                    })
                }
            }
        }
    }
}

impl Drop for SendersListView {
    fn drop(&mut self) {
        log::trace!("client::SendersListView::drop");
    }
}

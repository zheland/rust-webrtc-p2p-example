use async_std::sync::{Arc, Weak};
use browser_webrtc::signaling_protocol::ChannelId;
use browser_webrtc::{ReceiverEvent, Server};
use sycamore::prelude::*;

use crate::{ReceiverView, ReceiversListView};

#[derive(Debug)]
pub struct ReceiverBuilderView {
    receivers: Arc<ReceiversListView>,
    server: Weak<Server>,
    receiver_var: Signal<Option<Result<Arc<ReceiverView>, anyhow::Error>>>,
    ice_connection_state_var: Signal<String>,
    ice_gathering_state_var: Signal<String>,
    signaling_state_var: Signal<String>,
    channel_id: ChannelId,
}

impl ReceiverBuilderView {
    pub fn new(
        receivers: Arc<ReceiversListView>,
        server: Arc<Server>,
        channel_id: ChannelId,
    ) -> Arc<Self> {
        use wasm_bindgen_futures::spawn_local;

        log::trace!("client::ReceiverBuilderView::new");

        let receiver_var = Signal::new(None);
        let ice_connection_state_var = Signal::new(String::new());
        let ice_gathering_state_var = Signal::new(String::new());
        let signaling_state_var = Signal::new(String::new());

        let receiver = Arc::new(Self {
            receivers,
            server: Arc::downgrade(&server),
            receiver_var: receiver_var.clone(),
            ice_connection_state_var,
            ice_gathering_state_var,
            signaling_state_var,
            channel_id,
        });

        spawn_local({
            let receiver = Arc::clone(&receiver);
            async move { receiver_var.set(Some(receiver.init().await)) }
        });

        receiver
    }

    async fn init(self: Arc<Self>) -> Result<Arc<ReceiverView>, anyhow::Error> {
        use browser_webrtc::{default_rtc_configuration, RtcConfigurationExt};
        use log::error;

        let self_weak = Arc::downgrade(&self);
        let rtc_configuration = default_rtc_configuration().with_google_stun_server();
        let receiver = self
            .server
            .upgrade()
            .unwrap()
            .join_channel(
                self.channel_id.clone(),
                Some(rtc_configuration),
                Box::new(move |_, ev| {
                    let self_weak = Weak::clone(&self_weak);
                    Box::pin(async move {
                        let self_arc = self_weak.upgrade().unwrap();
                        self_arc.on_event(ev).await
                    })
                }),
            )
            .await;

        let receiver = match receiver {
            Ok(receiver) => receiver,
            Err(err) => {
                error!("{}", err);
                return Err(anyhow::Error::msg(err.to_string()));
            }
        };

        self.ice_connection_state_var
            .set(format!("{:?}", receiver.ice_connection_state()));
        self.ice_gathering_state_var
            .set(format!("{:?}", receiver.ice_gathering_state()));
        self.signaling_state_var
            .set(format!("{:?}", receiver.signaling_state()));

        let receiver_view = ReceiverView::new(receiver);

        Ok(receiver_view)
    }

    fn receiver<'a>(self: &Arc<Self>) -> Option<Arc<ReceiverView>> {
        self.receiver_var
            .get()
            .as_ref()
            .as_ref()
            .and_then(|receiver| receiver.as_ref().ok())
            .cloned()
    }

    async fn on_event(self: &Arc<Self>, ev: ReceiverEvent) {
        use log::{debug, error};
        match ev {
            ReceiverEvent::IceConnectionStateChange(value) => {
                self.ice_connection_state_var.set(format!("{:?}", value))
            }
            ReceiverEvent::IceGatheringStateChange(value) => {
                self.ice_gathering_state_var.set(format!("{:?}", value))
            }
            ReceiverEvent::RtcSignalingStateChange(value) => {
                self.signaling_state_var.set(format!("{:?}", value))
            }
            ReceiverEvent::MediaReceiver(media_receiver_builder) => {
                if let Some(receiver) = self.receiver() {
                    receiver.on_media_receiver(media_receiver_builder).await;
                }
            }
            ReceiverEvent::DataReceiver(data_receiver_buidler) => {
                if let Some(receiver) = self.receiver() {
                    receiver.on_data_receiver(data_receiver_buidler).await;
                }
            }
            ReceiverEvent::BinaryData(data) => {
                if let Some(receiver) = self.receiver() {
                    receiver.on_socket_binary_data(data).await;
                }
            }
            ReceiverEvent::Error(err) => error!("{}", err),
            ev => debug!("Receiver event {:?}", ev),
        }
    }

    pub fn view(self: &Arc<Self>) -> Template<DomNode> {
        let receiver_var = self.receiver_var.clone();
        let ice_connection_state_var = self.ice_connection_state_var.clone();
        let ice_gathering_state_var = self.ice_gathering_state_var.clone();
        let signaling_state_var = self.signaling_state_var.clone();

        let channel_id = self.channel_id.clone();

        let on_close_click = {
            let self_weak = Arc::downgrade(self);
            move |_| {
                let self_arc = self_weak.upgrade().unwrap();
                self_arc.receivers.remove_receiver(&self_arc)
            }
        };

        template! {
            div(class = "component") {
                h1() {
                    ("Receiver")
                }
                button(on:click = on_close_click, class = "close") {
                    ("close")
                }
                div(class = "monospace") {
                    ("channel id: ")
                    (channel_id.0)
                }
                div(class = "monospace") {
                    ("ice_connection_state: ")
                    (ice_connection_state_var.get())
                }
                div(class = "monospace") {
                    ("ice_gathering_state: ")
                    (ice_gathering_state_var.get())
                }
                div(class = "monospace") {
                    ("signaling_state: ")
                    (signaling_state_var.get())
                }
                ({
                    let receiver = receiver_var.get();

                    match receiver.as_ref() {
                        Some(Ok(receiver)) => {
                            receiver.view()
                        },
                        Some(Err(err)) => {
                            let err = err.to_string();
                            template! {
                                h2() {
                                    ("error")
                                }
                                textarea(class = "error", readonly = true) {
                                    (err)
                                }
                            }
                        },
                        None => {
                            template! {
                                h2(class = "loading") {
                                    ("loading...")
                                }
                            }
                        }
                    }
                })
            }
        }
    }
}

impl Drop for ReceiverBuilderView {
    fn drop(&mut self) {
        log::debug!("drop ReceiverBuilderView");
    }
}

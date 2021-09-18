use async_std::sync::{Arc, Weak};
use browser_webrtc::signaling_protocol::{ChannelId, NetworkMode};
use browser_webrtc::{DataSenderEvent, LocalMedia, MediaView, MediaViewAudio, SenderEvent, Server};
use sycamore::prelude::*;

use crate::{SenderView, SendersListView};

const DEFAULT_DATA_CHANNEL_NAME: &'static str = "default";

#[derive(Debug)]
pub struct SenderBuilderView {
    senders: Arc<SendersListView>,
    server: Weak<Server>,
    sender_var: Signal<Option<Result<Arc<SenderView>, anyhow::Error>>>,
    ice_connection_state_var: Signal<String>,
    ice_gathering_state_var: Signal<String>,
    signaling_state_var: Signal<String>,
    channel_id: ChannelId,
    network_mode: NetworkMode,
    should_use_video: bool,
    should_use_audio: bool,
    should_use_data_channel: bool,
}

impl SenderBuilderView {
    pub fn new(
        senders: Arc<SendersListView>,
        server: Arc<Server>,
        channel_id: ChannelId,
        network_mode: NetworkMode,
        should_use_video: bool,
        should_use_audio: bool,
        should_use_data_channel: bool,
    ) -> Arc<Self> {
        use wasm_bindgen_futures::spawn_local;

        log::trace!("client::SenderView::new");

        let sender_var = Signal::new(None);
        let ice_connection_state_var = Signal::new(String::new());
        let ice_gathering_state_var = Signal::new(String::new());
        let signaling_state_var = Signal::new(String::new());

        let sender = Arc::new(Self {
            senders,
            server: Arc::downgrade(&server),
            sender_var: sender_var.clone(),
            ice_connection_state_var,
            ice_gathering_state_var,
            signaling_state_var,
            channel_id: channel_id.clone(),
            network_mode,
            should_use_video,
            should_use_audio,
            should_use_data_channel,
        });

        spawn_local({
            let sender = Arc::clone(&sender);
            async move { sender_var.set(Some(sender.init().await)) }
        });

        sender
    }

    async fn init(self: Arc<Self>) -> Result<Arc<SenderView>, anyhow::Error> {
        use browser_webrtc::{default_rtc_configuration, RtcConfigurationExt};
        use log::error;

        let self_weak = Arc::downgrade(&self);
        let rtc_configuration = default_rtc_configuration().with_google_stun_server();
        let sender = self
            .server
            .upgrade()
            .unwrap()
            .open_channel(
                self.channel_id.clone(),
                self.network_mode,
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

        let sender = match sender {
            Ok(sender) => sender,
            Err(err) => {
                error!("{}", err);
                return Err(anyhow::Error::msg(err.to_string()));
            }
        };

        let media = match (self.should_use_video, self.should_use_audio) {
            (true, true) => Some(LocalMedia::with_video_and_audio().await),
            (true, false) => Some(LocalMedia::with_video().await),
            (false, true) => Some(LocalMedia::with_audio().await),
            (false, false) => None,
        };

        let media_stream = media.as_ref().map(|media| media.media_stream());
        let media_sender =
            media_stream.map(|media_stream| sender.add_media_stream(media_stream.clone()));
        let media_view = media_stream
            .map(|media_stream| {
                MediaView::new(media_stream.clone(), MediaViewAudio::Disable)
                    .map_err(|err| anyhow::Error::msg(err.to_string()))
            })
            .transpose()?;

        let self_weak = Arc::downgrade(&self);
        let data_sender = if self.should_use_data_channel {
            Some(sender.add_data_channel(
                DEFAULT_DATA_CHANNEL_NAME,
                Box::new(move |_, ev| {
                    let self_weak = Weak::clone(&self_weak);
                    Box::pin(async move {
                        let self_arc = self_weak.upgrade().unwrap();
                        self_arc.on_datachannel_event(ev).await
                    })
                }),
            ))
        } else {
            None
        };

        match sender.start().await {
            Ok(()) => {}
            Err(err) => {
                error!("{}", err);
                return Err(anyhow::Error::msg(err.to_string()));
            }
        };

        self.ice_connection_state_var
            .set(format!("{:?}", sender.ice_connection_state()));
        self.ice_gathering_state_var
            .set(format!("{:?}", sender.ice_gathering_state()));
        self.signaling_state_var
            .set(format!("{:?}", sender.signaling_state()));

        let sender_view = SenderView::new(sender, media_sender, media_view, data_sender);

        Ok(sender_view)
    }

    async fn on_event(self: &Arc<Self>, ev: SenderEvent) {
        use log::{debug, error};
        match ev {
            SenderEvent::Error(err) => error!("{}", err),
            SenderEvent::IceConnectionStateChange(value) => {
                self.ice_connection_state_var.set(format!("{:?}", value))
            }
            SenderEvent::IceGatheringStateChange(value) => {
                self.ice_gathering_state_var.set(format!("{:?}", value))
            }
            SenderEvent::RtcSignalingStateChange(value) => {
                self.signaling_state_var.set(format!("{:?}", value))
            }
            ev => debug!("Sender event {:?}", ev),
        }
    }

    pub async fn on_datachannel_event(self: &Arc<Self>, ev: DataSenderEvent) {
        use log::{debug, error};
        match ev {
            DataSenderEvent::Error(err) => error!("{}", err),
            ev => debug!("Sender event {:?}", ev),
        }
    }

    pub fn view(self: &Arc<Self>) -> Template<DomNode> {
        let sender_var = self.sender_var.clone();
        let ice_connection_state_var = self.ice_connection_state_var.clone();
        let ice_gathering_state_var = self.ice_gathering_state_var.clone();
        let signaling_state_var = self.signaling_state_var.clone();

        let channel_id = self.channel_id.clone();
        let network_mode = self.network_mode;
        let should_use_video = self.should_use_video;
        let should_use_audio = self.should_use_audio;
        let should_use_data_channel = self.should_use_data_channel;

        let on_close_click = {
            let self_arc = Arc::clone(self);
            move |_| self_arc.senders.remove_sender(&self_arc)
        };

        template! {
            div(class = "component") {
                h1() {
                    ("Sender")
                }
                button(on:click = on_close_click, class = "close") {
                    ("close")
                }
                div(class = "monospace") {
                    ("channel id: ")
                    (channel_id.0)
                }
                div(class = "monospace") {
                    ("network mode: ")
                    (format!("{:?}", network_mode))
                }
                div(class = "monospace") {
                    ("video: ")
                    (if should_use_video { "yes" } else { "no" })
                }
                div(class = "monospace") {
                    ("audio: ")
                    (if should_use_audio { "yes" } else { "no" })
                }
                div(class = "monospace") {
                    ("channel-data: ")
                    (if should_use_data_channel { "yes" } else { "no" })
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
                    let sender = sender_var.get();

                    match sender.as_ref() {
                        Some(Ok(sender)) => {
                            sender.view()
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

impl Drop for SenderBuilderView {
    fn drop(&mut self) {
        log::debug!("client::SenderBuilderView::drop");
    }
}

use core::cell::RefCell;

use async_std::sync::{Arc, Weak};
use browser_webrtc::{
    DataReceiver, DataReceiverBuilder, DataReceiverEvent, MediaReceiver, MediaReceiverBuilder,
    MediaReceiverEvent, MediaView, MediaViewAudio, Receiver,
};
use sycamore::prelude::*;

#[derive(Debug)]
pub struct ReceiverView {
    receiver: Arc<Receiver>,
    media_receivers_var: Signal<RefCell<Vec<Arc<MediaReceiver>>>>,
    media_views_var: Signal<RefCell<Vec<Arc<MediaView>>>>,
    data_receivers_var: Signal<RefCell<Vec<Arc<DataReceiver>>>>,
    web_binary_data_var: Signal<String>,
    socket_binary_data_var: Signal<String>,
}

impl ReceiverView {
    pub fn new(receiver: Arc<Receiver>) -> Arc<Self> {
        log::trace!("client::ReceiverView::new");

        let media_receivers_var = Signal::new(RefCell::new(Vec::new()));
        let media_views_var = Signal::new(RefCell::new(Vec::new()));
        let data_receivers_var = Signal::new(RefCell::new(Vec::new()));
        let web_binary_data_var = Signal::new(String::new());
        let socket_binary_data_var = Signal::new(String::new());

        Arc::new(Self {
            receiver,
            media_receivers_var,
            media_views_var,
            data_receivers_var,
            web_binary_data_var,
            socket_binary_data_var,
        })
    }

    pub async fn on_media_receiver(self: &Arc<Self>, builder: MediaReceiverBuilder) {
        log::trace!("client::Receiver::add_media_receiver");

        use crate::SignalVecPush;
        use log::error;

        let self_weak = Arc::downgrade(&self);

        let media_receiver = builder.build_with_handler(Box::new(move |_, ev| {
            let self_weak = Weak::clone(&self_weak);
            Box::pin(async move {
                let self_arc = self_weak.upgrade().unwrap();
                self_arc.on_media_receiver_event(ev).await
            })
        }));

        let media_view = MediaView::new(
            media_receiver.media_stream().clone(),
            MediaViewAudio::Enable,
        );

        self.media_receivers_var.push(media_receiver);

        match media_view {
            Ok(media_view) => self.media_views_var.push(media_view),
            Err(err) => error!("{}", err),
        }
    }

    pub async fn on_data_receiver(self: &Arc<Self>, builder: DataReceiverBuilder) {
        log::trace!("client::Receiver::add_data_receiver");

        use crate::SignalVecPush;

        let self_weak = Arc::downgrade(&self);

        let data_receiver = builder.build_with_handler(Box::new(move |_, ev| {
            let self_weak = Weak::clone(&self_weak);
            Box::pin(async move {
                let self_arc = self_weak.upgrade().unwrap();
                self_arc.on_data_receiver_event(ev).await
            })
        }));

        self.data_receivers_var.push(data_receiver);
    }

    pub async fn on_socket_binary_data(self: &Arc<Self>, data: Vec<u8>) {
        self.socket_binary_data_var
            .set(String::from_utf8_lossy(&data).to_string());
    }

    pub async fn on_media_receiver_event(self: &Arc<Self>, ev: MediaReceiverEvent) {
        use log::{debug, error};
        match ev {
            MediaReceiverEvent::Error(err) => error!("{}", err),
            ev => debug!("{:?}", ev),
        }
    }

    pub async fn on_data_receiver_event(self: &Arc<Self>, ev: DataReceiverEvent) {
        use log::{debug, error};
        match ev {
            DataReceiverEvent::Error(err) => error!("{}", err),
            ev => debug!("{:?}", ev),
        }
    }

    pub fn view(self: &Arc<Self>) -> Template<DomNode> {
        let media_views_var = self.media_views_var.clone();
        let web_binary_data_var = self.web_binary_data_var.clone();
        let socket_binary_data_var = self.socket_binary_data_var.clone();

        template! {
            div() {
                ({
                    Template::new_fragment(
                        media_views_var
                            .get()
                            .borrow()
                            .iter()
                            .map(|media_view| {
                                let node_ref = NodeRef::new();
                                let template = template! {
                                    div(class = "video", ref = node_ref) {}
                                };
                                let node: DomNode = node_ref.get();
                                let node = node.inner_element();
                                let _: Option<_> = node.append_child(media_view.view()).ok();
                                template
                            })
                            .collect(),
                    )
                })
            }
            div() {
                label() {
                    div() {
                        ("WebRtc DataChannel")
                    }
                    textarea(readonly = true) {
                        (web_binary_data_var.get())
                    }
                }
            }
            div() {
                label() {
                    div() {
                        ("WebSocket DataChannel")
                    }
                    textarea(readonly = true) {
                        (socket_binary_data_var.get())
                    }
                }
            }
        }
    }
}

impl Drop for ReceiverView {
    fn drop(&mut self) {
        log::trace!("client::ReceiverView::drop");
    }
}

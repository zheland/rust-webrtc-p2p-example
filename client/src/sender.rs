use async_std::sync::Arc;
use browser_webrtc::{DataSender, MediaSender, MediaView, Sender};
use sycamore::prelude::*;

#[derive(Debug)]
pub struct SenderView {
    sender: Arc<Sender>,
    media_sender: Option<Arc<MediaSender>>,
    media_view: Option<Arc<MediaView>>,
    data_sender: Option<Arc<DataSender>>,
}

impl SenderView {
    pub fn new(
        sender: Arc<Sender>,
        media_sender: Option<Arc<MediaSender>>,
        media_view: Option<Arc<MediaView>>,
        data_sender: Option<Arc<DataSender>>,
    ) -> Arc<Self> {
        log::trace!("client::SenderView::new");

        Arc::new(Self {
            sender,
            media_sender,
            media_view,
            data_sender,
        })
    }

    pub fn view(self: &Arc<Self>) -> Template<DomNode> {
        use wasm_bindgen::JsCast;
        use web_sys::{Event, HtmlTextAreaElement};

        let media_view = self.media_view.clone();
        let node_ref = NodeRef::new();
        let data_sender = self.data_sender.clone();

        let on_websocket_data_input = {
            let self_arc = Arc::clone(self);
            move |ev: Event| {
                let target: HtmlTextAreaElement = ev.target().unwrap().dyn_into().unwrap();
                let _ = self_arc
                    .sender
                    .send_binary_data(target.value().as_bytes().to_vec());
            }
        };

        template! {
            ({
                if let Some(media_view) = media_view.as_ref() {
                    let template = template! {
                        div(class = "video", ref = node_ref) {}
                    };
                    let node: DomNode = node_ref.get();
                    let node = node.inner_element();
                    let _: Option<_> = node.append_child(media_view.view()).ok();
                    template
                } else {
                    template! {}
                }
            })
            ({
                match data_sender.as_ref() {
                    Some(data_sender) => {
                        let on_webrtc_data_input = {
                            let data_sender = data_sender.clone();
                            move |ev: Event| {
                                let target: HtmlTextAreaElement = ev.target().unwrap().dyn_into().unwrap();
                                let _ = data_sender.send(target.value().as_bytes());
                            }
                        };

                        template! {
                            div() {
                                label() {
                                    div() {
                                        ("WebRtc DataChannel")
                                    }
                                    textarea(
                                        on:input = on_webrtc_data_input,
                                    ) {}
                                }
                            }
                        }
                    },
                    None => template! {},
                }
            })
            div() {
                label() {
                    div() {
                        ("WebSocket DataChannel")
                    }
                    textarea(
                        on:input = on_websocket_data_input,
                    ) {}
                }
            }
        }
    }
}

impl Drop for SenderView {
    fn drop(&mut self) {
        log::trace!("client::SenderView::drop");
    }
}

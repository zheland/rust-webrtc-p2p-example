use async_std::sync::{Arc, Weak};
use browser_webrtc::signaling_protocol::ChannelId;
use browser_webrtc::{NewServerError, ServerEvent};
use sycamore::prelude::*;

use crate::{ServerView, ServersListView};

#[derive(Debug)]
pub struct ServerBuilderView {
    servers: Arc<ServersListView>,
    addr: String,
    server_var: Signal<Option<Result<Arc<ServerView>, NewServerError>>>,
    channels_var: Signal<Vec<ChannelId>>,
}

impl ServerBuilderView {
    pub fn new(servers: Arc<ServersListView>, addr: String) -> Arc<Self> {
        use wasm_bindgen_futures::spawn_local;

        log::trace!("client::ServerBuilderView::new");

        let addr = if addr.starts_with("ws://") || addr.starts_with("wss://") {
            addr.to_owned()
        } else {
            format!("ws://{}", addr)
        };

        let server_var = Signal::new(None);
        let channels_var = Signal::new(Vec::new());

        let server = Arc::new(Self {
            servers,
            addr: addr.clone(),
            server_var: server_var.clone(),
            channels_var: channels_var.clone(),
        });

        spawn_local({
            let server = Arc::clone(&server);
            async move { server_var.set(Some(server.init().await)) }
        });

        server
    }

    async fn init(self: Arc<Self>) -> Result<Arc<ServerView>, NewServerError> {
        use browser_webrtc::Server;
        use log::error;

        let addr = self.addr.to_owned();
        let channels_var = self.channels_var.clone();

        let self_weak = Arc::downgrade(&self);
        let server = {
            Server::new(
                addr,
                Box::new(move |_, ev| {
                    let self_weak = Weak::clone(&self_weak);
                    Box::pin(async move { self_weak.upgrade().unwrap().on_event(ev).await })
                }),
            )
            .await
        };

        match server {
            Ok(server) => Ok(ServerView::new(server, channels_var)),
            Err(err) => {
                error!("{}", err);
                Err(err)
            }
        }
    }

    async fn on_event(self: &Arc<Self>, ev: ServerEvent) {
        use log::{debug, error};
        match ev {
            ServerEvent::OpenChannelIdsChanged(ids) => {
                debug!("Open channel ids: {:?}", &ids);
                self.channels_var.set(ids)
            }
            ServerEvent::Error(err) => error!("{}", err),
            ev => debug!("{:?}", ev),
        }
    }

    pub fn view(self: &Arc<Self>) -> Template<DomNode> {
        let server_var = self.server_var.clone();
        let addr = self.addr.clone();

        let on_close_click = {
            let self_arc = Arc::clone(self);
            move |_| self_arc.servers.remove_server(&self_arc)
        };

        template! {
            div(class = "component") {
                h1 {
                    ("Server")
                }
                button(on:click = on_close_click, class = "close") {
                    ("close")
                }
                div(class = "monospace") {
                    ("address: ")
                    (addr)
                }
                ({
                    let server = server_var.get();

                    match server.as_ref() {
                        Some(Ok(server)) => {
                            server.view()
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

impl Drop for ServerBuilderView {
    fn drop(&mut self) {
        log::trace!("client::ServerBuilderView::drop");
    }
}

use core::cell::RefCell;

use async_std::sync::Arc;
use sycamore::prelude::*;

use crate::ServerBuilderView;

#[derive(Debug)]
pub struct ServersListView {
    addr_var: Signal<String>,
    servers_var: Signal<RefCell<Vec<Arc<ServerBuilderView>>>>,
}

impl ServersListView {
    pub fn new() -> Arc<Self> {
        log::trace!("client::ServersListView::new");

        use crate::default_server_address;

        let addr_var = Signal::new(default_server_address());
        let servers_var = Signal::new(RefCell::new(Vec::new()));

        Arc::new(Self {
            addr_var,
            servers_var,
        })
    }

    pub fn add_server(self: &Arc<Self>) {
        use crate::SignalVecPush;
        let server = ServerBuilderView::new(Arc::clone(self), self.addr_var.get().as_ref().clone());
        self.servers_var.push(server);
    }

    pub fn remove_server(self: &Arc<Self>, server: &Arc<ServerBuilderView>) {
        use crate::SignalVecRemoveByPtrEq;
        self.servers_var.remove_by_ptr_eq(server);
    }

    pub fn view(self: &Arc<Self>) -> Template<DomNode> {
        let addr_var = self.addr_var.clone();

        let on_add_server_click = {
            let self_arc = Arc::clone(self);
            move |_| Arc::clone(&self_arc).add_server()
        };

        let servers_var = self.servers_var.clone();

        template! {
            div(class = "component") {
                h1() {
                    ("Servers")
                }
                div() {
                    label() {
                        ("address: ")
                        input(type = "text", bind:value = addr_var.clone())
                    }
                }
                button(on:click = on_add_server_click) {
                    ("Join server")
                }
                div() {
                    ({
                        Template::new_fragment(
                            servers_var
                                .get()
                                .borrow()
                                .iter()
                                .map(|server| server.view())
                                .collect(),
                        )
                    })
                }
            }
        }
    }
}

impl Drop for ServersListView {
    fn drop(&mut self) {
        log::trace!("client::ServersListView::drop");
    }
}

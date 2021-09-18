use crate::ServersListView;
use sycamore::prelude::*;

pub fn build_app_view() -> Template<DomNode> {
    let servers_view = ServersListView::new();
    servers_view.view()
}

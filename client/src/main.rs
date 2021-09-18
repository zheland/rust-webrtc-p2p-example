#![warn(
    clippy::all,
    rust_2018_idioms,
    missing_copy_implementations,
    missing_debug_implementations,
    single_use_lifetimes,
    trivial_casts,
    unused_import_braces,
    unused_qualifications,
    unused_results
)]

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc<'_> = wee_alloc::WeeAlloc::INIT;

mod app;
mod receiver;
mod receiver_builder;
mod receivers_list;
mod sender;
mod sender_builder;
mod senders_list;
mod server;
mod server_address;
mod server_builder;
mod servers_list;
mod signal_ext;

use app::build_app_view;
use receiver::ReceiverView;
use receiver_builder::ReceiverBuilderView;
use receivers_list::ReceiversListView;
use sender::SenderView;
use sender_builder::SenderBuilderView;
use senders_list::SendersListView;
use server::ServerView;
use server_address::default_server_address;
use server_builder::ServerBuilderView;
use servers_list::ServersListView;
use signal_ext::{SignalVecPush, SignalVecRemoveByPtrEq};

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).unwrap();
    sycamore::render(|| build_app_view());
}

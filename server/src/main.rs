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

mod app;
mod channel;
mod server;
mod server_data;
mod socket;
mod socket_sender;

use app::app;
use channel::{Channel, ChannelIceCandidates, ChannelKind, ChannelReceiver, ChannelSender};
use server::Server;
use server_data::ServerData;
use socket::{Socket, SocketId};
use socket_sender::SocketSender;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    app().await
}

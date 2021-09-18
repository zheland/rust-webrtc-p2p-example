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

/*
    Possible future improvements:
    - Extract common functionality from Sender and Receiver, DataSender and DataReceiver
      in order to reduce duplicate code.
    - Use traits with async function instead of handlers.
*/

mod boxfn;
mod closure;
mod data_receiver;
mod data_sender;
mod local_media;
mod media_receiver;
mod media_sender;
mod media_view;
mod receiver;
mod rtc_configuration;
mod sender;
mod server;
mod websocket;

pub use boxfn::{BoxAsyncFn2, BoxAsyncFn2Wrapper};
pub use closure::{closure_0, closure_1};
pub use data_receiver::{DataReceiver, DataReceiverBuilder, DataReceiverError, DataReceiverEvent};
pub use data_sender::{DataSender, DataSenderError, DataSenderEvent, DataSenderSendError};
pub use local_media::LocalMedia;
pub use media_receiver::{
    MediaReceiver, MediaReceiverBuilder, MediaReceiverError, MediaReceiverEvent,
};
pub use media_sender::MediaSender;
pub use media_view::{MediaView, MediaViewAudio, NewMediaViewError};
pub use receiver::{NewReceiverError, Receiver, ReceiverEvent, ReceiverSendError};
pub use rtc_configuration::{default_rtc_configuration, RtcConfigurationExt};
pub use sender::{NewSenderError, Sender, SenderEvent, SenderSendError};
pub use server::{
    NewServerError, Server, ServerEvent, ServerJoinChannelError, ServerOpenChannelError,
};
pub use websocket::{
    parse_websocket_server_message, send_websocket_client_message, WebSocketClientMessageSendError,
    WebSocketServerMessageParseError,
};

pub use signaling_protocol;

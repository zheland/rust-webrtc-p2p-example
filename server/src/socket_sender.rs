use futures::stream::SplitSink;
use signaling_protocol::{
    ServerMessage, ServerReceiverErrorMessage, ServerReceiverMessage, ServerSenderErrorMessage,
    ServerSenderMessage, SessionReceiverId, SessionSenderId,
};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::WebSocketStream;

#[derive(Debug)]
pub struct SocketSender(Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>);

impl SocketSender {
    pub fn new(sender: SplitSink<WebSocketStream<TcpStream>, Message>) -> Self {
        Self(Mutex::new(sender))
    }

    pub async fn send(&self, message: ServerMessage) {
        use bincode::serialize;
        use futures::SinkExt;
        use log::error;

        let message: Result<Vec<u8>, _> = serialize(&message);
        let message = match message {
            Ok(message) => message,
            Err(err) => {
                error!("send message serialization error: {}", err);
                return;
            }
        };

        match self.0.lock().await.send(Message::Binary(message)).await {
            Ok(()) => {}
            Err(err) => {
                error!("send message error: {}", err);
                return;
            }
        }
    }

    pub async fn send_sender_message(
        &self,
        sender_id: SessionSenderId,
        message: ServerSenderMessage,
    ) {
        self.send(ServerMessage::SenderMessage { sender_id, message })
            .await
    }

    pub async fn send_receiver_message(
        &self,
        receiver_id: SessionReceiverId,
        message: ServerReceiverMessage,
    ) {
        self.send(ServerMessage::ReceiverMessage {
            receiver_id,
            message,
        })
        .await
    }

    pub async fn send_sender_error(
        &self,
        sender_id: SessionSenderId,
        err: ServerSenderErrorMessage,
    ) {
        self.send_sender_message(sender_id, ServerSenderMessage::Error(err))
            .await
    }

    pub async fn send_receiver_error(
        &self,
        receiver_id: SessionReceiverId,
        err: ServerReceiverErrorMessage,
    ) {
        self.send_receiver_message(receiver_id, ServerReceiverMessage::Error(err))
            .await
    }
}

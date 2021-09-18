use signaling_protocol::{ClientMessage, ServerMessage};
use thiserror::Error;
use wasm_bindgen::JsValue;
use web_sys::{MessageEvent, WebSocket};

pub fn parse_websocket_server_message(
    ev: MessageEvent,
) -> Result<ServerMessage, WebSocketServerMessageParseError> {
    use bincode::deserialize;
    use js_sys::{ArrayBuffer, Uint8Array};
    use wasm_bindgen::JsCast;

    let array_buffer: ArrayBuffer = ev
        .data()
        .dyn_into()
        .map_err(WebSocketServerMessageParseError::NonArrayData)?;
    let data = Uint8Array::new(&array_buffer).to_vec();
    Ok(deserialize(&data)?)
}

pub fn send_websocket_client_message(
    web_socket: &WebSocket,
    msg: ClientMessage,
) -> Result<(), WebSocketClientMessageSendError> {
    use bincode::serialize;

    let request: Vec<u8> = serialize(&msg)?;
    web_socket
        .send_with_u8_array(&request)
        .map_err(WebSocketClientMessageSendError::WebSocketSendError)?;
    Ok(())
}

#[derive(Error, Debug)]
pub enum WebSocketServerMessageParseError {
    #[error("non-array websocket data received: {0:?}")]
    NonArrayData(JsValue),
    #[error("websocket data deserialization error: {0}")]
    DeserializationFailed(#[from] bincode::Error),
}

#[derive(Error, Debug)]
pub enum WebSocketClientMessageSendError {
    #[error("WebSocket send error: {0:?}")]
    WebSocketSendError(JsValue),
    #[error("ClientMessageData serialization error: {0}")]
    SerializationFailed(#[from] bincode::Error),
}

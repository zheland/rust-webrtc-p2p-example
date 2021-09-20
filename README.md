# rust-wasm-client-and-server-webrtc-demo (WIP)

## About

Demo includes a WebRTC-client written in rust using the sycamore library and a signal and WebRTC-server.

## State

- [x] Signaling protocol,
- [x] WebSocket signaling server,
- [x] Multiple signaling sessions per one WebSocket-connection,
- [x] Initial WebAssemply client WebRTC library implementation,
- [x] Multiple senders and receivers per one client,
- [x] WebAssemply client Reactive UI,
- [x] Messaging through the signaling server,
- [x] Client Sender PeerToPeer mode
- [ ] Client Sender ClientServer mode
- [x] Client-To-Client WebRTC-connection,
- [x] Client-To-Client video sending and receiving,
- [x] Client-To-Client binary sending and receiving,
- [ ] Client-To-Server-To-Client WebRTC comminication,

## Setup

* Run `bash setup.sh`

## Usage

* Run `bash watch.sh`
* Open `localhost:8080` in browser
* Edit the server address if necessary and click button `[Join server]`.
* Click button `[Open channel]` to start sending video and text using the specified signaling server for the specified channel.
* Click button `[Join channel CHANNELNAME]` to start receiving video and text from the specified channel.

## License

Licensed under either of

* Apache License, Version 2.0,
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any
additional terms or conditions.

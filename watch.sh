#!/bin/bash

trap "kill 0" EXIT

SERVER_ADDRESS=$(hostname -I | awk '{print $1}')
SERVER_PORT="9010"

(
    cd client
    mkdir -p target
    echo $"window.server_address = \"ws://$SERVER_ADDRESS:$SERVER_PORT\";" \
        > target/server_address.js
    trunk serve --release -d dist \
        -w . ../browser-webrtc ../signaling-protocol
) &
CLIENT_PID=$!

(
    cd server
    cargo watch -s "RUST_LOG=debug cargo run -- -a $SERVER_ADDRESS -p $SERVER_PORT"
) &
SERVER_PID=$!

wait
exit

use core::sync::atomic::AtomicU32;
use std::io;
use std::sync::Arc;

use log::info;
use thiserror::Error;
use tokio::net::TcpListener;

use crate::ServerData;

#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
    data: Arc<ServerData>,
    next_socket_id: AtomicU32,
}

impl Server {
    pub async fn new<Address: AsRef<str>>(addr: Address) -> Result<Arc<Self>, NewServerError> {
        let listener = TcpListener::bind(addr.as_ref()).await?;
        info!("started on address: {}", addr.as_ref());
        let data = Arc::new(ServerData::new());
        let next_socket_id = AtomicU32::new(0);

        Ok(Arc::new(Self {
            listener,
            data,
            next_socket_id,
        }))
    }

    pub async fn run(self: Arc<Self>) {
        use crate::{Socket, SocketId};
        use core::sync::atomic::Ordering;
        use tokio::spawn;
        use tokio::task::JoinHandle;

        while let Ok((stream, addr)) = self.listener.accept().await {
            let data = Arc::clone(&self.data);
            let socket_id = SocketId(self.next_socket_id.fetch_add(1, Ordering::Relaxed));
            let _: JoinHandle<()> = spawn(async move {
                let session = Socket::new(socket_id, Arc::clone(&data), stream, addr)
                    .await
                    .unwrap();
                Socket::run(session).await;
                data.update_open_channel_ids().await;
            });
        }
    }
}

#[derive(Error, Debug)]
pub enum NewServerError {
    #[error("TcpListener bind error: {0}")]
    BindTcpListenerError(#[from] io::Error),
}

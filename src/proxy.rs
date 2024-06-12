use std::fmt::Debug;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::trace;

use crate::AdbServerProtocolConnection;

enum Direction {
    IncomingToOutgoing,
    OutgoingToIncoming,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::IncomingToOutgoing => write!(f, "Incoming >> Outgoing"),
            Direction::OutgoingToIncoming => write!(f, "Outgoing >> Incoming"),
        }
    }
}

/// a proxy which connects 2 adb server protocol connections and optionally logs the messages
#[derive(Debug)]
pub struct AdbServerProxy {}

impl AdbServerProxy {
    /// Create a new AdbServerProxy.
    pub fn new() -> AdbServerProxy {
        AdbServerProxy {}
    }

    /// Proxy the incoming stream to the outgoing stream.
    pub async fn proxy<I, O>(
        &self,
        incoming_stream: I,
        outgoing_stream: O,
        connection_id: u32,
    ) -> crate::util::Result<()>
    where
        I: AsyncRead + AsyncWrite + Unpin,
        O: AsyncRead + AsyncWrite + Unpin,
    {
        let mut incoming_stream = AdbServerProtocolConnection::new(incoming_stream);
        let mut outgoing_stream = AdbServerProtocolConnection::new(outgoing_stream);

        loop {
            tokio::select! {
                packet = incoming_stream.next() => {
                    match packet {
                        Some(Ok(packet)) => {
                            trace_message(connection_id, Direction::IncomingToOutgoing, &packet);
                            outgoing_stream.send(packet).await.unwrap();
                        },
                        Some(Err(e)) => return Err(e),
                        None => return Ok(()),
                    }
                }
                packet = outgoing_stream.next() => {
                    match packet {
                        Some(Ok(packet)) => {
                            trace_message(connection_id, Direction::OutgoingToIncoming, &packet);
                            incoming_stream.send(packet).await.unwrap();
                        },
                        Some(Err(e)) => return Err(e),
                        None => return Ok(()),
                    }
                }
            }
        }
    }
}

fn trace_message(connection_id: u32, direction: Direction, message: impl Debug) {
    trace!(
        "Connection: {}\n============================ {} ============================\n{:?}\n",
        connection_id, direction, message
    );
}

#[cfg(test)]
mod tests {
    use core::net::SocketAddr;
    use std::sync::Arc;
    use tokio::{self};

    use super::*;

    #[ignore]
    #[tokio::test]
    async fn test_proxy() -> crate::util::Result<()> {
        let listening_addr = "127.0.0.1:5051".parse::<SocketAddr>().unwrap();
        let connecting_addr = "127.0.0.1:5555".parse::<SocketAddr>().unwrap();

        let listener = tokio::net::TcpListener::bind(listening_addr).await?;
        let proxy = Arc::new(AdbServerProxy::new());
        loop {
            let proxy = Arc::clone(&proxy);
            let (incoming_stream, _) = listener.accept().await?;
            let outgoing_stream = tokio::net::TcpStream::connect(connecting_addr).await?;

            tokio::spawn(async move {
                proxy
                    .proxy(incoming_stream, outgoing_stream, 0)
                    .await
                    .unwrap();
            });
        }
    }   
}

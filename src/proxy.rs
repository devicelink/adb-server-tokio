use std::fmt::Debug;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::trace;

use crate::{AdbServerProtocolConnection, Result};

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

/// Configuration for the AdbServerProxy
#[derive(Debug)]
pub struct AdbServerProxyConfig {
    /// Override the maximum ADB packet size inside the proxy
    pub max_adb_packet_size: Option<u32>,
}

impl AdbServerProxyConfig {
    /// Set the maximum ADB packet size
    pub fn max_adb_packet_size(mut self, max_adb_packet_size: u32) -> Self {
        self.max_adb_packet_size = Some(max_adb_packet_size);
        self
    }
}

impl Default for AdbServerProxyConfig {
    fn default() -> Self {
        AdbServerProxyConfig {
            max_adb_packet_size: None,
        }
    }
}

/// a proxy which connects 2 adb server protocol connections and optionally logs the messages
#[derive(Debug)]
pub struct AdbServerProxy {
    config: AdbServerProxyConfig,
}

impl AdbServerProxy {
    /// Create a new AdbServerProxy.
    pub fn new(config: Option<AdbServerProxyConfig>) -> AdbServerProxy {
        AdbServerProxy {
            config: config.unwrap_or_default(),
        }
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
                    self.forward(connection_id, packet, &mut outgoing_stream, Direction::IncomingToOutgoing).await.unwrap();

                }
                packet = outgoing_stream.next() => {
                    self.forward(connection_id, packet, &mut incoming_stream, Direction::OutgoingToIncoming).await.unwrap();
                }
            }
        }
    }

    async fn forward(&self, connection_id: u32, packet: Option<Result<crate::AdbPacket>>, outgoing_stream: &mut crate::AdbServerProtocolConnection<impl AsyncRead + AsyncWrite + Unpin>, direction: Direction) -> Result<()> {
        match packet {
            Some(Ok(mut packet)) => {
                trace_message(connection_id, direction, &packet);
                if let Some(max_size) = self.config.max_adb_packet_size {
                    override_max_adb_packet_size_on_connect(&mut packet, max_size);
                }
                outgoing_stream.send(packet).await
            },
            Some(Err(e)) => return Err(e),
            None => return Ok(()),
        }
    }
}

fn override_max_adb_packet_size_on_connect(packet: &mut crate::AdbPacket, max_size: u32) {
    match packet.header.command {
        crate::Command::A_CNXN => {
            packet.header.arg1 = max_size;
        },
        _ => {}
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
        let proxy = Arc::new(AdbServerProxy::new(None));
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

use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;
use tokio::net::UdpSocket;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("Socket I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Packet error: {0}")]
    PacketError(String),
}

pub struct UdpSender {
    socket: Arc<UdpSocket>,
    target_addr: SocketAddr,
}

impl UdpSender {
    pub async fn bind(local_port: u16, target_addr: SocketAddr) -> Result<Self, TransportError> {
        let local_addr = format!("0.0.0.0:{}", local_port);
        let socket = UdpSocket::bind(local_addr).await?;
        Ok(Self {
            socket: Arc::new(socket),
            target_addr,
        })
    }

    pub async fn send_packet(&self, packet: &[u8]) -> Result<usize, TransportError> {
        let sent = self.socket.send_to(packet, self.target_addr).await?;
        Ok(sent)
    }

    pub fn socket(&self) -> Arc<UdpSocket> {
        Arc::clone(&self.socket)
    }
}

pub struct UdpReceiver {
    socket: Arc<UdpSocket>,
}

impl UdpReceiver {
    pub async fn bind(listen_port: u16) -> Result<Self, TransportError> {
        let listen_addr = format!("0.0.0.0:{}", listen_port);
        let socket = UdpSocket::bind(listen_addr).await?;
        Ok(Self {
            socket: Arc::new(socket),
        })
    }

    pub async fn recv_packet(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), TransportError> {
        let (bytes, src) = self.socket.recv_from(buf).await?;
        Ok((bytes, src))
    }

    pub fn socket(&self) -> Arc<UdpSocket> {
        Arc::clone(&self.socket)
    }
}

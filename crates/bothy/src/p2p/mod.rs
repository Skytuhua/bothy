//! Direct channel between online devices (WebRTC data channel, [`str0m`]).
//!
//! The channel only speeds things up. After a device uploads a segment, it tells its connected
//! peers, and sends the segment's bytes when they are small, so peers see writes within about a
//! second instead of at their next poll. The remote stays the durable record: nothing travels
//! here that is not already uploaded, and losing the channel only means waiting for the next
//! poll.
//!
//! Devices find each other through signaling files on the remote ([`crate::codec::signal`]).
//! P2P runs while background sync runs ([`crate::Database::start_auto_sync`]), because the
//! signaling files arrive with its polls.

mod driver;
mod stun;
mod wire;

use std::net::IpAddr;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, watch};

use crate::database::Inner;
use crate::model::DeviceId;
use crate::remote::RemoteEntry;

/// Google's public STUN server, the default for finding the address peers outside the local
/// network can reach.
pub const DEFAULT_STUN_SERVER: &str = "stun.l.google.com:19302";

/// Settings for the direct channel.
#[derive(Clone, Debug)]
pub struct P2pConfig {
    pub(crate) stun_servers: Vec<String>,
    pub(crate) host: Option<IpAddr>,
    pub(crate) inline_segments: bool,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            stun_servers: vec![DEFAULT_STUN_SERVER.to_owned()],
            host: None,
            inline_segments: true,
        }
    }
}

impl P2pConfig {
    /// The default: STUN through [`DEFAULT_STUN_SERVER`].
    pub fn new() -> Self {
        Self::default()
    }

    /// STUN servers (`host:port`). An empty list keeps P2P on the local network.
    pub fn stun_servers(mut self, servers: Vec<String>) -> Self {
        self.stun_servers = servers;
        self
    }

    /// The local address to offer to peers instead of the detected one (e.g. `127.0.0.1` in
    /// tests).
    pub fn host_address(mut self, ip: IpAddr) -> Self {
        self.host = Some(ip);
        self
    }

    /// Whether small segments travel over the channel (default) or are only announced, so
    /// peers fetch them from the remote.
    pub fn inline_segments(mut self, on: bool) -> Self {
        self.inline_segments = on;
        self
    }
}

/// State of the channel to one peer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PeerState {
    /// Offer or answer written; waiting for the other side or for the connection.
    Signaling,
    /// The channel is open.
    Connected,
    /// The connection could not be made; retried later. Sync goes through the remote.
    Failed,
    /// The channel closed; retried when the peer is online again.
    Disconnected,
}

/// One peer of [`P2pStatus`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PeerStatus {
    /// The peer device.
    pub device_id: DeviceId,
    /// Its channel.
    pub state: PeerState,
}

/// Snapshot of the direct channels.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct P2pStatus {
    /// Whether P2P is running (it runs with background sync when configured).
    pub running: bool,
    /// Looking for peers or waiting for one to answer: background sync polls every few
    /// seconds meanwhile.
    pub discovering: bool,
    /// Peers this device knows about, by device id.
    pub peers: Vec<PeerStatus>,
}

impl P2pStatus {
    /// Number of open channels.
    pub fn connected(&self) -> usize {
        self.peers
            .iter()
            .filter(|p| p.state == PeerState::Connected)
            .count()
    }
}

/// Commands from the engine to the P2P task.
pub(crate) enum Cmd {
    /// We uploaded segment `seq`.
    Announce { seq: u64, bytes: Vec<u8> },
    /// Signaling files seen by a sync.
    Signals(Vec<RemoteEntry>),
    /// Stop, delete our signaling files, then reply.
    Stop(oneshot::Sender<()>),
}

/// The engine's handle on a running P2P task.
pub(crate) struct Link {
    cmd: mpsc::UnboundedSender<Cmd>,
    status: watch::Receiver<P2pStatus>,
}

impl Link {
    pub(crate) fn send(&self, cmd: Cmd) {
        let _ = self.cmd.send(cmd);
    }

    pub(crate) fn status(&self) -> P2pStatus {
        self.status.borrow().clone()
    }
}

/// Starts the P2P task; it runs until [`stop`].
pub(crate) fn start(inner: &Arc<Inner>, config: P2pConfig) -> tokio::task::JoinHandle<()> {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (status_tx, status_rx) = watch::channel(P2pStatus {
        running: true,
        discovering: true,
        peers: Vec::new(),
    });
    inner.set_p2p(Some(Link {
        cmd: cmd_tx,
        status: status_rx,
    }));
    tokio::spawn(driver::run(inner.clone(), config, cmd_rx, status_tx))
}

/// Stops the P2P task and waits until it has cleaned up (bounded).
pub(crate) async fn stop(inner: &Arc<Inner>, task: tokio::task::JoinHandle<()>) {
    if let Some(link) = inner.take_p2p() {
        let (tx, rx) = oneshot::channel();
        link.send(Cmd::Stop(tx));
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), rx).await;
    }
    task.abort();
    let _ = task.await;
}

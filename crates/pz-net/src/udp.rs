//! Plain `std::net` UDP transport (no async runtime, works the same on every platform).
use crate::{ClientId, ClientMsg, Server, ServerMsg};
use std::{collections::HashMap, io, net::{SocketAddr, ToSocketAddrs, UdpSocket}, time::{Duration, Instant}};

/// Clients silent for this long are dropped.
const TIMEOUT: Duration = Duration::from_secs(10);

const MAX_DATAGRAM: usize = 65_507;

/// Wraps a [`Server`]: unknown senders are joined automatically at `spawn`.
pub struct UdpServer {
    pub server: Server,
    socket: UdpSocket,
    clients: HashMap<SocketAddr, (ClientId, Instant)>,
    next_id: ClientId,
    spawn: [f32; 2],
}

impl UdpServer {
    pub fn bind(addr: impl ToSocketAddrs, server: Server, spawn: [f32; 2]) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self { server, socket, clients: HashMap::new(), next_id: 1, spawn })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> { self.socket.local_addr() }
    pub fn client_count(&self) -> usize { self.clients.len() }

    /// Drain pending datagrams into the simulation. Malformed packets are dropped.
    pub fn poll(&mut self) -> io::Result<()> {
        let mut buf = vec![0u8; MAX_DATAGRAM];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((n, from)) => {
                    let Ok(msg) = ClientMsg::decode(&buf[..n]) else { continue };
                    let entry = self.clients.entry(from).or_insert_with(|| {
                        let id = self.next_id;
                        self.next_id += 1;
                        self.server.join(id, self.spawn[0], self.spawn[1]);
                        (id, Instant::now())
                    });
                    entry.1 = Instant::now();
                    let id = entry.0;
                    self.server.handle(id, msg);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                // Windows reports ICMP "port unreachable" from an earlier send as a recv error.
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                Err(e) => return Err(e),
            }
        }
    }

    /// Advance one tick and send the snapshot to every client.
    pub fn step(&mut self) -> io::Result<()> {
        let stale: Vec<SocketAddr> = self.clients.iter().filter(|(_, (_, t))| t.elapsed() > TIMEOUT).map(|(a, _)| *a).collect();
        for a in stale {
            if let Some((id, _)) = self.clients.remove(&a) { self.server.leave(id); }
        }
        let mut snap = self.server.tick();
        for (addr, (id, _)) in &self.clients {
            if let ServerMsg::Snapshot { you, .. } = &mut snap { *you = *id; }
            match self.socket.send_to(&snap.encode(), addr) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

pub struct UdpClient { socket: UdpSocket, seq: u32 }

impl UdpClient {
    pub fn connect(server: impl ToSocketAddrs) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.connect(server)?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket, seq: 0 })
    }

    pub fn send_input(&mut self, dir: [f32; 2], run: bool) -> io::Result<()> {
        self.seq += 1;
        self.socket.send(&ClientMsg::Input { seq: self.seq, dir, run }.encode()).map(|_| ())
    }

    /// Latest snapshot waiting in the socket, if any (older ones are discarded).
    pub fn latest_snapshot(&mut self) -> io::Result<Option<ServerMsg>> {
        let mut buf = vec![0u8; MAX_DATAGRAM];
        let mut latest = None;
        loop {
            match self.socket.recv(&mut buf) {
                Ok(n) => if let Ok(m) = ServerMsg::decode(&buf[..n]) { latest = Some(m); },
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(latest),
                Err(e) => return Err(e),
            }
        }
    }
}

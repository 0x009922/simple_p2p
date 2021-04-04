mod peers_map;

use crate::message::Message;
use log::info;
use message_io::events::EventQueue;
use message_io::network::{Endpoint, NetEvent, Network, Transport};
use peers_map::{PeerAddr, PeersMap};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct Peer {
    peers: Arc<Mutex<peers_map::PeersMap>>,
    public_addr: SocketAddr,
    period: u32,
    network: Arc<Mutex<Network>>,
    event_queue: EventQueue<NetEvent>,
    connect: Option<String>,
}

impl Peer {
    pub fn new(port: u32, period: u32, connect: Option<String>) -> anyhow::Result<Self> {
        let (mut network, event_queue) = Network::split();

        // Listening own addr
        let listen_addr = format!("127.0.0.1:{}", port);
        match network.listen(Transport::FramedTcp, &listen_addr) {
            Ok((_, addr)) => {
                log_my_address(&addr);

                Ok(Self {
                    event_queue,
                    network: Arc::new(Mutex::new(network)),
                    period,
                    connect,
                    public_addr: addr,
                    peers: Arc::new(Mutex::new(PeersMap::new(addr))),
                })
            }
            Err(_) => Err(format!("Can not listen on {}", listen_addr)),
        }
    }

    pub fn run(mut self) {
        if let Some(addr) = &self.connect {
            let mut network = self.network.lock().unwrap();

            // Connection to the first peer
            match network.connect(Transport::FramedTcp, addr) {
                Ok((endpoint, _)) => {
                    {
                        let mut peers = self.peers.lock().unwrap();
                        peers.add_old_one(endpoint);
                    }

                    // Передаю свой публичный адрес
                    send_message(
                        &mut network,
                        endpoint,
                        &Message::MyPubAddr(self.public_addr.clone()),
                    );

                    // Request a list of existing peers
                    // Response will be in event queue
                    send_message(&mut network, endpoint, &Message::GiveMeAListOfPeers);
                }
                Err(_) => {
                    println!("Failed to connect to {}", &addr);
                }
            }
        }

        // spawning thread which will be send random messages to known peers
        self.spawn_emit_loop();

        loop {
            match self.event_queue.receive() {
                // Waiting events
                NetEvent::Message(message_sender, input_data) => {
                    match bincode::deserialize(&input_data).unwrap() {
                        Message::MyPubAddr(pub_addr) => {
                            let mut peers = self.peers.lock().unwrap();
                            peers.add_new_one(message_sender, pub_addr);
                        }
                        Message::GiveMeAListOfPeers => {
                            let list = {
                                let peers = self.peers.lock().unwrap();
                                peers.get_peers_list()
                            };
                            let msg = Message::TakePeersList(list);
                            send_message(&mut self.network.lock().unwrap(), message_sender, &msg);
                        }
                        Message::TakePeersList(addrs) => {
                            let filtered: Vec<&SocketAddr> = addrs
                                .iter()
                                .filter_map(|x| {
                                    // Проверяю, чтобы не было себя
                                    if x != &self.public_addr {
                                        Some(x)
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            log_connected_to_the_peers(&filtered);

                            let mut network = self.network.lock().unwrap();

                            for peer in filtered {
                                if peer == &message_sender.addr() {
                                    continue;
                                }

                                // к каждому подключиться и послать свой публичный адрес
                                // и запомнить

                                // connecting to peer
                                let (endpoint, _) =
                                    network.connect(Transport::FramedTcp, *peer).unwrap();

                                // sending public address
                                let msg = Message::MyPubAddr(self.public_addr);
                                send_message(&mut network, endpoint, &msg);

                                // saving peer
                                self.peers.lock().unwrap().add_old_one(endpoint);
                                // self.peers.add_old_one(endpoint);
                            }
                        }
                        Message::Info(text) => {
                            let pub_addr = self
                                .peers
                                .lock()
                                .unwrap()
                                .get_pub_addr(&message_sender)
                                .unwrap();
                            log_message_received(&pub_addr, &text);
                        }
                    }
                }
                NetEvent::Connected(_, _) => {}
                NetEvent::Disconnected(endpoint) => {
                    let mut peers = self.peers.lock().unwrap();
                    PeersMap::drop(&mut peers, endpoint);
                    // self.peers.drop(endpoint);
                }
            }
        }
    }

    fn spawn_emit_loop(&self) {
        let sleep_duration = Duration::from_secs(self.period as u64);
        let peers_mut = Arc::clone(&self.peers);
        let network_mut = Arc::clone(&self.network);

        thread::spawn(move || {
            // sleeping and sending
            loop {
                thread::sleep(sleep_duration);

                let peers = peers_mut.lock().unwrap();
                let receivers = peers.receivers();

                // if there are no receivers, skip
                if receivers.len() == 0 {
                    continue;
                }

                let mut network = network_mut.lock().unwrap();

                let msg_text = generate_random_message();
                let msg = Message::Info(msg_text.clone());

                log_sending_message(
                    &msg_text,
                    &receivers
                        .iter()
                        .map(|PeerAddr { public, .. }| public)
                        .collect(),
                );

                for PeerAddr { endpoint, .. } in receivers {
                    send_message(&mut network, endpoint, &msg);
                }
            }
        });
    }
}

fn send_message(network: &mut Network, to: Endpoint, msg: &Message) {
    let output_data = bincode::serialize(msg).unwrap();
    network.send(to, &output_data);
}

fn generate_random_message() -> String {
    petname::Petnames::default().generate_one(2, "-")
}

trait ToSocketAddr {
    fn get_addr(&self) -> SocketAddr;
}

impl ToSocketAddr for Endpoint {
    fn get_addr(&self) -> SocketAddr {
        self.addr()
    }
}

impl ToSocketAddr for &Endpoint {
    fn get_addr(&self) -> SocketAddr {
        self.addr()
    }
}

impl ToSocketAddr for SocketAddr {
    fn get_addr(&self) -> SocketAddr {
        self.clone()
    }
}

impl ToSocketAddr for &SocketAddr {
    fn get_addr(&self) -> SocketAddr {
        *self.clone()
    }
}

fn format_list_of_addrs<T: ToSocketAddr>(items: &Vec<T>) -> String {
    if items.len() == 0 {
        "[no one]".to_owned()
    } else {
        let joined = items
            .iter()
            .map(|x| format!("\"{}\"", ToSocketAddr::get_addr(x)))
            .collect::<Vec<String>>()
            .join(", ");

        format!("[{}]", joined)
    }
}

fn log_message_received<T: ToSocketAddr>(from: &T, text: &str) {
    info!(
        "Received message [{}] from \"{}\"",
        text,
        ToSocketAddr::get_addr(from)
    );
}

fn log_my_address<T: ToSocketAddr>(addr: &T) {
    info!("My address is \"{}\"", ToSocketAddr::get_addr(addr));
}

fn log_connected_to_the_peers<T: ToSocketAddr>(peers: &Vec<T>) {
    info!("Connected to the peers at {}", format_list_of_addrs(peers));
}

fn log_sending_message<T: ToSocketAddr>(message: &str, receivers: &Vec<T>) {
    info!(
        "Sending message [{}] to {}",
        message,
        format_list_of_addrs(receivers)
    );
}

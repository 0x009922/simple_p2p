use crate::message::Message;
use message_io::events::EventQueue;
use message_io::network::{Endpoint, NetEvent, Network, Transport};

use std::net::SocketAddr;

pub struct Peer {
    peers: peers_map::PeersMap,
    public_addr: SocketAddr,
    period: u32,
    network: Network,
    event_queue: EventQueue<NetEvent>,
    connect: Option<String>,
}



impl Peer {
    pub fn new(port: u32, period: u32, connect: Option<String>) -> Result<Self, String> {
        let (mut network, event_queue) = Network::split();

        // Listening own addr
        let listen_addr = format!("127.0.0.1:{}", port);
        match network.listen(Transport::FramedTcp, &listen_addr) {
            Ok((_, addr)) => {
                println!("Listening on {}", listen_addr);
                Ok(Self {
                    event_queue,
                    network,
                    period,
                    connect,
                    public_addr: addr,
                    peers: peers_map::PeersMap::new(addr),
                })
            }
            Err(_) => Err(format!("Can not listen on {}", listen_addr)),
        }

        // // Connection to the first peer
        // let connect_peer = connect;
        // match network.connect(Transport::FramedTcp, connect_peer) {
        //     Ok((endpoint, _)) => {
        //         let mut peers: HashSet<Endpoint> = HashSet::new();
        //         peers.insert(endpoint);

        //         println!("Connected to {}", connect);

        //         Ok(Self {
        //             event_queue,
        //             network,
        //             emit_period: period,
        //             peers,
        //         })
        //     }
        //     Err(_) => {
        //         Err(format!(
        //             "Can not connect to the discovery server at {}",
        //             connect_peer
        //         ))
        //     }
        // }
    }

    pub fn run(mut self) {
        // firstly peer should take all existing peers and connect to them
        // then, peer should send to each of them a random message every N seconds
        // and peer should listen to messages and handle them

        if let Some(addr) = &self.connect {
            // Connection to the first peer
            match self.network.connect(Transport::FramedTcp, addr) {
                Ok((endpoint, _)) => {
                    self.peers.add_old_one(endpoint);

                    // Передаю свой публичный адрес
                    self.send_message(endpoint, Message::MyPubAddr(self.public_addr.clone()));

                    // Request a list of existing peers
                    self.send_message(endpoint, Message::GiveMeAListOfPeers);
                }
                Err(_) => {
                    println!("Failed to connect to {}", &addr);
                }
            }
        }

        // // at this moment in peers set may be only one peer - connection peer
        // if let Some(connect_peer) = self.peers.iter().next() {
        //     let message = Message::Info("Hey!".to_owned());
        //     let output_data = bincode::serialize(&message).unwrap();
        //     self.network.send(*connect_peer, &output_data);
        // }

        loop {
            match self.event_queue.receive() {
                // Waiting events
                NetEvent::Message(message_sender, input_data) => {
                    match bincode::deserialize(&input_data).unwrap() {
                        Message::MyPubAddr(pub_addr) => {
                            self.peers.add_new_one(message_sender, pub_addr);
                        }
                        Message::GiveMeAListOfPeers => {
                            let list = self.peers.get_peers_list();
                            let msg = Message::TakePeersList(list);
                            self.send_message(message_sender, msg);
                        }
                        Message::TakePeersList(addrs) => {
                            println!("Taking peers: {:?}", addrs);

                            let filtered: Vec<SocketAddr> = addrs
                                .iter()
                                .filter_map(|x| {
                                    // Проверяю, чтобы не было себя и кого-то, к кому есть подключение
                                    // (а это по идее может быть только текущий отправитель)
                                    if x != &self.public_addr && x != &message_sender.addr() {
                                        Some(x.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            for peer in filtered {
                                // к каждому подключиться и послать свой публичный адрес
                                // и запомнить

                                let (endpoint, _) =
                                    self.network.connect(Transport::FramedTcp, peer).unwrap();
                                let msg = Message::MyPubAddr(self.public_addr);
                                self.send_message(endpoint, msg);

                                self.peers.add_old_one(endpoint);
                            }
                        }
                        Message::Info(text) => {
                            log_message_received(&message_sender, &text);
                        }
                    }
                }
                NetEvent::Connected(_, _) => {
                    // self.register_peer(endpoint);
                }
                NetEvent::Disconnected(endpoint) => {
                    self.peers.drop(endpoint);
                }
            }
        }
    }

    // fn register_peer(&mut self, peer: Endpoint, info: PeerInfo) {
    //     self.peers.insert(peer, info);
    //     self.check_peers();
    // }

    // fn unregister_peer(&mut self, peer: Endpoint) {
    //     self.peers.remove(&peer);
    //     self.check_peers();
    // }

    // fn check_peers(&self) {
    //     let formatted: Vec<(SocketAddr, SocketAddr)> = self
    //         .peers
    //         .iter()
    //         .map(|(endpoint, info)| (endpoint.addr(), info.listen_addr.clone()))
    //         .collect();
    //     println!("Peers: {:?}", formatted);
    // }

    fn send_message(&mut self, to: Endpoint, msg: Message) {
        let output_data = bincode::serialize(&msg).unwrap();
        self.network.send(to, &output_data);
    }
}

mod peers_map {
    use message_io::network::Endpoint;
    use std::collections::HashMap;
    use std::net::SocketAddr;

    pub struct PeersMap {
        map: HashMap<Endpoint, PeerInfo>,
        self_pub_addr: SocketAddr,
    }

    #[derive(Debug)]
    enum PeerInfo {
        OldOne,
        NewOne(SocketAddr),
    }

    impl PeersMap {
        pub fn new(self_pub_addr: SocketAddr) -> Self {
            Self {
                map: HashMap::new(),
                self_pub_addr
            }
        }

        pub fn add_old_one(&mut self, endpoint: Endpoint) {
            println!("add old one: {}", endpoint.addr());
            self.map.insert(endpoint, PeerInfo::OldOne);
        }

        pub fn add_new_one(&mut self, endpoint: Endpoint, pub_addr: SocketAddr) {
            println!("add new one: {} ({})", endpoint.addr(), pub_addr);
            self.map.insert(endpoint, PeerInfo::NewOne(pub_addr));
        }

        pub fn drop(&mut self, endpoint: Endpoint) {
            println!("drop: {}", endpoint.addr());
            self.map.remove(&endpoint);
        }

        pub fn get_peers_list(&self) -> Vec<SocketAddr> {
            let mut list: Vec<SocketAddr> = Vec::with_capacity(self.map.len() + 1);
            list.push(self.self_pub_addr);
            self.map
                .iter()
                .map(|(endpoint, info)| match info {
                    PeerInfo::OldOne => endpoint.addr(),
                    PeerInfo::NewOne(public_addr) => public_addr.clone(),
                })
                .for_each(|addr| {
                    list.push(addr);
                });

            list
        }
    }
}

trait PeerAddrFmt {
    fn format(&self) -> String;
}

impl PeerAddrFmt for Endpoint {
    fn format(&self) -> String {
        format!("\"{}\" ({})", self.addr(), self)
    }
}

fn log_message_received<T: PeerAddrFmt>(from: &T, text: &str) {
    println!(
        "Received message [{}] from {}",
        text,
        PeerAddrFmt::format(from)
    );
}

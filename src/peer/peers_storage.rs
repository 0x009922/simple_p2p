use message_io::network::Endpoint;
use std::collections::HashMap;
use std::net::SocketAddr;

/// Storage for peers - old ones and new ones
pub struct PeersStorage<T: PeerEndpoint> {
    map: HashMap<T, PeerInfo>,
    self_pub_addr: SocketAddr,
}

/// Trait that generalizes endpoint behavior - for tests
pub trait PeerEndpoint {
    fn addr(&self) -> SocketAddr;
}

impl PeerEndpoint for Endpoint {
    fn addr(&self) -> SocketAddr {
        self.addr()
    }
}

/// PeerAddr contains public peer addr (which he listens to)
/// and actual connection endpoint. If peer is "old", endpoint equals public
#[derive(Debug, PartialEq)]
pub struct PeerAddr<T: PeerEndpoint> {
    pub public: SocketAddr,
    pub endpoint: T,
}

// #[derive(Debug)]
/// Peer may be old, or may be new. If new, it contains his public addr
enum PeerInfo {
    OldOne,
    NewOne(SocketAddr),
}

impl<T: PeerEndpoint + std::hash::Hash + std::cmp::Eq + Clone> PeersStorage<T> {
    pub fn new(self_pub_addr: SocketAddr) -> Self {
        Self {
            map: HashMap::new(),
            self_pub_addr,
        }
    }

    pub fn add_old_one(&mut self, endpoint: T) {
        self.map.insert(endpoint, PeerInfo::OldOne);
    }

    pub fn add_new_one(&mut self, endpoint: T, pub_addr: SocketAddr) {
        self.map.insert(endpoint, PeerInfo::NewOne(pub_addr));
    }

    pub fn drop(&mut self, endpoint: T) {
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

    pub fn receivers(&self) -> Vec<PeerAddr<T>> {
        self.map
            .iter()
            .map(|(endpoint, info)| {
                let public = match info {
                    PeerInfo::OldOne => endpoint.addr(),
                    PeerInfo::NewOne(public_addr) => public_addr.clone(),
                };
                PeerAddr {
                    endpoint: endpoint.clone(),
                    public,
                }
            })
            .collect()
    }

    pub fn get_pub_addr(&self, endpoint: &T) -> Option<SocketAddr> {
        self.map.get(endpoint).map(|founded| match founded {
            PeerInfo::OldOne => endpoint.addr(),
            PeerInfo::NewOne(addr) => addr.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{PeerAddr, PeerEndpoint, PeersStorage as Store, SocketAddr};
    use std::hash::Hash;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct Endpoint {
        addr: SocketAddr,
    }

    impl Endpoint {
        fn new(addr: SocketAddr) -> Self {
            Self { addr }
        }
    }

    impl PeerEndpoint for Endpoint {
        fn addr(&self) -> SocketAddr {
            self.addr
        }
    }

    fn socket_addr(s: &str) -> SocketAddr {
        s.parse().expect("Unable to parse string to SocketAddr")
    }

    fn endpoint(s: &str) -> Endpoint {
        Endpoint::new(socket_addr(s))
    }

    #[test]
    fn initially_only_self_addr_in_list() {
        let addr = socket_addr("127.0.0.1:4412");
        let map: Store<Endpoint> = Store::new(addr);

        assert_eq!(map.get_peers_list(), vec![addr]);
    }

    #[test]
    fn old_one_appears() {
        let self_addr = socket_addr("127.0.0.1:4412");
        let old_addr = socket_addr("127.0.0.1:8000");
        let old = Endpoint::new(old_addr);

        let mut map: Store<Endpoint> = Store::new(self_addr);
        map.add_old_one(old);

        assert_eq!(map.get_peers_list(), vec![self_addr, old_addr]);
    }

    #[test]
    fn get_old_one_pub_addr() {
        let self_addr = socket_addr("127.0.0.1:4412");
        let old_addr = socket_addr("127.0.0.1:8000");
        let old = Endpoint::new(old_addr);

        let mut map: Store<Endpoint> = Store::new(self_addr);
        map.add_old_one(old.clone());

        assert_eq!(map.get_pub_addr(&old), Some(old_addr));
    }

    #[test]
    fn old_one_receiver() {
        let self_addr = socket_addr("127.0.0.1:4412");
        let old_addr = socket_addr("127.0.0.1:8000");
        let old = Endpoint::new(old_addr);

        let mut map: Store<Endpoint> = Store::new(self_addr);
        map.add_old_one(old.clone());

        assert_eq!(
            map.receivers(),
            vec![PeerAddr {
                endpoint: old,
                public: old_addr
            }]
        );
    }

    #[test]
    fn new_one_appear() {
        let self_addr = socket_addr("127.0.0.1:4412");
        let new_addr = socket_addr("127.0.0.1:51523");
        let new_pub_addr = socket_addr("127.0.0.1:8000");
        let new_end = Endpoint::new(new_addr);

        let mut map: Store<Endpoint> = Store::new(self_addr);
        map.add_new_one(new_end, new_pub_addr);

        assert_eq!(map.get_peers_list(), vec![self_addr, new_pub_addr]);
    }

    #[test]
    fn get_new_one_pub_addr() {
        let self_addr = socket_addr("127.0.0.1:4412");
        let new_addr = socket_addr("127.0.0.1:51523");
        let new_pub_addr = socket_addr("127.0.0.1:8000");
        let new_end = Endpoint::new(new_addr);

        let mut map: Store<Endpoint> = Store::new(self_addr);
        map.add_new_one(new_end.clone(), new_pub_addr);

        assert_eq!(map.get_pub_addr(&new_end), Some(new_pub_addr));
    }

    #[test]
    fn new_one_receiver() {
        let self_addr = socket_addr("127.0.0.1:4412");
        let new_addr = socket_addr("127.0.0.1:51523");
        let new_pub_addr = socket_addr("127.0.0.1:8000");
        let new_end = Endpoint::new(new_addr);

        let mut map: Store<Endpoint> = Store::new(self_addr);
        map.add_new_one(new_end.clone(), new_pub_addr);

        assert_eq!(
            map.receivers(),
            vec![PeerAddr {
                endpoint: new_end,
                public: new_pub_addr
            }]
        );
    }

    #[test]
    fn dropping_endpoint_works() {
        let mut map: Store<Endpoint> = Store::new(socket_addr("127.0.0.1:8000"));

        map.add_old_one(endpoint("127.0.0.1:8001"));
        map.drop(endpoint("127.0.0.1:8001"));

        assert_eq!(map.get_peers_list(), vec![socket_addr("127.0.0.1:8000")]);
    }
}

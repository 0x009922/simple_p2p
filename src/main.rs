mod logger;
mod message;
mod peer;

use clap::{value_t, App, Arg};
use peer::Peer;

fn main() {
    let arg_matches = App::new("simple_p2p")
        .version("0.1.0")
        .author("0x009922 <a.marcius26@gmail.com>")
        .about("Simple implementation of peer-to-peer network")
        .arg(
            Arg::with_name("port")
                .long("port")
                .long_help("Sets the port to listen to.\n   Example: --port 8000")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("period")
                .long("period")
                .long_help("Sets the period (in seconds) of emitting messages to other peers.\n   Example: --period 5")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("connect")
                .long("connect")
                .long_help("Sets the optional peer addr to connect to.\n   Example: --connect 127.0.0.1:8000")
                .takes_value(true),
        )
        .get_matches();

    let port = value_t!(arg_matches, "port", u32).unwrap();
    let period = value_t!(arg_matches, "period", u32).unwrap();
    let connect = value_t!(arg_matches, "connect", String).ok();

    logger::init();

    Peer::new(port, period, connect).unwrap().run();
}

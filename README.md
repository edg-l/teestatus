# teestatus
Request info about teeworlds servers.

Example
```rust,no_run
use teestatus::*;
use std::net::UdpSocket;

fn main() {
	env_logger::init();
	let sock = UdpSocket::bind("0.0.0.0:0").expect("can't bind socket");
	sock.connect("0.0.0.0:8303")
		.expect("can't connect socket");
	println!("info: {:#?}", ServerInfo::new(&sock).unwrap());
}
```

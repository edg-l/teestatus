use std::borrow::Cow;
use std::net::UdpSocket;
use teestatus::*;
use std::time::Duration;

fn main() {
    env_logger::init();

    // No 0.7 support yet.
    //
    let master4 = MasterServer {
        hostname: Cow::Borrowed("49.12.97.180"),
        port: 8300,
    };

    let master3 = MasterServer {
        hostname: Cow::Borrowed("51.255.129.49"),
        port: 8300,
    };

    // These 2 don't work yet since they are 0.7.
    let master2 = MasterServer {
        hostname: Cow::Borrowed("51.89.37.201"),
        port: 8300,
    };

    let master1 = MasterServer {
        hostname: Cow::Borrowed("164.132.193.153"),
        port: 8300,
    };

    let timeout = 250;

    let sock = UdpSocket::bind("0.0.0.0:0").expect("can't bind socket");
    sock.set_write_timeout(Some(Duration::from_millis(timeout)))
        .unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(timeout)))
        .unwrap();

    let mut servers = master3.get_server_list(&sock).unwrap();
    println!("Loaded {}", servers.len());
    servers.extend(&master4.get_server_list(&sock).unwrap());
    println!("Loaded {}", servers.len());
    servers.extend(&master2.get_server_list(&sock).unwrap());
    println!("Loaded {}", servers.len());
    servers.extend(&master1.get_server_list(&sock).unwrap());
    println!("Loaded {}", servers.len());

    let mut server_infos = vec![];

    for (ip, port) in servers.iter() {
        let addr = format!("{}:{}", ip.to_string(), port);
        sock.connect(addr.clone()).unwrap();
        match ServerInfo::new(&sock) {
            Ok(info) => {
                println!("Loaded server '{}'", info.name);
                println!(
                    "Server has {} connected players ({}/{})",
                    info.players.len(),
                    info.client_count,
                    info.max_client_count
                );
                server_infos.push(info);
            }
            Err(e) => {
                println!("Error loading server: {}", addr);
                println!("Error: {:?}", e);
            }
        }
    }

    println!("Loaded {} servers out of {}.", server_infos.len(), servers.len());
}

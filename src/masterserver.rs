use byteorder::{BigEndian, ReadBytesExt};
use std::net::{Ipv6Addr, UdpSocket};
use std::{
    borrow::Cow,
    net::{IpAddr, Ipv4Addr},
};

use crate::errors::*;
use crate::util::*;

pub struct MasterServer<'a> {
    pub hostname: Cow<'a, str>,
    pub port: u16,
}

// https://github.com/DaRealFreak/Teeworlds-ServerInfo/blob/master/tw_serverinfo/master_servers.py

impl<'a> MasterServer<'a> {
    // Returns a vector filled with a pair of ip + port.
    pub fn get_server_list(&self, sock: &UdpSocket) -> Result<(Vec<(IpAddr, u16)>)> {
        sock.connect(format!("{}:{}", self.hostname, self.port))
            .unwrap();

        sock.set_nonblocking(true)?;

        let (buf, _, _) = create_packet(PacketType::GetCount, Some(b"\xff\xff"), false);
        let sent = sock.send(&buf)?;
        log::debug!("sent = {}", sent);

        let (buf, _, _) = create_packet(PacketType::GetList, Some(b"\xff\xff"), false);
        let sent = sock.send(&buf)?;
        log::debug!("sent = {}", sent);

        let (buf, _, _) = create_packet(PacketType::GetInfo, Some(b"xe"), true);
        let sent = sock.send(&buf)?;
        log::debug!("sent = {}", sent);

        sock.set_nonblocking(false)?;

        let mut count = None;
        let mut servers = vec![];

        loop {
            let mut recvbuf = [0; 1400];
            let res = sock.recv(&mut recvbuf);
            // log::debug!("data received: {:?}", recvbuf);

            match res {
                Err(_) => break,
                Ok(res) => {
                    if res > 0 {
                        let packet_id = &recvbuf[10..14];

                        log::debug!("Received packet with id: {:?}", packet_id);

                        if PacketType::Count == *packet_id {
                            log::debug!("Processing Count packet.");
                            let mut val = &recvbuf[14..=15];
                            count = Some(val.read_u16::<BigEndian>()?);
                            log::debug!("master server count: {:?}", count);
                        } else if PacketType::List == *packet_id {
                            log::debug!("Processing List packet.");
                            let mut ip;
                            for i in (14..(recvbuf.len() - 14)).step_by(18) {
                                if &recvbuf[i..i + 12]
                                    == b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\xff\xff"
                                {
                                    let mut raw = &recvbuf[i + 12..i + 16];
                                    ip = IpAddr::V4(Ipv4Addr::new(
                                        raw.read_u8()?,
                                        raw.read_u8()?,
                                        raw.read_u8()?,
                                        raw.read_u8()?,
                                    ));
                                } else {
                                    let mut raw = &recvbuf[i..i + 16];
                                    ip = IpAddr::V6(Ipv6Addr::new(
                                        raw.read_u16::<BigEndian>()?,
                                        raw.read_u16::<BigEndian>()?,
                                        raw.read_u16::<BigEndian>()?,
                                        raw.read_u16::<BigEndian>()?,
                                        raw.read_u16::<BigEndian>()?,
                                        raw.read_u16::<BigEndian>()?,
                                        raw.read_u16::<BigEndian>()?,
                                        raw.read_u16::<BigEndian>()?,
                                    ));
                                }

                                let port = (&recvbuf[i + 16..i + 18]).read_u16::<BigEndian>()?;

                                if port == 0 || ip.is_unspecified() {
                                    break;
                                }
                                log::debug!("Adding ip '{}' and port {}", ip, port);
                                servers.push((ip, port));

                                if let Some(count) = count {
                                    if servers.len() >= count as usize {
                                        log::debug!("Added all servers.");
                                        break;
                                    }
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        Ok(servers)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::time::Duration;

    #[test]
    fn it_works() {
        let master = MasterServer {
            hostname: Cow::Borrowed("49.12.97.180"),
            port: 8300,
        };
        let sock = UdpSocket::bind("0.0.0.0:0").expect("can't bind socket");
        sock.set_write_timeout(Some(Duration::from_millis(400)))
            .unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(400)))
            .unwrap();
        master.get_server_list(&sock).unwrap();
    }
}

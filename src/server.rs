use bytes::Buf;
use std::net::UdpSocket;
use std::slice::IterMut;
use std::collections::HashSet;

use crate::util::*;
use crate::common::*;
use crate::errors::*;

#[derive(Debug)]
pub struct ServerInfo {
    pub version: String,
    pub name: String,
    pub map: String,
    pub password: bool,
    pub game_type: String,
    pub player_count: i32,
    pub max_player_count: i32,
    pub client_count: i32,
    pub max_client_count: i32,
    pub mapcrc: i32,
    pub mapsize: i32,
    pub players: Vec<Player>,
}

fn get_player(data: &mut IterMut<&[u8]>, is_modern: bool) -> Result<Player> {
    let name = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;
    let clan = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;
    let country = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;
    let score = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;
    let is_spectator = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;

    if is_modern {
        // Reserved
        data.next();
    }

    Ok(Player {
        name: name.to_string(),
        clan: clan.to_string(),
        country: country.parse()?,
        score: score.parse()?,
        is_spectator: is_spectator.parse::<i32>()? == 0,
    })
}

impl ServerInfo {
    /// The socket needs to be previously `connect`ed to a remote address.
    pub fn new(sock: &UdpSocket) -> Result<ServerInfo> {
        let (buf, extra_token, token) = create_packet(PacketType::GetInfo, Some(b"xe"), true);

        let token = token.expect("token should always have value here.");

        log::debug!("generated extra_token={}, token={}", extra_token, token);

        let sent = sock.send(&buf)?;

        log::debug!("sent {} bytes", sent);
        if sent != buf.len() {
            log::warn!(
                "bytes sent ({}) not equal to buffer size ({})!",
                sent,
                buf.len()
            );
        }

        // Max packet size in ddnet is 1400.
        let mut recvbuf = [0; 1400];

        let res = sock.recv(&mut recvbuf)?;

        log::debug!("received {} bytes", res);

        let mut data = &recvbuf[..];

        // Skip padding
        data.advance(10);

        let svtype_raw = data.get_i32().to_be_bytes();
        let svtype = std::str::from_utf8(&svtype_raw)?;

        log::debug!("type value: {}", svtype);
        let is_modern = svtype.eq("iext");

        // Data is separated by a null.
        let mut split: Vec<&[u8]> = data.split(|x| *x == 0x00).collect();
        let mut data = split.iter_mut();

        // Token
        let mixed_token_recv = {
            let num_str = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;
            num_str.parse::<i32>()?
        };

        let extra_token_recv = ((mixed_token_recv >> 8) & 0xffff) as u16;
        let token_recv = (mixed_token_recv & 0xff) as u8;

        log::debug!("mixed token received: {}", mixed_token_recv);
        log::debug!("token received: {} == {}", token, token_recv);
        log::debug!("extra token received: {} == {}", extra_token, extra_token_recv);

        if is_modern && extra_token != extra_token_recv || token != token_recv {
            return Err(RequestError::TokenError {
                wanted_extra_token: extra_token,
                wanted_token: token,
                received_extra_token: extra_token_recv,
                received_token: token_recv,
            });
        }

        let version = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;

        log::debug!("version: {}", version);

        let name = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;

        log::debug!("name: {}", name);

        let map = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;

        log::debug!("map: {}", map);

        let mut map_crc = "";
        let mut map_size = "";

        if is_modern {
            map_crc = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;
            log::debug!("map_crc: {}", map_crc);

            map_size = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;
            log::debug!("map_size: {}", map_size);
        }

        let game_type = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;

        log::debug!("game_type: {}", game_type);

        let flags = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;

        log::debug!("flags: {}", flags);

        let num_players = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;

        log::debug!("num_players: {}", num_players);

        let max_players = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;
        let num_players = num_players.parse::<i32>()?;

        log::debug!("max_players: {}", max_players);

        let num_clients = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;
        let num_clients = num_clients.parse::<i32>()?;

        log::debug!("num_clients: {}", num_clients);

        let max_clients = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;

        log::debug!("max_clients: {}", max_clients);

        let reserved = {
            if is_modern {
                std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?
            } else {
                ""
            }
        };

        log::debug!("reserved: {}", reserved);

        let mut players = vec![];

        for _ in 0..num_clients {
            if let Ok(player) = get_player(&mut data, is_modern) {
                log::debug!("player loaded: {:?}", player);
                players.push(player);
            } else {
                break;
            }
        }

        let mut more_packets = HashSet::new();

        // Main packet num is 0.
        more_packets.insert(0);

        // process "more" packets
        // max 6 tries to avoid endless loops
        for _ in 0..6 {
            // Only check for more packets if the current one is really filled up.
            log::debug!(
                "num_clients {:?}, players len {:?}",
                num_clients,
                players.len()
            );

            if players.len() < num_clients as usize {
                let mut recvbuf = [0; 1400];

                log::debug!("receiving a 'more' packet");
                if let Ok(res) = sock.recv(&mut recvbuf) {
                    log::debug!("recv size: {:?}", res);

                    if res > 0 {
                        log::debug!("received more packets {:?}", res);
                        let mut data = &recvbuf[..];

                        // Skip padding
                        data.advance(10);

                        let more_type_raw = data.get_i32().to_be_bytes();
                        let more_type = std::str::from_utf8(&more_type_raw)?;

                        if !more_type.eq("iex+") {
                            log::warn!(
                                "'more' packet type field should match 'iex+' but it is '{:?}'",
                                svtype
                            );
                        }

                        log::debug!("type value: {}", more_type);

                        // Data is separated by a null.
                        let mut split: Vec<&[u8]> = data.split(|x| *x == 0x00).collect();
                        let mut data = split.iter_mut();

                        // Token
                        let mixed_token_recv = {
                            let num_str = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;
                            num_str.parse::<i32>()?
                        };

                        let extra_token_recv = ((mixed_token_recv >> 8) & 0xffff) as u16;
                        let token_recv = (mixed_token_recv & 0xff) as u8;

                        log::debug!("mixed token received: {}", mixed_token_recv);
                        log::debug!("token received: {} == {}", token, token_recv);
                        log::debug!("extra token received: {} == {}", extra_token, extra_token_recv);

                        if extra_token != extra_token_recv || token != token_recv {
                            return Err(RequestError::TokenError {
                                wanted_extra_token: extra_token,
                                wanted_token: token,
                                received_extra_token: extra_token_recv,
                                received_token: token_recv,
                            });
                        }


                        let packet_no = {
                            let num_str = std::str::from_utf8(data.next().ok_or(RequestError::Missing)?)?;
                            num_str.parse::<i32>()?
                        };

                        if !more_packets.insert(packet_no) {
                            log::warn!("server sent a repeated packet: {}", packet_no);
                            continue;
                        }

                        log::debug!("packet number: {}", packet_no);

                        // reserved
                        data.next();

                        let len = players.len() as usize;
                        for _ in 0..(num_clients as usize - len) {
                            if let Ok(player) = get_player(&mut data, is_modern) {
                                log::debug!("player loaded: {:?}", player);
                                players.push(player);
                            } else {
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }
        }

        Ok(ServerInfo {
            version: version.to_string(),
            name: name.to_string(),
            map: map.to_string(),
            password: flags.eq("1"),
            game_type: game_type.to_string(),
            player_count: num_players,
            max_player_count: max_players.parse()?,
            client_count: num_clients,
            max_client_count: max_clients.parse()?,
            mapcrc: map_crc.parse().unwrap_or_else(|_| 0),
            mapsize: map_size.parse().unwrap_or_else(|_| 0),
            players,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use std::net::UdpSocket;
    use std::time::Duration;

    #[test]
    fn it_works() {
        env_logger::init();
        let sock = UdpSocket::bind("0.0.0.0:0").expect("can't bind socket");
        sock.set_write_timeout(Some(Duration::from_millis(400)))
            .unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(400)))
            .unwrap();
        sock.connect("94.237.94.154:8309")
            .expect("can't connect socket");
        ServerInfo::new(&sock).unwrap();
    }
}

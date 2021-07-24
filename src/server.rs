use log::debug;
use nom::sequence::tuple;
use nom::IResult;
use nom::{
    char, cond, do_parse, many_till, map_res, named, tag, take, take_str, take_until,
    terminated,
};
use std::net::UdpSocket;

use crate::errors::*;
use crate::util::*;

/// Player info.
#[derive(Debug)]
pub struct Player<'a> {
    pub name: &'a str,
    pub clan: &'a str,
    pub country: i32,
    pub score: i32,
    pub is_spectator: bool,
    pub reserved: &'a str,
}

#[derive(Debug)]
pub struct ServerInfo<'a> {
    pub version: &'a str,
    pub token: i32,
    pub name: &'a str,
    pub map: &'a str,
    pub password: bool,
    pub game_type: &'a str,
    pub player_count: i32,
    pub max_player_count: i32,
    pub client_count: i32,
    pub max_client_count: i32,
    pub map_crc: Option<i32>,
    pub map_size: Option<i32>,
    pub players: Vec<Player<'a>>,
    pub buffers: Vec<Vec<u8>>,
}

named!(padding, take!(10));
named!(response_type<&str>, take_str!(4));

named!(next_data, terminated!(take_until!("\0"), char!('\0')));

named!(
    next_str<&str>,
    map_res!(next_data, |x| std::str::from_utf8(x))
);

named!(
    next_int<i32>,
    map_res!(next_str, |s: &str| s.parse::<i32>())
);

named!(
    server_info<ServerInfo>,
    do_parse!(
        _padd: padding
            >> resp_type: response_type
            >> token: next_int
            >> version: next_str
            >> name: next_str
            >> map: next_str
            >> map_crc: cond!(resp_type == "iext", next_int)
            >> map_size: cond!(resp_type == "iext", next_int)
            >> game_type: next_str
            >> flags: next_int
            >> num_players: next_int
            >> max_players: next_int
            >> num_clients: next_int
            >> max_clients: next_int
            >> _reserved: next_str
            >> (ServerInfo {
                token,
                version,
                name,
                map,
                map_crc,
                map_size,
                player_count: num_players,
                max_player_count: max_players,
                client_count: num_clients,
                max_client_count: max_clients,
                password: (flags & 1) == 1,
                game_type,
                players: Vec::new(),
                buffers: Vec::new()
            })
    )
);

named!(read_players<&[u8], (Vec<Player>, &[u8])>, many_till!(get_player, tag!("\0\0")));

fn get_player(i: &[u8]) -> IResult<&[u8], Player> {
    let (input, (name, clan, country, score, is_player, reserved)) =
        tuple((next_str, next_str, next_int, next_int, next_int, next_str))(i)?;
    IResult::Ok((
        input,
        Player {
            name,
            clan,
            country,
            score,
            is_spectator: is_player != 1,
            reserved,
        },
    ))
}

impl<'a> ServerInfo<'a> {
    /// Parses the main packet.
    fn parse_main(data: &'a [u8]) -> Result<ServerInfo<'a>> {
        let (input, mut server_info) = server_info(data).unwrap();

        if server_info.client_count > 0 {
            let (_input, (ps, _)) = read_players(input).unwrap();
            server_info.players.extend(ps);
        }

        Ok(server_info)
    }

    /// Parses the more packet.
    fn parse_more(&mut self, data: &'a [u8]) {
        let (input, _) =
            tuple((padding, response_type, next_int, next_int, next_str))(data).unwrap();

        let (_, (more_players, _)) = read_players(input).unwrap();
        self.players.extend(more_players);
    }

    /// Creates the necessary buffers that you need to hold and use to get the server info.
    pub fn create_buffers() -> Vec<Vec<u8>> {
        let mut buffers = Vec::new();
        // Main buffer
        let main_vec = vec![0; 1400];
        buffers.push(main_vec);
        // More packet buffer
        let more_vec = vec![0; 1400];
        buffers.push(more_vec);

        buffers
    }

    /// The socket must be already connected.
    /// Using the provided buffers to hold the response,
    /// this function parses the data received doing zero copy into a [ServerInfo].
    ///
    /// See also [ServerInfo::create_buffers()]
    pub fn new(sock: &UdpSocket, buffers: &'a mut [Vec<u8>]) -> Result<ServerInfo<'a>> {
        let (buf, extra_token, token) = create_packet(PacketType::GetInfo, Some(b"xe"), true);
        let token = token.expect("token should always have value here.");

        log::debug!("generated extra_token={}, token={}", extra_token, token);

        // TODO: Use a single buffer with split_mut_at and use the recv value.

        let sent = sock.send(&buf)?;

        log::debug!("sent {} bytes", sent);
        if sent != buf.len() {
            log::warn!(
                "bytes sent ({}) not equal to buffer size ({})!",
                sent,
                buf.len()
            );
        }

        let iter = &mut buffers.iter_mut();

        if let Some(data) = iter.next() {
            let res = sock.recv(data)?;

            log::debug!("received {} packets", res);
            let mut info = ServerInfo::parse_main(data).unwrap();

            debug!(
                "Players parsed={} total_players={}",
                info.players.len(),
                info.max_client_count
            );

            if info.players.len() < info.client_count as usize {
                for more_data in iter {
                    let res = sock.recv(more_data)?;
                    if res > 0 {
                        info.parse_more(more_data);
                    }
                    debug!(
                        "Players parsed={} total_players={}",
                        info.players.len(),
                        info.client_count
                    );
                }
            }

            Ok(info)
        } else {
            unimplemented!()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn it_works() {
        let data = include_bytes!("samples/server_info.data");
        let data_more = include_bytes!("samples/server_info_more.data");
        let mut info = ServerInfo::parse_main(data).unwrap();
        info.parse_more(data_more);

        assert_eq!(info.client_count, 63);
        assert_eq!(info.game_type, "DDraceNetwork");
        assert_eq!(info.max_player_count, 63);
        assert_eq!(info.max_client_count, 63);
        assert_eq!(info.map, "Multeasymap");
        assert_eq!(info.players.len(), info.client_count as usize);
        assert_eq!(
            info.players.iter().filter(|x| !x.is_spectator).count(),
            info.player_count as usize
        );
    }

    /*
    #[test]
    fn it_works_2() {
        use std::time::Duration;
        env_logger::init();
        let sock = UdpSocket::bind("0.0.0.0:0").expect("can't bind socket");
        sock.set_write_timeout(Some(Duration::from_millis(400)))
            .unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(400)))
            .unwrap();
        sock.connect("130.61.123.168:8341")
            .expect("can't connect socket");
        let mut buffers = ServerInfo::create_buffers();
        ServerInfo::new(&sock, &mut buffers).unwrap();
    }
    */
}

use nom::sequence::tuple;
use nom::IResult;
use nom::{
    char, cond, do_parse, many_till, map_res, named, tag, take, take_str, take_until, terminated,
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
    pub is_player: bool,
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

#[rustfmt::skip]
named!(server_info<(
        i32, &str, &str, &str, 
        Option<i32>, Option<i32>,
        i32, i32, i32, i32, i32, &str)>,
    do_parse!(
        padd: padding >>
        resp_type: response_type >>
        token: next_int >>
        version: next_str >>
        name: next_str >>
        map: next_str >>
        map_crc: cond!(resp_type == "iext", next_int) >>
        map_size: cond!(resp_type == "iext", next_int) >>
        game_type: next_str >>
        flags: next_int >>
        num_players: next_int >>
        max_players: next_int >>
        num_clients: next_int >>
        max_clients: next_int >>
        reserved: next_str >>
        (token, version, name, map, map_crc, map_size, num_players, max_players, num_clients, max_clients, flags, game_type)
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
            is_player: is_player == 1,
            reserved,
        },
    ))
}

impl<'a> ServerInfo<'a> {
    fn parse_main<S: AsRef<[u8]>>(data: &'a S) -> Result<ServerInfo<'a>> {
        let (_input, info) = server_info(data.as_ref()).unwrap();

        let mut server_info = ServerInfo {
            token: info.0,
            version: info.1,
            name: info.2,
            map: info.3,
            map_crc: info.4,
            map_size: info.5,
            player_count: info.6,
            max_player_count: info.7,
            client_count: info.8,
            max_client_count: info.9,
            password: (info.10 & 1) == 1,
            game_type: info.11,
            players: Vec::new(),
        };

        let (_input, (ps, _)) = read_players(_input).unwrap();

        server_info.players.extend(ps);

        Ok(server_info)
    }

    fn parse_more<S: AsRef<[u8]>>(&mut self, data: &'a S) {
        let (input, _) =
            tuple((padding, response_type, next_int, next_int, next_str))(data.as_ref()).unwrap();

        let (_, (more_players, _)) = read_players(input).unwrap();
        self.players.extend(more_players);
    }

    pub fn create_buffers() -> Vec<Vec<u8>> {
        let mut buffers = Vec::new();
        // Main buffer
        buffers.push(Vec::with_capacity(1400));
        // More packet buffer
        buffers.push(Vec::with_capacity(1400));

        buffers
    }

    pub fn new_v2(sock: &UdpSocket, buffers: &'a mut Vec<Vec<u8>>) -> Result<ServerInfo<'a>> {
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

        let ref mut iter = buffers.iter_mut();

        if let Some(data) = iter.next() {
            let res = sock.recv(data)?;
            log::debug!("received {} packets", res);
            let mut info = ServerInfo::parse_main(data).unwrap();

            if (info.client_count as usize) < info.players.len() {
                while let Some(more_data) = iter.next() {
                    let res = sock.recv(more_data)?;
                    if res > 0 {
                        info.parse_more(more_data);
                    }
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

    #[test]
    fn it_works() {
        env_logger::init();
        let data = include_bytes!("server_response.data");
        let info = ServerInfo::parse_main(&data).unwrap();
        log::info!("{:#?}", info);
    }
}

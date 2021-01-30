use bytes::{BufMut, BytesMut};
use rand::Rng;

#[derive(Debug, PartialEq, Eq)]
pub enum PacketType {
    // Packets sent.
    GetCount,
    GetList,
    GetInfo,
    GetInfo64Legacy,
    // Packets received
    Count,
    List,
    Info,
    Info64Legacy,
    InfoExtended,
    InfoExtendedMore,
}

impl PacketType {
    pub fn value(&self) -> &'static [u8] {
        match self {
            PacketType::GetCount => b"cou2",
            PacketType::GetList => b"req2",
            PacketType::GetInfo => b"gie3",
            PacketType::GetInfo64Legacy => b"fstd",
            PacketType::Info => b"inf3",
            PacketType::Count => b"siz2",
            PacketType::List => b"lis2",
            PacketType::Info64Legacy => b"dtsf",
            PacketType::InfoExtended => b"iext",
            PacketType::InfoExtendedMore => b"iex+",
        }
    }
}

impl std::cmp::PartialEq<[u8]> for PacketType {
    fn eq(&self, other: &[u8]) -> bool {
        *other == *self.value()
    }
}

impl std::cmp::PartialEq<PacketType> for [u8] {
    fn eq(&self, other: &PacketType) -> bool {
        *self == *other.value()
    }
}

pub fn create_packet(packet: PacketType, magic_bytes: Option<&[u8]>, add_token: bool) -> (BytesMut, u16, Option<u8>) {
    let mut buf = BytesMut::new();
    let mut rng = rand::thread_rng();
    let extra_token = rng.gen::<u16>();
    if let Some(magic_bytes) = magic_bytes {
        buf.put(&magic_bytes[..]);
    }
    buf.put_u16(extra_token); // extra token
    // reserved
    buf.put_u8(0x0);
    buf.put_u8(0x0);
    // padding
    buf.put_u8(0xff);
    buf.put_u8(0xff);
    buf.put_u8(0xff);
    buf.put_u8(0xff);
    buf.put(&packet.value()[..]); // vanilla request
    let mut token = None;
    if add_token {
        let val = rng.gen::<u8>();
        buf.put_u8(val);
        token = Some(val);
    }
    (buf, extra_token, token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::{BigEndian, ReadBytesExt};

    #[test]
    fn packet_compares() {
        let a = &b"inf3"[..];
        assert_eq!(PacketType::Info, a[..]);
        assert_eq!(a[..], PacketType::Info);
        assert_ne!(PacketType::GetInfo, a[..]);
        assert_ne!(PacketType::Info64Legacy, a[..]);

        let a = &b"iext"[..];
        assert_eq!(PacketType::InfoExtended, a[..]);
        assert_ne!(PacketType::Info64Legacy, a[..]);

        let a = &b"iex+"[..];
        assert_eq!(PacketType::InfoExtendedMore, a[..]);
        assert_ne!(PacketType::InfoExtended, a[..]);

        let a = &b"dtsf"[..];
        assert_eq!(PacketType::Info64Legacy, a[..]);
        assert_ne!(PacketType::Info, a[..]);
    }

    #[test]
    fn packet_creates() {
        let (buf, extra_token, token) = create_packet(PacketType::GetInfo, Some(b"xe"), true);
        assert_eq!(&buf[0..2], b"xe");

        let mut rdr = &buf[2..4];
        assert_eq!(rdr.read_u16::<BigEndian>().unwrap(), extra_token);

        assert_eq!(buf[4], 0);
        assert_eq!(buf[5], 0);

        for x in 6..10 {
            assert_eq!(buf[x], 0xff);
        }

        assert_eq!(PacketType::GetInfo, buf[10..14]);
        assert_eq!(buf[14], token.unwrap());
    }

    #[test]
    fn packet_creates2() {
        let (buf, extra_token, _) = create_packet(PacketType::GetInfo, None, false);
        let mut rdr = &buf[0..2];
        assert_eq!(rdr.read_u16::<BigEndian>().unwrap(), extra_token);

        assert_eq!(buf[2], 0);
        assert_eq!(buf[3], 0);

        for x in 4..8 {
            assert_eq!(buf[x], 0xff);
        }

        assert_eq!(PacketType::GetInfo, buf[8..12]);
    }
}

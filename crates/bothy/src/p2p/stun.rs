//! A minimal STUN client (RFC 5389 Binding) to learn the server-reflexive address of the P2P
//! socket. The STUN server only sees that address; nothing about the database.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

const BINDING_REQUEST: u16 = 0x0001;
const BINDING_SUCCESS: u16 = 0x0101;
const MAGIC_COOKIE: u32 = 0x2112_a442;
const XOR_MAPPED_ADDRESS: u16 = 0x0020;
const MAPPED_ADDRESS: u16 = 0x0001;

/// A Binding request with this transaction id.
pub(crate) fn request(tid: [u8; 12]) -> Vec<u8> {
    let mut b = Vec::with_capacity(20);
    b.extend_from_slice(&BINDING_REQUEST.to_be_bytes());
    b.extend_from_slice(&0u16.to_be_bytes());
    b.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
    b.extend_from_slice(&tid);
    b
}

/// Whether `data` looks like a STUN message carrying our magic cookie and `tid`.
pub(crate) fn is_response_to(data: &[u8], tid: &[u8; 12]) -> bool {
    data.len() >= 20 && data[4..8] == MAGIC_COOKIE.to_be_bytes() && data[8..20] == tid[..]
}

/// The mapped address of a Binding success response to `tid`.
pub(crate) fn mapped_address(data: &[u8], tid: &[u8; 12]) -> Option<SocketAddr> {
    if !is_response_to(data, tid) || u16::from_be_bytes([data[0], data[1]]) != BINDING_SUCCESS {
        return None;
    }
    let len = usize::from(u16::from_be_bytes([data[2], data[3]]));
    let attrs = data.get(20..20 + len)?;
    let mut fallback = None;
    let mut i = 0;
    while i + 4 <= attrs.len() {
        let ty = u16::from_be_bytes([attrs[i], attrs[i + 1]]);
        let alen = usize::from(u16::from_be_bytes([attrs[i + 2], attrs[i + 3]]));
        let value = attrs.get(i + 4..i + 4 + alen)?;
        match ty {
            XOR_MAPPED_ADDRESS => return address(value, Some(tid)),
            MAPPED_ADDRESS => fallback = address(value, None),
            _ => {}
        }
        i += 4 + alen.div_ceil(4) * 4;
    }
    fallback
}

fn address(v: &[u8], xor: Option<&[u8; 12]>) -> Option<SocketAddr> {
    if v.len() < 4 {
        return None;
    }
    let cookie = MAGIC_COOKIE.to_be_bytes();
    let mut port = u16::from_be_bytes([v[2], v[3]]);
    if xor.is_some() {
        port ^= u16::from_be_bytes([cookie[0], cookie[1]]);
    }
    let ip = match v[1] {
        0x01 => {
            let mut o: [u8; 4] = v.get(4..8)?.try_into().ok()?;
            if xor.is_some() {
                for (b, k) in o.iter_mut().zip(cookie) {
                    *b ^= k;
                }
            }
            IpAddr::V4(Ipv4Addr::from(o))
        }
        0x02 => {
            let mut o: [u8; 16] = v.get(4..20)?.try_into().ok()?;
            if let Some(tid) = xor {
                let key: Vec<u8> = cookie.iter().chain(tid.iter()).copied().collect();
                for (b, k) in o.iter_mut().zip(key) {
                    *b ^= k;
                }
            }
            IpAddr::V6(Ipv6Addr::from(o))
        }
        _ => return None,
    };
    Some(SocketAddr::new(ip, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a Binding success response with one XOR-MAPPED-ADDRESS.
    fn response(tid: [u8; 12], addr: SocketAddr) -> Vec<u8> {
        let cookie = MAGIC_COOKIE.to_be_bytes();
        let SocketAddr::V4(a) = addr else {
            unreachable!()
        };
        let mut attr = vec![0, 0x01];
        attr.extend_from_slice(&(a.port() ^ 0x2112).to_be_bytes());
        attr.extend(a.ip().octets().iter().zip(cookie).map(|(b, k)| b ^ k));
        let mut b = Vec::new();
        b.extend_from_slice(&BINDING_SUCCESS.to_be_bytes());
        b.extend_from_slice(&(4 + attr.len() as u16).to_be_bytes());
        b.extend_from_slice(&cookie);
        b.extend_from_slice(&tid);
        b.extend_from_slice(&XOR_MAPPED_ADDRESS.to_be_bytes());
        b.extend_from_slice(&(attr.len() as u16).to_be_bytes());
        b.extend_from_slice(&attr);
        b
    }

    #[test]
    fn request_and_response() {
        let tid = [7; 12];
        let req = request(tid);
        assert_eq!(req.len(), 20);
        assert_eq!(&req[..2], &[0, 1]);
        let addr: SocketAddr = "203.0.113.9:54321".parse().unwrap();
        let resp = response(tid, addr);
        assert!(is_response_to(&resp, &tid));
        assert_eq!(mapped_address(&resp, &tid), Some(addr));
        assert_eq!(mapped_address(&resp, &[8; 12]), None);
        assert_eq!(mapped_address(&resp[..21], &tid), None);
    }
}

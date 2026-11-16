//! Shared address admission for pinned, untrusted public HTTP targets.
use std::net::IpAddr;

pub(crate) fn allowed_ip(ip: IpAddr, private: bool) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, _, _] = ip.octets();
            if a == 0
                || a >= 224
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (18..=19).contains(&b))
                || (a == 192 && b == 0)
            {
                return false;
            }
            private || !(ip.is_private() || ip.is_loopback())
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return allowed_ip(IpAddr::V4(v4), private);
            }
            if ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_unicast_link_local()
                || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0xdb8)
                || ip.segments()[0] == 0x2002
                || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0)
                || ip.segments()[0] & 0xffc0 == 0xfec0
            {
                return false;
            }
            if ip.is_loopback() || ip.is_unique_local() {
                return private;
            }
            ip.segments()[0] & 0xe000 == 0x2000
        }
    }
}

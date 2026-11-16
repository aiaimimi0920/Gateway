//! Ask the OS for assigned interface addresses; do not infer ownership from private CIDRs or DNS.
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[cfg(windows)]
pub(super) fn contains(ip: IpAddr) -> bool {
    use windows_sys::Win32::{
        NetworkManagement::IpHelper::{
            FreeMibTable, GetUnicastIpAddressTable, MIB_UNICASTIPADDRESS_TABLE,
        },
        Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC},
    };
    struct Table(*mut MIB_UNICASTIPADDRESS_TABLE);
    impl Drop for Table {
        fn drop(&mut self) {
            unsafe {
                FreeMibTable(self.0.cast());
            }
        }
    }
    let mut table = std::ptr::null_mut();
    // The API allocates the complete variable-length table; the guard frees it on every return.
    unsafe {
        if GetUnicastIpAddressTable(AF_UNSPEC, &mut table) != 0 || table.is_null() {
            return false;
        }
        let table = Table(table);
        let rows =
            std::slice::from_raw_parts((*table.0).Table.as_ptr(), (*table.0).NumEntries as usize);
        rows.iter().any(|row| match row.Address.si_family {
            AF_INET => {
                IpAddr::V4(Ipv4Addr::from(
                    row.Address.Ipv4.sin_addr.S_un.S_addr.to_ne_bytes(),
                )) == ip
            }
            AF_INET6 => IpAddr::V6(Ipv6Addr::from(row.Address.Ipv6.sin6_addr.u.Byte)) == ip,
            _ => false,
        })
    }
}

#[cfg(unix)]
pub(super) fn contains(ip: IpAddr) -> bool {
    struct Interfaces(*mut libc::ifaddrs);
    impl Drop for Interfaces {
        fn drop(&mut self) {
            unsafe {
                libc::freeifaddrs(self.0);
            }
        }
    }
    let mut head = std::ptr::null_mut();
    // getifaddrs owns this trusted OS list; inspect the family before casting each address.
    unsafe {
        if libc::getifaddrs(&mut head) != 0 {
            return false;
        }
        let _guard = Interfaces(head);
        let mut cursor = head;
        while let Some(entry) = cursor.as_ref() {
            if let Some(address) = entry.ifa_addr.as_ref() {
                let matches = match address.sa_family as i32 {
                    libc::AF_INET => {
                        IpAddr::V4(Ipv4Addr::from(
                            (*(entry.ifa_addr as *const libc::sockaddr_in))
                                .sin_addr
                                .s_addr
                                .to_ne_bytes(),
                        )) == ip
                    }
                    libc::AF_INET6 => {
                        IpAddr::V6(Ipv6Addr::from(
                            (*(entry.ifa_addr as *const libc::sockaddr_in6))
                                .sin6_addr
                                .s6_addr,
                        )) == ip
                    }
                    _ => false,
                };
                if matches {
                    return true;
                }
            }
            cursor = entry.ifa_next;
        }
    }
    false
}

#[cfg(not(any(windows, unix)))]
pub(super) fn contains(_ip: IpAddr) -> bool {
    false
}

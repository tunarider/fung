//! Raw TCP sockets for `_native.net` (stage 4d): sockets made the way Go's
//! `net` package makes them (non-blocking, close-on-exec, no `SIGPIPE`),
//! a `connect` that can be waited for (`poll` through a `Watch`), a
//! listening socket whose failing system call is known (Go names it in
//! its message: `bind: address already in use`), and name lookup with
//! `getaddrinfo` (Go's cgo resolver), keeping its error code.
//!
//! `std::net` has no non-blocking `connect` and does not say which call
//! of `bind` failed, so these few calls go through `libc`. Unix only,
//! like the rest of the native layer.

use std::ffi::{CStr, CString};
use std::io;
use std::mem;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use super::oserr;

/// A new TCP socket for `addr`'s family: non-blocking, closed on exec,
/// and (macOS) never raising `SIGPIPE` on a write to a closed peer
/// (Linux's `send` gets `MSG_NOSIGNAL` from the standard library).
fn socket(addr: &SocketAddr) -> io::Result<OwnedFd> {
    let family = match addr {
        SocketAddr::V4(_) => libc::AF_INET,
        SocketAddr::V6(_) => libc::AF_INET6,
    };
    #[cfg(target_os = "linux")]
    let ty = libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;
    #[cfg(not(target_os = "linux"))]
    let ty = libc::SOCK_STREAM;
    // SAFETY: plain system call; the result is checked before use.
    let fd = unsafe { libc::socket(family, ty, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `fd` was just returned by `socket` and is owned by no one else.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    #[cfg(not(target_os = "linux"))]
    {
        // SAFETY: `fd` is an open descriptor; fcntl takes no pointers here.
        unsafe {
            if libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) < 0 {
                return Err(io::Error::last_os_error());
            }
            let flags = libc::fcntl(fd.as_raw_fd(), libc::F_GETFL);
            if flags < 0 || libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) < 0
            {
                return Err(io::Error::last_os_error());
            }
        }
    }
    no_sigpipe(&fd)?;
    Ok(fd)
}

/// macOS: `SO_NOSIGPIPE`, so a write to a closed peer fails with `EPIPE`
/// instead of killing the process. Elsewhere nothing.
pub(crate) fn no_sigpipe(fd: &impl AsRawFd) -> io::Result<()> {
    #[cfg(target_vendor = "apple")]
    set_int(fd, libc::SOL_SOCKET, libc::SO_NOSIGPIPE, 1)?;
    let _ = fd;
    Ok(())
}

/// `setsockopt` with an int value.
fn set_int(
    fd: &impl AsRawFd,
    level: libc::c_int,
    name: libc::c_int,
    v: libc::c_int,
) -> io::Result<()> {
    // SAFETY: `fd` is open; the option value is a live c_int of the given size.
    let rc = unsafe {
        libc::setsockopt(
            fd.as_raw_fd(),
            level,
            name,
            (&v as *const libc::c_int).cast(),
            mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// `addr` as a C socket address.
fn raw(addr: &SocketAddr) -> (libc::sockaddr_storage, libc::socklen_t) {
    // SAFETY: all-zero bytes are a valid `sockaddr_storage` (plain integers).
    let mut st: libc::sockaddr_storage = unsafe { mem::zeroed() };
    let len = match addr {
        SocketAddr::V4(a) => {
            // SAFETY: as above, for `sockaddr_in`.
            let mut sin: libc::sockaddr_in = unsafe { mem::zeroed() };
            sin.sin_family = libc::AF_INET as libc::sa_family_t;
            sin.sin_port = a.port().to_be();
            sin.sin_addr = libc::in_addr {
                s_addr: u32::from_ne_bytes(a.ip().octets()),
            };
            #[cfg(target_vendor = "apple")]
            {
                sin.sin_len = mem::size_of::<libc::sockaddr_in>() as u8;
            }
            // SAFETY: `sockaddr_storage` is larger than `sockaddr_in` and
            // aligned for every socket address type.
            unsafe { std::ptr::write((&mut st as *mut libc::sockaddr_storage).cast(), sin) };
            mem::size_of::<libc::sockaddr_in>()
        }
        SocketAddr::V6(a) => {
            // SAFETY: as above, for `sockaddr_in6`.
            let mut sin6: libc::sockaddr_in6 = unsafe { mem::zeroed() };
            sin6.sin6_family = libc::AF_INET6 as libc::sa_family_t;
            sin6.sin6_port = a.port().to_be();
            sin6.sin6_flowinfo = a.flowinfo();
            sin6.sin6_addr = libc::in6_addr {
                s6_addr: a.ip().octets(),
            };
            sin6.sin6_scope_id = a.scope_id();
            #[cfg(target_vendor = "apple")]
            {
                sin6.sin6_len = mem::size_of::<libc::sockaddr_in6>() as u8;
            }
            // SAFETY: as for `sockaddr_in`.
            unsafe { std::ptr::write((&mut st as *mut libc::sockaddr_storage).cast(), sin6) };
            mem::size_of::<libc::sockaddr_in6>()
        }
    };
    (st, len as libc::socklen_t)
}

/// Starts connecting to `addr`: the socket, and whether it is connected
/// already (otherwise it becomes writable when the attempt ends; then
/// `finish_connect` tells how). The error names the failing call
/// (`socket` or `connect`), as Go's message does.
pub(crate) fn start_connect(
    addr: &SocketAddr,
) -> Result<(TcpStream, bool), (&'static str, io::Error)> {
    let fd = socket(addr).map_err(|e| ("socket", e))?;
    let (st, len) = raw(addr);
    // SAFETY: `fd` is open and `st`/`len` describe a valid address.
    let rc = unsafe {
        libc::connect(
            fd.as_raw_fd(),
            (&st as *const libc::sockaddr_storage).cast(),
            len,
        )
    };
    let s = TcpStream::from(fd);
    if rc == 0 {
        return Ok((s, true));
    }
    let e = io::Error::last_os_error();
    match e.raw_os_error() {
        // A non-blocking connect goes on in the background (also after
        // an interrupted call).
        Some(libc::EINPROGRESS | libc::EINTR) => Ok((s, false)),
        _ => Err(("connect", e)),
    }
}

/// How a connect `start_connect` left running ended (`SO_ERROR`).
pub(crate) fn finish_connect(s: &TcpStream) -> io::Result<()> {
    match s.take_error()? {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// A listening socket on `addr` with `SO_REUSEADDR` (as Go and the
/// standard library set it), the system's largest backlog (Go reads the
/// system limit; the kernel lowers a larger value to it). The error names
/// the failing call.
pub(crate) fn listen(addr: &SocketAddr) -> Result<TcpListener, (&'static str, io::Error)> {
    let fd = socket(addr).map_err(|e| ("socket", e))?;
    set_int(&fd, libc::SOL_SOCKET, libc::SO_REUSEADDR, 1).map_err(|e| ("setsockopt", e))?;
    let (st, len) = raw(addr);
    // SAFETY: `fd` is open and `st`/`len` describe a valid address.
    let rc = unsafe {
        libc::bind(
            fd.as_raw_fd(),
            (&st as *const libc::sockaddr_storage).cast(),
            len,
        )
    };
    if rc < 0 {
        return Err(("bind", io::Error::last_os_error()));
    }
    // SAFETY: `fd` is an open, bound socket.
    if unsafe { libc::listen(fd.as_raw_fd(), libc::c_int::MAX) } < 0 {
        return Err(("listen", io::Error::last_os_error()));
    }
    Ok(TcpListener::from(fd))
}

/// The addresses `host` names, in the resolver's order (Go's cgo
/// resolver, `getaddrinfo` without flags for any family), or Go's reason
/// for the failure: `no such host`, an errno's text, or the resolver's
/// own text. It blocks: callers run it on a thread of its own.
pub(crate) fn lookup(host: &str) -> Result<Vec<IpAddr>, String> {
    let no_such_host = || "no such host".to_string();
    let c_host = CString::new(host).map_err(|_| no_such_host())?;
    // SAFETY: all-zero bytes are a valid `addrinfo` hints value (null
    // pointers and zero integers).
    let mut hints: libc::addrinfo = unsafe { mem::zeroed() };
    hints.ai_family = libc::AF_UNSPEC;
    hints.ai_socktype = libc::SOCK_STREAM;
    let mut res: *mut libc::addrinfo = std::ptr::null_mut();
    // SAFETY: `c_host` is a NUL-terminated string, `hints` a valid value,
    // `res` a place for the result list, freed below.
    let rc = unsafe { libc::getaddrinfo(c_host.as_ptr(), std::ptr::null(), &hints, &mut res) };
    if rc != 0 {
        return Err(lookup_reason(rc, io::Error::last_os_error()));
    }
    let mut ips = Vec::new();
    let mut p = res;
    while !p.is_null() {
        // SAFETY: `p` is a node of the list `getaddrinfo` returned, not
        // freed yet; `ai_addr` points to an address of `ai_family`'s type.
        unsafe {
            let ai = &*p;
            if !ai.ai_addr.is_null() {
                match ai.ai_family {
                    libc::AF_INET => {
                        let sin = &*(ai.ai_addr as *const libc::sockaddr_in);
                        ips.push(IpAddr::V4(Ipv4Addr::from(
                            sin.sin_addr.s_addr.to_ne_bytes(),
                        )));
                    }
                    libc::AF_INET6 => {
                        let sin6 = &*(ai.ai_addr as *const libc::sockaddr_in6);
                        ips.push(IpAddr::V6(Ipv6Addr::from(sin6.sin6_addr.s6_addr)));
                    }
                    _ => {}
                }
            }
            p = ai.ai_next;
        }
    }
    // SAFETY: `res` is the list `getaddrinfo` returned, freed once.
    unsafe { libc::freeaddrinfo(res) };
    if ips.is_empty() {
        return Err(no_such_host());
    }
    Ok(ips)
}

/// Go's text for a `getaddrinfo` failure (`cgoLookupIPCNAME`): a name
/// that does not exist is `no such host`, a system error its errno's
/// text, anything else the resolver's own message (`gai_strerror`).
fn lookup_reason(rc: libc::c_int, errno: io::Error) -> String {
    #[cfg(target_os = "linux")]
    const NODATA: libc::c_int = -5;
    #[cfg(not(target_os = "linux"))]
    const NODATA: libc::c_int = 7;
    if rc == libc::EAI_NONAME || rc == NODATA {
        return "no such host".into();
    }
    if rc == libc::EAI_SYSTEM
        && let Some(n) = errno.raw_os_error().filter(|&n| n != 0)
    {
        return oserr::errno_text(n);
    }
    // SAFETY: `gai_strerror` returns a static NUL-terminated string.
    unsafe { CStr::from_ptr(libc::gai_strerror(rc)) }
        .to_string_lossy()
        .into_owned()
}

use libc::{c_int, size_t};
use std::ptr;

#[repr(C)]
pub struct QnetConn(*mut htx::api::Conn);

#[repr(C)]
pub struct QnetStream(*mut htx::api::SecureStream);

/// # Safety
/// Callers must ensure `out_client` and `out_server` are valid, non-null pointers to writable QnetConn slots.
#[no_mangle]
pub unsafe extern "C" fn qnet_dial_inproc(out_client: *mut QnetConn, out_server: *mut QnetConn) -> c_int {
    let (c, s) = htx::api::dial_inproc_secure();
    let b1 = Box::new(c);
    let b2 = Box::new(s);
    if !out_client.is_null() { (*out_client).0 = Box::into_raw(b1); }
    if !out_server.is_null() { (*out_server).0 = Box::into_raw(b2); }
    0
}

/// # Safety
/// `conn` must be a valid, non-null pointer to an initialized QnetConn.
#[no_mangle]
pub unsafe extern "C" fn qnet_conn_open_stream(conn: *mut QnetConn) -> QnetStream {
    if conn.is_null() { return QnetStream(ptr::null_mut()); }
    let c = &*((*conn).0);
    let s = c.open_stream();
    QnetStream(Box::into_raw(Box::new(s)))
}

/// # Safety
/// `conn` must be a valid, non-null pointer to an initialized QnetConn.
#[no_mangle]
pub unsafe extern "C" fn qnet_conn_accept_stream(conn: *mut QnetConn, timeout_ms: u64) -> QnetStream {
    if conn.is_null() { return QnetStream(ptr::null_mut()); }
    let c = &*((*conn).0);
    match c.accept_stream(timeout_ms) {
        Some(s) => QnetStream(Box::into_raw(Box::new(s))),
        None => QnetStream(ptr::null_mut()),
    }
}

/// # Safety
/// `st` must be a valid, non-null pointer to a QnetStream. `data` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn qnet_stream_write(st: *mut QnetStream, data: *const u8, len: size_t) -> c_int {
    if st.is_null() || data.is_null() { return -1; }
    let s = &*((*st).0);
    let slice = std::slice::from_raw_parts(data, len);
    s.write(slice);
    0
}

/// # Safety
/// `st` must be valid. `out_buf` must point to `cap` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn qnet_stream_read(st: *mut QnetStream, out_buf: *mut u8, cap: size_t) -> isize {
    if st.is_null() || out_buf.is_null() { return -1; }
    let s = &*((*st).0);
    if let Some(buf) = s.read() {
        let n = buf.len().min(cap);
        ptr::copy_nonoverlapping(buf.as_ptr(), out_buf, n);
        n as isize
    } else { -2 }
}

/// # Safety
/// `conn` must be a valid pointer returned by this library. It will be freed and set to null.
#[no_mangle]
pub unsafe extern "C" fn qnet_conn_free(conn: *mut QnetConn) {
    if conn.is_null() { return; }
    if !(*conn).0.is_null() { let _ = Box::from_raw((*conn).0); (*conn).0 = ptr::null_mut(); }
}

/// # Safety
/// `st` must be a valid pointer returned by this library. It will be freed and set to null.
#[no_mangle]
pub unsafe extern "C" fn qnet_stream_free(st: *mut QnetStream) {
    if st.is_null() { return; }
    if !(*st).0.is_null() { let _ = Box::from_raw((*st).0); (*st).0 = ptr::null_mut(); }
}

//! # Rust-locked localtime implementation
//!
//! **Warning:** this crate is currently proof-of-concept and it wasn't deeply audited!
//! While I believe this fixes the unsoundness I may have introduced other bug(s).
//! Use at your own risk or, better, help improve it!
//!
//! This is a fork of a C `localtime_r` implementation with minimal changes required to make
//! calling it in parallel to setting env **from Rust** sound. It does so by calling into Rust code
//! to get the environment variable instead of using raw system `getenv`.
//!
//! Obviously, this does **not** interact with the system implementation of `localtime_r`.
//! E.g. if you call [`localtime`] in this crate it will not affect static variables in the system
//! library. This is considered a feature because system `localtime` library is a huge dumpster
//! fire and if you use it you can easily get UB - not just from setting env vars. (NetBSD
//! implementation is a bit better though.)
//!
//! This crate is meant to be a cheaper-to-implement alternative to rewriting whole `localtime_r` in
//! Rust which people are unwilling to do due to large code size. It only required changing a few
//! lines and writing glue Rust code.

use std::io;
use libc::time_t;
use libc::c_char;

extern "C" {
    fn rl_localtime_r(sec: *const time_t, out: *mut libc::tm) -> *mut libc::tm;
    fn rl_timegm(tm: *mut libc::tm) -> time_t;
    fn rl_mktime(tm: *mut libc::tm) -> time_t;
}

/// Converts Unix time to calendar time based on current locale.
///
/// This is a **sound** version of `localtime_r` from libc with proper locking.
/// Calling this and concurently setting env **from Rust** using `std::env::set_var` is completely
/// fine. Calling this in parallel is also fine.
pub fn localtime(sec: time_t) -> io::Result<libc::tm> {
    unsafe {
        let mut out = std::mem::zeroed();
        if rl_localtime_r(&sec, &mut out).is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(out)
    }
}

/// Converts calendar time to Unix time using UTC timezone.
///
/// Note that this method is soundly available even on platforms that normally don't have it.
pub fn timegm(mut tm: libc::tm) -> time_t {
    // C functions happily modify the inputs... Garbage everywhere...
    unsafe {
        rl_timegm(&mut tm)
    }
}

/// Converts calendar time to Unix time using local timezone.
///
/// Note that this method is soundly available even on platforms that normally don't have it.
pub fn mktime(mut tm: libc::tm) -> time_t {
    // C functions happily modify the inputs... Garbage everywhere...
    unsafe {
        rl_mktime(&mut tm)
    }
}

/// Efficient C-compatible Option<Cow<OsStr>>
///
/// This type can be sent to C code which can read the string off `ptr` and deallocate it later.
/// As opposed to `OsString` from `std` this type can have null pointer and can have non-zero `len`
/// when `capaity` is zero - that means static string. Also, unless `ptr` is null the string is
/// zero-terminated.
///
/// Nullable pointer is hopefully obvious. The zero capacity trick is to avoid allocation when the
/// string is empty just to add `0` at the end.
#[repr(C)]
struct COsString {
    ptr: *const c_char,
    len: usize,
    capacity: usize,
}

impl COsString {
    /// Creates C-compatible OS string with null pointer (`None` semantics)
    fn null() -> Self {
        COsString {
            ptr: std::ptr::null(),
            len: 0,
            capacity: 0,
        }
    }

    /// Creates empty C-compatible OS string (`""` semantics)
    fn empty() -> Self {
        static EMPTY: c_char = 0;
        COsString {
            ptr: &EMPTY,
            len: 1,
            capacity: 0,
        }
    }

    /// Deallocates the string
    unsafe fn dealloc(self) {
        if self.capacity > 0 {
            Vec::from_raw_parts(self.ptr as *mut u8, self.len, self.capacity);
        }
    }
}

/// Conversion adds 0 at the end.
impl From<std::ffi::OsString> for COsString {
    fn from(value: std::ffi::OsString) -> Self {
        use std::os::unix::ffi::OsStringExt;

        let mut vec = value.into_vec();
        if !vec.is_empty() {
            // add zero terminator
            vec.push(0);

            let ptr = vec.as_mut_ptr();
            let len = vec.len();
            let capacity = vec.capacity();
            std::mem::forget(vec);
            COsString {
                ptr: ptr as *const c_char,
                len,
                capacity,
            }
        } else {
            COsString::empty()
        }
    }
}

/// Conversion adds 0 at the end. `None` is converted to null.
impl From<Option<std::ffi::OsString>> for COsString {
    fn from(value: Option<std::ffi::OsString>) -> Self {
        value
            .map(Into::into)
            .unwrap_or(COsString::null())
    }
}

/// Provides getenv Rust function to C code.
///
/// Rust implements proper locking, so this can be called safely from any thread.
/// The returned value should be deallocated with [`rust_os_string_dealloc`].
///
/// **Important:** despite the value being owned the data behind `ptr` MUST NOT change in C code!
/// `ptr` may point to static memory.
#[no_mangle]
extern "C" fn rust_getenv(name: *const c_char, name_len: usize) -> COsString {
    use std::os::unix::ffi::OsStrExt;
    use std::ffi::OsStr;

    let name = unsafe {
        let name = std::slice::from_raw_parts(name as *const u8, name_len);
        OsStr::from_bytes(name)
    };
    std::env::var_os(name).into()
}

/// Deallocates C-compatible OS string returned from `rust_getenv`.
#[no_mangle]
extern "C" fn rust_os_string_dealloc(string: COsString) {
    unsafe {
        string.dealloc();
    }
}

#[cfg(test)]
mod tests {
    // this is only one test to avoid problems with test parallelism
    #[test]
    fn basic_test() {
        std::env::set_var("TZ", "");
        let time = super::localtime(0).unwrap();
        assert_eq!(time.tm_sec, 0);
        assert_eq!(time.tm_min, 0);
        assert_eq!(time.tm_hour, 0);
        assert_eq!(time.tm_mday, 1);
        assert_eq!(time.tm_mon, 0);
        assert_eq!(time.tm_year, 70);
        assert_eq!(time.tm_yday, 0);
        assert_eq!(time.tm_wday, 4);
        assert_eq!(time.tm_gmtoff, 0);
        assert!(time.tm_isdst < 1);

        let setter_thread = std::thread::spawn(|| {
            for _ in 0..1000000 {
                std::env::set_var("TZ", "");
            }
        });
        for _ in 0..1000000 {
            super::localtime(0).unwrap();
        }
        setter_thread.join().unwrap();

        assert_eq!(super::timegm(time), 0);
    }
}

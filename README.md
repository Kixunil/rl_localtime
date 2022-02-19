# Rust-locked localtime implementation

## Crate abandoned

**This crate is abandoned** because there's a pure Rust implemenation - [`tz-rs`](https://docs.rs/tz-rs/).
The code is up for archival purposes only.

## About

**Warning:** this crate is currently proof-of-concept and it wasn't deeply audited!
While I believe this fixes the unsoundness, I may have introduced other bug(s).
Use at your own risk or, better, help improve it!

This is a fork of a C `localtime_r` implementation with minimal changes required to make
calling it in parallel to setting env **from Rust** sound. It does so by calling into Rust code
to get the environment variable instead of using raw system `getenv`.

Obviously, this does **not** interact with the system implementation of `localtime_r`.
E.g. if you call [`localtime`] in this crate it will not affect static variables in the system
library. This is considered a feature because system `localtime` library is a huge dumpster
fire and if you use it you can easily get UB - not just from setting env vars. (NetBSD
implementation is a bit better though.)

This crate is meant to be a cheaper-to-implement alternative to rewriting whole `localtime_r` in
Rust which people are unwilling to do due to large code size. It only required changing a few
lines and writing glue Rust code.

## License

The C code, including my modifications, is released into public domain.
The Rust code is released under MITNFA license.

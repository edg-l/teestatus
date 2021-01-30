//! # teestatus
//! [![Crates.io](https://meritbadge.herokuapp.com/teestatus)](https://crates.io/crates/teestatus)
//! ![Rust](https://github.com/edg-l/teestatus/workflows/Rust/badge.svg)
//! [![Docs](https://docs.rs/teestatus/badge.svg)](https://docs.rs/teestatus)
//!
//! Request info about teeworlds servers.
//!
//! Example
//! ```rust,no_run
//! use teestatus::*;
//! use std::net::UdpSocket;
//!
//! env_logger::init();
//! let sock = UdpSocket::bind("0.0.0.0:0").expect("can't bind socket");
//! sock.connect("0.0.0.0:8303")
//!     .expect("can't connect socket");
//! println!("info: {:#?}", ServerInfo::new(&sock).unwrap());
//! ```
//! Example to fetch servers from a master server:
//! ```rust,no_run
//! let master = MasterServer {
//! 	hostname: Cow::Borrowed("49.12.97.180"),
//! 	port: 8300,
//! };
//! let sock = UdpSocket::bind("0.0.0.0:0").expect("can't bind socket");
//! let servers = master.get_server_list(&sock).unwrap();
//! ```

pub mod errors;

mod server;
mod masterserver;
mod common;
mod util;

pub use common::*;
pub use server::*;
pub use masterserver::*;

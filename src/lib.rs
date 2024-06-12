#![crate_type = "lib"]
#![forbid(unsafe_code)]
#![forbid(missing_debug_implementations)]
#![forbid(missing_docs)]

//! This is a library for interacting with the ADB server using Tokio.
//!
//! # Examples
//! 
//! Checkout the [AdbServerProxy](proxy::AdbServerProxy) implementation in the proxy module for an example of how to use this library.
//!
//! # Features
//!
//! - Parse and Serailize ADB Packets
//! - Proxy ADB Server Connections
//!
//! # References
//!
//! - [ADB Protocol](https://cs.android.com/android/platform/superproject/main/+/main:packages/modules/adb/protocol.txt)
//!

mod util;
mod adb;
mod proxy;

pub use util::*;
pub use adb::*;
pub use proxy::*;
[![Crates.io](https://img.shields.io/crates/v/adb-server-tokio.svg)](https://crates.io/crates/adb-server-tokio)
[![Docs.rs](https://docs.rs/adb-server-tokio/badge.svg)](https://docs.rs/adb-server-tokio)
[![Build](https://github.com/devicelink/adb-server-tokio/actions/workflows/build.yaml/badge.svg?branch=main)](https://github.com/devicelink/adb-server-tokio/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

# ADB server protocol implementation using Rust and Tokio

## Usage 

Checkout [the proxy](./src/proxy.rs) to understand how to use the AdbServerProtocolConnection.

## Protocol

Details about the protocol can be found in the [Android Source Code](https://cs.android.com/android/platform/superproject/main/+/main:packages/modules/adb/protocol.txt)

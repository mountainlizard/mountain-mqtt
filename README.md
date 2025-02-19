# mountain-mqtt

A `no_std` compatible [MQTT v5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html) client.

## Features

1. Compatible with [`embedded-hal`](https://github.com/rust-embedded/embedded-hal). Provides adapters to use [`embedded-hal-async`](https://crates.io/crates/embedded-hal-async) and [`embedded-io-async`](https://crates.io/crates/embedded-io-async) traits (`Read`, `Write` and `ReadReady`) for network connection, e.g. using [`embassy-net`](https://crates.io/crates/embassy-net) for TCP.
2. Compatible with [`tokio`](https://tokio.rs). Provides adapters to use [`tokio::net::TcpStream`](https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html) `TcpStream`.
3. Layered design to allow reuse in different environments.
4. Fairly thorough tests for `data`, `codec` and `packet` modules against the MQTT v5 specification.

## Todo

1. Support for Quality of Service level 2 in `Client`. The relevant MQTT v5 packets are implemented, but not the state management for handling them in the client.
2. More sophisticated client implementation(s) - the current `Client` implementation `ClientNoQueue` only supports a single pending acknowledgement at a time, and waits for this before returning when sending packets, by polling for data ready. The concurrency model is not ideal, but allows support for embedded and tokio networking with the same relatively simple code. It may be possible to performance and capabilities either by providing separate client implementations for embedded and tokio, or by refactoring the shared concurrency model.
3. Improve and add integration tests for `packet_client` and `client` modules.

## Non-goals

1. MQTT v3 support is not planned.
2. Server support is not planned, but the `data` and `codec` modules support the packets needed for this.

## Layers

1. `data` module - provides basic data types used by MQTT to build packets.
2. `codec` module - provides simple reader and writer traits, and implementations using a `buf: &'a [u8]` and position. `Read` and `Write` traits for data items.
3. `packets` module - provides traits for describing MQTT v5 packets, and a struct for each packet type, with `Read` and `Write` implementations.
4. `packet_client` module - provides a basic low-level client for reading and writing packets directly, using a `Connection` trait with implementations for tokio `TcpStream` and embedded-hal-async `Read + Write + ReadyReady`.
5. `client` module - provides a higher-level basic client that manages connection state, waiting for acknowledgement etc.

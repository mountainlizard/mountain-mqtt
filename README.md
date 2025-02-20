# mountain-mqtt

A `no_std` compatible [MQTT v5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html) client.

Note that this is in very early development. It is functional but not yet stable or feature complete. The API will almost certainly change over time.

## Features

1. Compatible with [`embedded-hal`](https://github.com/rust-embedded/embedded-hal). Provides adapters to use [`embedded-hal-async`](https://crates.io/crates/embedded-hal-async) and [`embedded-io-async`](https://crates.io/crates/embedded-io-async) traits (`Read`, `Write` and `ReadReady`) for network connection, e.g. using [`embassy-net`](https://crates.io/crates/embassy-net) for TCP.
2. Compatible with [`tokio`](https://tokio.rs). Provides adapters to use [`tokio::net::TcpStream`](https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html) `TcpStream`.
3. Layered design to allow reuse in different environments.
4. Fairly thorough tests for `data`, `codec` and `packet` modules against the [MQTT v5 specification](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html).
5. Provides a basic client trait/implementation for connecting, disconnecting, subscribing and unsubscribing, publishing messages and receiving pubished messages from the server. Supports Quality of Service levels 0 and 1.
6. Can run without allocation, using only `core` on `no_std`.

## Adding to your project

There is not yet a published crate, so check out the project sources (alongside the project where you want to use mountain-mqtt) and then reference via path from your `Cargo.toml`.

1. For embedded-hal applications:

   ```toml
   [dependencies]
   mountain-mqtt = { path = "../mountain-mqtt", default-features = false, features = [
   "embedded-io-async",
   "embedded-hal-async",
   ] }
   ```

2. For tokio applications:

   ```toml
   [dependencies]
   mountain-mqtt = { path = "../mountain-mqtt", default-features = false, features = [
   "tokio",
   ] }
   ```

## Todo

1. Support for Quality of Service level 2 in `Client`. The relevant MQTT v5 packets are implemented, but not the state management for handling them in the client.
2. More sophisticated client implementation(s) - the current `Client` implementation `ClientNoQueue` only supports a single pending acknowledgement at a time, and waits for this before returning when sending packets, by polling for data ready. The concurrency model is not ideal, but allows support for embedded and tokio networking with the same relatively simple code and no allocation. A better model should be achievable, maybe using different approaches for tokio (where we can use std) and embedded/no_std.
3. Improve and add integration tests for `packet_client` and `client` modules.
4. Publish as a crate.

## Non-goals

The following goals are not planned, but may be considered later:

1. MQTT v3 support.
2. Server support. Note that the `data` and `codec` modules support the packets needed for this if anyone wants to implement one :)

## Layers

1. `data` module - provides basic data types used in MQTT packets.
2. `codec` module - provides simple reader and writer traits, and implementations using a `buf: &'a [u8]` and position. `Read` and `Write` traits for data items.
3. `packets` module - provides traits for describing MQTT v5 packets, and a struct for each packet type, with `Read` and `Write` implementations.
4. `packet_client` module - provides a basic low-level client for reading and writing packets directly, using a `Connection` trait with implementations for tokio `TcpStream` and embedded-hal-async `Read + Write + ReadyReady`.
5. `client` module - provides a higher-level basic client that manages connection state, waiting for acknowledgement etc.

## Example code

See the `examples` directory for a simple example of using the basic client - try it out with `cargo run --example client_example`:

```rust
use mountain_mqtt::{
    client::{Client, ClientError},
    data::quality_of_service::QualityOfService,
    packets::connect::Connect,
    tokio::client_tcp,
};
use tokio::sync::mpsc;

/// Connect to an MQTT server on 127.0.0.1:1883,
/// server must accept connections with no username or password.
/// Subscribe to a topic, send a message, check we receive it
/// back, then unsubscribe and disconnect.
#[tokio::main]
async fn main() -> Result<(), ClientError> {
    let ip = core::net::Ipv4Addr::new(127, 0, 0, 1);
    let port = 1883;
    let timeout_millis = 5000;
    let mut buf = [0; 1024];

    // We'll use a channel to handle incoming messages, this would allow us to receive
    // them in another task, here we'll just read them back at the end of the example
    let (message_tx, mut message_rx) = mpsc::channel(32);

    // Create a client.
    // The message_handler closure is called whenever a published message is received.
    // This sends copies of the message contents to our channel for later processing.
    let mut client = client_tcp(ip, port, timeout_millis, &mut buf, |message| {
        message_tx
            .try_send((message.topic_name.to_owned(), message.payload.to_vec()))
            .map_err(|_| ClientError::MessageHandlerError)
    })
    .await;

    // Send a Connect packet to connect to the server.
    // `unauthenticated` uses default settings and no username/password, see `Connect::new` for
    // available options (keep alive, will, authentication, additional properties etc.)
    client
        .connect(Connect::unauthenticated("mountain-mqtt-example-client-id"))
        .await?;

    let topic_name = "mountain-mqtt-example-topic";
    let retain = false;

    client.subscribe(topic_name, QualityOfService::QoS0).await?;

    client
        .publish(
            topic_name,
            "Hello MQTT!".as_bytes(),
            QualityOfService::QoS0,
            retain,
        )
        .await?;

    // We are expecting one packet from the server, so just poll once with wait = true.
    // The normal way to use this would be to poll in a loop with wait = false, calling
    // any other required method between polling (e.g. to publish messages, send pings etc.)
    client.poll(true).await?;

    // Check we got the message back
    let (topic, payload) = message_rx.try_recv().unwrap();
    println!(
        "Received from '{}': '{}'",
        topic,
        String::from_utf8_lossy(&payload)
    );

    client.unsubscribe(topic_name).await?;
    client.disconnect().await?;

    Ok(())
}
```

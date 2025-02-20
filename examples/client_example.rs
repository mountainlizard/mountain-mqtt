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

    // Create a client with a message_handler that just sends the messages on to the channel
    let mut client = client_tcp(ip, port, timeout_millis, &mut buf, |topic_name, payload| {
        message_tx
            .try_send((topic_name.to_owned(), payload.to_vec()))
            .map_err(|_| ClientError::MessageHandlerError)
    })
    .await;

    // Send a Connect packet to connect to the server.
    // `unauthenticated` uses default settings and no username/password, see `Connect::new` for
    // available options (keep alive, will, authentication, additional properties etc.)
    client
        .connect(Connect::unauthenticated("mountain-mqtt-example"))
        .await?;

    let topic_name = "mountain-mqtt-example-topic";
    let retain = false;

    // Subscribe to the topic so we will get our messages back
    client
        .subscribe_to_topic(topic_name, &QualityOfService::QoS0)
        .await?;

    // Publish a message
    client
        .send_message(
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
    println!("{}: {}", topic, String::from_utf8_lossy(&payload));

    client.unsubscribe_from_topic(topic_name).await?;

    client.disconnect().await?;

    Ok(())
}

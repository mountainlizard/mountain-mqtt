use std::{env, error::Error};

use mountain_mqtt::packet_client::PacketClient;
use mountain_mqtt::packets::connect::Connect;
use mountain_mqtt::packets::packet_generic::PacketGeneric;
use mountain_mqtt::tokio::ConnectionTcpStream;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting test server");

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let listener = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);

    loop {
        // Asynchronously wait for an inbound socket.
        let (tcp_stream, _) = listener.accept().await?;
        println!("Accepted connection...");

        tokio::spawn(async move {
            let mut buf = [0; 16384];
            let connection = ConnectionTcpStream::new(tcp_stream);
            let mut client = PacketClient::new(connection, &mut buf);

            loop {
                {
                    let packet: PacketGeneric<'_, 16, 16, 16> = client
                        .receive()
                        .await
                        .expect("failed to read packet from socket");
                    println!("Received packet {:?}", packet);
                }

                client
                    .send(Connect::unauthenticated("blah"))
                    .await
                    .expect("failed to send packet to socket")

                // socket
                //     .read_exact(&mut buf)
                //     .await
                //     .expect("failed to read data from socket");
                // println!("Received data {:?}", buf);

                // socket
                //     .write_all(&buf)
                //     .await
                //     .expect("failed to write data to socket");
                // println!("... echoed!");
            }
        });
    }
}

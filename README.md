# mountain-mqtt

## TODO

- [x] Port across mqtt core in lightbox as new `Client`.
- [x] Refactor `Client` errors to wrap packet read/write errors where appropriate, add own state-based errors on top.
- [x] Send puback in response to qos1 publish in client state
- [x] Look at suback reason codes - expect to match the requested subscription qos?
- [x] Integration test for `Client`
- [ ] Features - base code just relies on heapless etc., then a tokio feature for tokio adapters, embassy for embassy net adapters
- [ ] Embedded network adapter, connect and disconnect from a pico w
- [ ] Look at embassy-net TcpSocket methods set_keep_alive and set_timeout - can use to timeout and close socket when it detects that endpoint is no longer responding at TCP level.
- [ ] Extended embassy client, wrapping ClientNoQueue. Accept a list of topics to subscripe to on each reconnect, and then everything else is done with queues - incoming for messages to publish, outgoing for received messages.
- [ ] Client integration test using QoS1 publish (send and receive)
- [ ] Add github actions for integration tests
- [ ] More integration tests - try username and password, some properties?
- [ ] Look at how we add in more embedded stuff - e.g. implementation of our Delay for embassy delay, convenience methods for using embassy networking, cfg-gated fmt implementations for errors. Probably nothing needed on no_std since we (hopefully) just don't use std?
- [ ] Docs, with some fairly full examples, overview of what you need to know about mqtt to use the highest level API (client?). Would be nice to have a simple/diagrammed run through a typical mqtt exchange, doesn't seem to be much like that available at the moment.
- [ ] Can we make packet reads neater, by monitoring current remaining length for special case handling? So rather than checking remaining len directly, just read part by part and stop if we run out of data at a point where omitting remaining elements is accepted by the spec.
- [ ] Look at new approaches to concurrency - e.g. use timeouts throughout reading/writing rather than just checking first byte of packet is available. E.g. in tokio, look at `timeout`, and carefully read up on which `TcpStream` methods are cancel safe - seems to be the "xxx_buf" ones, where it's guaranteed that any data that has been read has been used to update the provided buf position. Not sure if there is an equivalent in embassy-net, the Read trait mentions that implementations should document whether they are cancel safe, need to look into this. If this isn't possible, can we use multiple tasks so we don't require timeouts (or if we need a timeout, it will be one that leads to just giving up on the connection and starting again, unlike the current requirement for a timeout to intersperse sends and pings).

## Robust reconnection approaches

Multiple tasks, separate read and write tasks:

1. Make a connection to server (using a new tcp stream, client state etc.)
2. Send a connect packet
3. Wait for connack with timeout, if timeout kill connection, return to 1.
4. Start Task 1: Uses only receive-side of TcpStream. Loop until connection killed, waiting for a packet (either with a timeout/read_ready poll, or if we have Task 3 watchdog doesn't need a timeout) and providing to client state. On receiving publish message back from client, push it to queue for another task to handle. On error in client state or reading from stream, kill connection, return to 1. If we DON'T have Task 3 (in which case we need to know we will do the following check regularly and so we need timeouts on read), on not having seen any packets from the server for more than a timeout that must be a sensible amount longer than PING_INTERVAL, kill connection, return to 1.
   Start Task 2: Uses only send-side of TcpStream. Loop until connection killed, waiting for either a (topic_name/payload) to publish from a queue and using client_state to make a Publish packet then send it, or a timeout indicating we are due for a ping, so use client_state to make one and then send it.
   TBD: Start Task 3: Watchdog - if there have been no received packets from server for timeout, kill connection and return to 1. Note that this probably implies we need to make task 1, 2 (and 3?) be joined futures, since we want to drop tasks 1 and 2 to abort any ongoing read/write attempts, also drop the stream.

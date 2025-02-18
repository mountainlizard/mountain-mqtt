# mountain-mqtt

## TODO

- [x] Port across mqtt core in lightbox as new `Client`.
- [x] Refactor `Client` errors to wrap packet read/write errors where appropriate, add own state-based errors on top.
- [x] Send puback in response to qos1 publish in client state
- [ ] Look at suback reason codes - expect to match the requested subscription qos?
- [ ] Integration test for `Client`
- [ ] Embedded network adapter, connect and disconnect from a pico w
- [ ] Add github actions for integration tests
- [ ] More integration tests - try username and password, some properties?
- [ ] Look at how we add in more embedded stuff - e.g. implementation of our Delay for embassy delay, convenience methods for using embassy networking, cfg-gated fmt implementations for errors. Probably nothing needed on no_std since we (hopefully) just don't use std?
- [ ] Docs, with some fairly full examples, overview of what you need to know about mqtt to use the highest level API (client?). Would be nice to have a simple/diagrammed run through a typical mqtt exchange, doesn't seem to be much like that available at the moment.
- [ ] Can we make packet reads neater, by monitoring current remaining length for special case handling?
- [ ] Robust reconnection - see below.

## Robust reconnection approaches

Any approach - look at embassy-net TcpSocket methods set_keep_alive and set_timeout - can use to timeout and close socket when it detects that endpoint is no longer responding at TCP level.

Multiple tasks, separate read and write tasks:

1. Make a connection to server (using a new tcp stream, client state etc.)
2. Send a connect packet
3. Wait for connack with timeout, if timeout kill connection, return to 1.
4. Start Task 1: Uses only receive-side of TcpStream. Loop until connection killed, waiting for a packet (either with a timeout/read_ready poll, or if we have Task 3 watchdog doesn't need a timeout) and providing to client state. On receiving publish message back from client, push it to queue for another task to handle. On error in client state or reading from stream, kill connection, return to 1. If we DON'T have Task 3 (in which case we need to know we will do the following check regularly and so we need timeouts on read), on not having seen any packets from the server for more than a timeout that must be a sensible amount longer than PING_INTERVAL, kill connection, return to 1.
   Start Task 2: Uses only send-side of TcpStream. Loop until connection killed, waiting for either a (topic_name/payload) to publish from a queue and using client_state to make a Publish packet then send it, or a timeout indicating we are due for a ping, so use client_state to make one and then send it.
   TBD: Start Task 3: Watchdog - if there have been no received packets from server for timeout, kill connection and return to 1. Note that this probably implies we need to make task 1, 2 (and 3?) be joined futures, since we want to drop tasks 1 and 2 to abort any ongoing read/write attempts, also drop the stream.

Would all be much simpler with a plain old "read_with_timeout" that didn't trash data on timeout... Then we can just have a single loop that:

1. While client_state is waiting for response, read packets with timeout, passing any published messages to handler
2. If there's a packet to send, send it.
3. If we've not had a packet come back for a while, kill connection and restart

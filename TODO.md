# TODO

- [ ] Look at allowing "NoQueue" client state (or a new client state based on it) to queue more acks, this shouldn't need much more than storing a specified number of ids of each type (plus qos for suback), we can retain the state where we're waiting for something, but relax the requirement to always wait for this to clear before sending more messages. Will only fail if we send messages faster than server can deal with them, and will then result in an error when the queue is full. Each identifier is only u16, qos is a 3-element enum, 1 byte.

- [ ] Can we make PollClient compatible with old Client? Or some other way to have a general trait for "actions" that can be converted to MQTT that can be used with Client and PollClient?

- [ ] Description of PollClient. Basically a) you can send batches of sub/unsub/qos1 and then check fairly easily for them all being acked so you can continue. Recommend doing this with an async function that loops on "is_waiting", and is called using a timeout. b) qos1 publish is not re-sent, but allows you to detect if the server is not getting messages in a reasonable time, and respond by reconnecting, so it's just a kind of enhanced liveness check. If you want real QoS1 you need something that can cache messages.

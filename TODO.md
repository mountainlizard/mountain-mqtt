# TODO

- [ ] Look at allowing "NoQueue" client state (or a new client state based on it) to queue more acks, this shouldn't need much more than storing a specified number of ids of each type (plus qos for suback), we can retain the state where we're waiting for something, but relax the requirement to always wait for this to clear before sending more messages. Will only fail if we send messages faster than server can deal with them, and will then result in an error when the queue is full. Each identifier is only u16, qos is a 3-element enum, 1 byte.

# mountain-mqtt

## TODO

- [ ] Implement subscribe and unsubscribe packets (including ack)
- [ ] Implement reason code subsets for packets, similarly to what we have for properties
- [ ] Implement will in connect (new Will struct, optional instance in connect packet)
- [ ] Implement QoS2 packets (remaining Pub... packets)
- [ ] Simple tokio network adapter, see if we can connect and disconnect from mosquitto server
- [ ] Embedded network adapter, connect and disconnect from a pico w
- [ ] Do we need a "Raw" client? Should be pretty simple, just send a packet, poll for a packet.
- [ ] Port across mqtt core in lightbox as new MqttClient, test.
- [ ] Add integration tests?

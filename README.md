# mountain-mqtt

## TODO

- [x] Implement subscribe and unsubscribe packets (including ack)
- [x] Implement reason code subsets for packets, similarly to what we have for properties
- [x] Implement will in connect (new Will struct, optional instance in connect packet)
- [x] Implement `PacketRead` for connect packet (this should be the only missing read?)
- [x] Tests for connect packets with will
- [ ] Look at a bit flag library, or at least share the shifts between encode/decode?
- [ ] Neater modules
  - `data` for all raw data (i.e. everything but packets - include packet_identifier etc. since we might want to support read/write of this)
  - `codec` for reader, writer, read/write - should only operate on stuff from `data`
  - `packets` just the packets themselves (including generic packet in future?)
- [ ] Implement QoS2 packets (remaining Pub... packets)
- [ ] Simple tokio network adapter, see if we can connect and disconnect from mosquitto server
- [ ] Embedded network adapter, connect and disconnect from a pico w
- [ ] Do we need a "Raw" client? Should be pretty simple, just send a packet, poll for a packet.
- [ ] Port across mqtt core in lightbox as new MqttClient, test.
- [ ] Add integration tests?
- [ ] Docs, with some fairly full examples, overview of what you need to know about mqtt to use the highest level API (client?). Would be nice to have a simple/diagrammed run through a typical mqtt exchange, doesn't seem to be much like that available at the moment.

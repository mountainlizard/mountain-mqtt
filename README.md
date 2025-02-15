# mountain-mqtt

## TODO

- [x] Implement subscribe and unsubscribe packets (including ack)
- [x] Implement reason code subsets for packets, similarly to what we have for properties
- [x] Implement will in connect (new Will struct, optional instance in connect packet)
- [x] Implement `PacketRead` for connect packet (this should be the only missing read?)
- [x] Tests for connect packets with will
- [x] Implement QoS2 packets (remaining Pub... packets)
- [x] Look at a bit flag library, or at least share the shifts between encode/decode? - implemented using consts for shifts, bits and masks, and moving code to from/try_from where it makes sense.
- [x] Neater modules
  - `data` for all raw data (i.e. everything but packets - include packet_identifier etc. since we might want to support read/write of this)
  - `codec` for reader, writer, read/write - should only operate on stuff from `data`
  - `packets` just the packets themselves (including packet and generic packet in future?)
- [x] Review errors - check we're not over-using MalformedPacket, work out what error we want to expose at raw client level, should reader/writer just use this directly? - Done, there is now no more MalformedPacket error, only the ReasonCode, all errors have their own more specific variants. We now use `PacketReadError` and `PacketWriteError` in the reader/writer and packet client layers, we can add new error types as needed for higher levels.
- [x] Make types for properties and reason codes consistent - either re-use both reason code and property types from publish for puback, or make different reason code and property types for each. Currently we reuse reason codes types but distinguish property types. - WONTFIX - reviewed, and the differences make sense. The property types are different because they contain different properties - publish has an extended set, then all "responses" (puback, pubrec, pubrel, pubcomp) just have reason string and user property. The reason codes are shared between puback and pubrec because both are responding directly to a publish message - this gives them the name PublishReasonCode. Then pubrel and pubcomp share the pubrel reason code because they are both simple "success or no packet found" options. Can review later as we have more higher level client code, if this gets confusing.
- [ ] Simple tokio network adapter, see if we can connect and disconnect from mosquitto server
- [ ] Embedded network adapter, connect and disconnect from a pico w
- [ ] Do we need a "Raw" client? Should be pretty simple, just send a packet, poll for a packet.
- [ ] Port across mqtt core in lightbox as new MqttClient, test.
- [ ] Add integration tests?
- [ ] Docs, with some fairly full examples, overview of what you need to know about mqtt to use the highest level API (client?). Would be nice to have a simple/diagrammed run through a typical mqtt exchange, doesn't seem to be much like that available at the moment.

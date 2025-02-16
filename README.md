# mountain-mqtt

## TODO

- [x] Simple tokio network adapter, see if we can connect and disconnect from mosquitto server
- [ ] Move to expected packets in integration test
- [ ] Make suback and unsuback packets share same "first and additional" structure for reason codes, matching up to subscribe/unsubscribe packets
- [ ] Can we make packet reads neater, by monitoring current remaining length for special case handling?
- [ ] Embedded network adapter, connect and disconnect from a pico w
- [ ] Port across mqtt core in lightbox as new MqttClient, test.
- [ ] Add integration tests?
- [ ] Docs, with some fairly full examples, overview of what you need to know about mqtt to use the highest level API (client?). Would be nice to have a simple/diagrammed run through a typical mqtt exchange, doesn't seem to be much like that available at the moment.

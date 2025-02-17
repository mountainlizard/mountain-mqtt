# mountain-mqtt

## TODO

- [x] Port across mqtt core in lightbox as new `Client`.
- [ ] Integration test for `Client`
- [ ] Refactor `Client` errors to wrap packet read/write errors where appropriate, add own state-based errors on top.
- [ ] Embedded network adapter, connect and disconnect from a pico w
- [ ] Add github actions for integration tests
- [ ] More integration tests - try username and password, some properties?
- [ ] Look at how we add in more embedded stuff - e.g. implementation of our Delay for embassy delay, convenience methods for using embassy networking, cfg-gated fmt implementations for errors. Probably nothing needed on no_std since we (hopefully) just don't use std?
- [ ] Docs, with some fairly full examples, overview of what you need to know about mqtt to use the highest level API (client?). Would be nice to have a simple/diagrammed run through a typical mqtt exchange, doesn't seem to be much like that available at the moment.
- [ ] Can we make packet reads neater, by monitoring current remaining length for special case handling?

# Delay Tolerant Routing Daemon

A daemon to route traffic in according to the Bundle Protocol [RFC 9171](https://datatracker.ietf.org/doc/rfc9171/) along with client apis.

This software is still in its early days and is most probably not ready for real world use.

## A note on versions

I am aware that there are github releases with a major version of `1.*` or higher.
However this is purely due to the way semantic-release works, i would have preferred `0.*` but it does not support that.

## Background

Delay Tolerant Networking (DTN) cares about networking in environments that limited connectivity or large communication delays.
Instead of expecting a reliable and somewhat low latency environment like TCP does DTN can be used if there is no end-to-end connectivity all the time.
The routers of a DTN take care to buffer the data so that arbitrary delays can be handled.
Also the routers choose a path that is either currently most appropriate or will be available in the near future.

Instead of using TCP/UDP stream to send data, data is encapsulated in bundles.
Each Bundle sent should be self sufficient and independent of potentially other bundles since delivery order is not guaranteed.

One of the main current application of DTN is for the use in deep space networking.
DTN could e.g. also be used to send data from one mobile phone to the other without relying on both being online at the same time.

## Features

* Most of the Bundle Protocol [RFC 9171](https://datatracker.ietf.org/doc/rfc9171/) 
* TCPCL as convergance layer [RFC 9174](https://datatracker.ietf.org/doc/rfc9174/)
* A grpc client endpoint as well as a client library and cli
* Support for routing bundles to other connected nodes and based on user defined static routes

## Usage DTRD

Run it with `docker run ghcr.io/huettner94/dtn:main` (or compile it using cargo and run from the `dtrd` folder).

The following environment variables are defined:
| Name | Usage |
|-|-|
| NODE_ID | The node id of the current node. Needs to use the `dtn` protocol e.g. `dtn://node2` |
| RUST_LOG | Configure the log level. e.g. `tcpcl=debug,dtrd=debug` |
| GRPC_CLIENTAPI_ADDRESS | Admin and user clients connect using grpc on this address |
| TCPCL_LISTEN_ADDRESS | The address of the TCPCL convergance layer (see [RFC 9174](https://datatracker.ietf.org/doc/rfc9174/)) |
| TCPCL_CERTIFICATE_PATH, TCPCL_KEY_PATH, TCPCL_TRUSTED_CERTS_PATH | If TLS should be used for TCPCL then the keys and certificates needs to be specified here | 
| TOKIO_TRACING_PORT | If set tracing of tokio is enabled and connections are accepted on this port |

To generate the certificates for testing the tool `dtrd/gencert.sh` can be used.

## Usage Cli client

Run it with `docker run ghcr.io/huettner94/dtn:main dtrd_cli` (or compile it using cargo and run from the `cli` folder).

E.g. `docker run ghcr.io/huettner94/dtn:main dtrd_cli bundle --url http://localhost:50051 submit -d "dtn://node2/testlistener" --data thecakeisalie`

## Contributing

Pull requests are welcome. For major changes, please open an issue first
to discuss what you would like to change.

## License

All of this code is available under GPL v3 as specified in LICENSE.txt

# [![links - an easy to set up selfhostable link shortener](https://raw.githubusercontent.com/janm-dev/links/main/misc/banner.webp)](https://github.com/janm-dev/links)

[![repository license](https://img.shields.io/github/license/janm-dev/links)](https://github.com/janm-dev/links/blob/main/LICENSE)
[![test coverage](https://codecov.io/gh/janm-dev/links/branch/main/graph/badge.svg?token=SIN9JWZ3BG)](https://codecov.io/gh/janm-dev/links)
[![regular automated dependency audit](https://github.com/janm-dev/links/actions/workflows/audit.yaml/badge.svg)](https://github.com/janm-dev/links/actions/workflows/audit.yaml)
[![continuous integration](https://github.com/janm-dev/links/actions/workflows/ci.yaml/badge.svg)](https://github.com/janm-dev/links/actions/workflows/ci.yaml)
[![compatibility tests](https://github.com/janm-dev/links/actions/workflows/tests.yaml/badge.svg)](https://github.com/janm-dev/links/actions/workflows/tests.yaml)
[![contact us via email at dev@janm.dev](https://img.shields.io/badge/contact-dev%40janm.dev-informational)](mailto:dev+links@janm.dev)

## What it does

Links is an all-in-one link shortener, redirecting links like <https://example.com/07Qdzc9W> or <https://example.com/my-cool-link> to wherever you want, all while (soon) optionally collecting useful, but privacy-focused, statistics.
Links can be configured via a [command line interface](#cli), or (soon) via an HTTP-based api and website. Redirects are stored in a flexible, configurable way. Currently, a volatile in-memory store <sup>(not recommended)</sup> and [Redis](https://redis.com/) <sup>(recommended)</sup> are supported.
Links is designed to scale up and down horizontally as much as needed. You can run the links server as a [standalone executable](#standalone-executable) or in a lightweight [Docker container](#docker-container), load-balancing between different redirector servers however necessary (all requests are stateless, so each HTTP/gRPC request can be sent to any redirector).

## [Code documentation](https://docs.links.janm.dev/links/index.html)

## How to set it up

### Standalone executable

To start the links redirector server without TLS encryption (no HTTPS, unencrypted gRPC API), simply run the server executable without any arguments. The server will listen for incoming HTTP requests on port 80 on all interfaces and addresses and for API calls on port 50051 on localhost. There will also be inactive (due to no TLS configuration) listeners ready on port 443 for HTTPS and port 530 for encrypted gRPC API calls, both on all interfaces.

To run the server with TLS (HTTP, HTTPS, and encrypted gRPC), a certificate and key are required. You can get some for free from [Let's Encrypt](https://letsencrypt.org/). Once you have both of them saved somewhere in `pem` format, run the links server with `--tls-enable true` to enable tls, and pass the certificate file path with `--tls-cert` and the key file path with `--tls-key` (i.e. `links-server --tls-enable true --tls-cert path/to/cert.pem --tls-key path/to/key.pem`). Optionally, you can use `--https-redirect true` to redirect all HTTP requests to HTTPS first, before the external redirect. The command-line options have the same name as the same config option in the file, but they are in `kebab-case`, e.g. `--log-level ...`. For information about other optional arguments, run the links server with the `--help` flag.

In addition to command-line arguments, the links redirector server can also be configured using a config file, in toml, yaml, or json format. To load the configuration file, specify it with `--config path/to/config.file` when running the server. Self-documenting example config files can be found at [example-config.toml](./example-config.toml), [example-config.yaml](./example-config.yaml), and [example-config.json](./example-config.json).

Environment variables can also be used for configuration, similarly to command-line arguments. The environment variables have the same name as the same config option in the file, but they are in `SCREAMING_SNAKE_CASE` with the prefix `LINKS_`, e.g. `LINKS_LOG_LEVEL=...`.

You can use one or more of the above configuration methods at the same time. If an option is specified with multiple of these methods, the following order of precedence is used, later sources overriding earlier ones:

0. default values
1. environment variables
2. config file
3. command-line arguments

### Docker container

Since the links docker container is essentially just the server executable in container format, most of [the above section](#standalone-executable) applies, with one difference - tls is enabled by default with a randomly generated (at container build-time) self-signed certificate. This certificate and key pair should never be used in production, but they are useful for local testing. The certificate is located in `/cert.pem`, and the key in `/key.pem`. This default configuration is set from a config file located in `/config.toml`, set via the docker command with `--config /config.toml`.

## Editing redirects

### CLI

You can access the low-level gRPC API via the links cli to perform operations on the backend store.

For all the below commands, you can use `--help` to get help, `-v` to get more verbose results, `-t` to enable TLS encryption (required when it is enabled in the server), `-H` or `LINKS_RPC_HOST` to specify the redirector's hostname, `-P` or `LINKS_RPC_PORT` to specify the redirector's gRPC API port, and `-T` or `LINKS_RPC_TOKEN` to specify the API token. Run `links-cli help` for more info about the cli, or `links-cli help SUBCOMMAND` for more information about the specific subcommand. In a development environment, replace `links-cli` with `cargo run --bin cli --`.

To create a new redirect with a random ID and an optional vanity path, run

```sh
#            or use environment variables              destination URL    optional vanity path
links-cli -H 'links-host.example' -T 'API TOKEN' new https://example.com/ example-vanity-path
```

To remove a redirect or vanity path, run

```sh
#    or use environment variables        vanity path or link ID
links-cli -T 'RANDOM SECRET API TOKEN' rem example-vanity-path
```

To change a redirect's destination URL, run

```sh
#             link ID    new destination URL
links-cli set 0pB5DK8T https://example.com/new
```

For instructions on more `links-cli` subcommands, run `links-cli help`.

### HTTP API

TODO - not implemented yet

## How it works

Links has 3 main parts:

- The redirector server, responsible for redirecting incoming HTTP(S) requests to their destinations
- A backend store (e.g. Redis), responsible for storing redirect and vanity path information
- A configuration utility/service, responsible for allowing the user to set up redirects

All redirects have an ID - a 40 bit / 5 byte number, represented as an 8 character string in the format of `digit + 7 * base 38`, e.g. `07Qdzc9W` (see docs in `/src/id.rs` for more details). This ID format is designed to be readable and short, while giving a unique ID for over a trillion different redirects.

When the redirector server receives a request, it first checks if the path of the request is a links ID (starts with a digit, e.g. `07Qdzc9W`) or a vanity path (freeform text, doesn't start with a digit). If the path is not an ID, the server gets the ID of that redirect from the store. Then, the server gets the destination URL of the redirect corresponding to the ID. It then redirects the request to that URL, or responds with a 404 status if the ID or destination URL could not be found.

The redirector server is also responsible for allowing the user to edit redirects via a low-level gRPC API. You can interact with that API via a [command-line utility](#cli) or (soon) via an HTTP-based API and website. Definitions for the gRPC interface are located in `/proto/links.proto`. This API exists so that you can easily interact with the redirect store in a generic way, no matter what the actual storage backend is, or how much links has been scaled (though you _can_ (soon) optionally disable that API).

The backend store is accessed by the redirector server, and can also be used via the gRPC API. No external access is needed to set up or edit redirects. The storage backend implementation is generic, so that you can choose where redirects are stored. Redirects are stored in a backend-specific way, but generally as a map of `vanity path -> ID` and `ID -> destination URL`.

## How to build it

To build links, you first need to install a recent version of [Rust](https://www.rust-lang.org), ideally via [rustup](https://rustup.rs). You also need to install a recent version of [protoc](https://github.com/protocolbuffers/protobuf#protocol-compiler-installation) to compile .proto files.
Once you have all of those installed, you can simply run `cargo build --release` to build an optimized release version of all links binaries, which will be located in `/target/release/`.
If you just want to try links out, you can also just run it with `cargo run`. This version will not be optimized and will include debug symbols, but will compile faster.

To build a links redirector Docker container, run `docker build .`. The server binary in the container will be release-optimized. The container itself is made to be lightweight, using a `FROM scratch` container with statically linked musl-libc and OpenSSL for the final image to avoid the size overhead of Debian or Alpine. The final container image is only approximately 10 MB, about 35% smaller than an Alpine-based image.

### MSRV

The minimum supported rust version is currently Rust 1.62, but this may change in the future with a minor version update.

## Directory structure

| path          | contents                                               |
| ------------- | ------------------------------------------------------ |
| `/misc/`      | miscellaneous static assets                            |
| `/proto/`     | links protobuf/gRPC files for the low-level API        |
| `/src/`       | rust source for all non-website links stuff            |
| `/src/bin/`   | executable binary sources                              |
| `/src/store/` | implementation of storage backends                     |
| `/tests/`     | integration/end-to-end testing of links                |
| `/.github/`   | github-specific configuration/resources                |
| `/target/`    | rust build output <sup>_(do not commit)_</sup>         |
| `/build.rs`   | rust build script, compiles protobuf and minifies html |
| `/Dockerfile` | links redirector server docker container               |

## Attribution

- The readme header is based on an image of the Gale crater on Mars (where there is a rock outcrop named "Link") courtesy NASA/JPL-Caltech.
- While this repository doesn't contain the dependencies' source code, compiled distributions of links may contain compiled/binary forms of its dependencies. Dependency license information is included with each [release](https://github.com/janm-dev/links/releases).

## License

This project is licensed under GNU AGPLv3 or later (`AGPL-3.0-or-later`). You can find the full license text in the LICENSE file.

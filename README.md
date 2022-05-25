# ![links - an easy to set up selfhostable link shortener](misc/banner.webp)

[![repository license](https://img.shields.io/github/license/janm-dev/links)](https://github.com/janm-dev/links/blob/main/LICENSE)
[![weekly automated dependency audit](https://github.com/janm-dev/links/actions/workflows/audit.yaml/badge.svg)](https://github.com/janm-dev/links/actions/workflows/audit.yaml)
[![continuous integration (tests, linting, formatting, ...)](https://github.com/janm-dev/links/actions/workflows/ci.yaml/badge.svg)](https://github.com/janm-dev/links/actions/workflows/ci.yaml)
[![contact us via email at dev@janm.dev](https://img.shields.io/badge/contact-dev%40janm.dev-informational)](mailto:dev+links@janm.dev)

## What it does

Links is an all-in-one link shortener, redirecting links like <https://example.com/07Qdzc9W> or <https://example.com/my-cool-link> to wherever you want, all while (soon) optionally collecting useful (but privacy-focused) statistics.
Links can be configured via a [command line interface](#cli), or (soon) via an HTTP-based api and website. Redirects are stored in a flexible, configurable way. Currently, a volatile in-memory store <sup>(not recommended)</sup>, a file-based store <sup>(decent if you can't use redis and don't need to scale)</sup>, and [Redis](https://redis.com/) <sup>(recommended)</sup> are supported.
Links is designed to scale up and down horizontally as much as needed. You can run the links server as a [standalone executable](#standalone-executable) or in a lightweight [Docker container](#docker-container), load-balancing between different redirector servers however necessary (all requests are stateless, so each HTTP/gRPC request can be sent to any redirector).

## How to set it up

### Docker container

TODO - many important features are still work-in-progress

### Standalone executable

TODO - many important features are still work-in-progress

## Editing redirects

### CLI

You can access the low-level gRPC API via the links cli to perform operations on the backend store.

For all the below commands, you can use `--help` to get help, `-v` to get more verbose results, `-h` to specify the redirector's hostname, `-p` to specify the redirector's gRPC API port, and `-t` to specify the API token. Run `links-cli help` for more info about the cli, or `links-cli help SUBCOMMAND` for more information about the specific subcommand. In a development environment, replace `links-cli` with `cargo run --bin cli --`.

To create a new redirect with a random ID and an optional vanity path, run

```sh
links-cli -h 'LINKS HOST' -t 'API TOKEN' new https://example.com/
```

or

```sh
links-cli -h 'LINKS HOST' -t 'API TOKEN' new https://example.com/ example-vanity-path
```

To remove a redirect or vanity path, run

```sh
links-cli -h 'LINKS HOST' -t 'API TOKEN' rem 4QGk87b7
```

or

```sh
links-cli -h 'LINKS HOST' -t 'API TOKEN' rem example-vanity-path
```

For instructions on more `links-cli` subcommands, run `links-cli --help`.

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

To build links, you first need to install a recent version of [Rust](https://www.rust-lang.org), ideally via [rustup](https://rustup.rs). You also need to install [CMake](https://cmake.org/) or [protoc](https://grpc.io/docs/protoc-installation/) to compile .proto files.
Once you have all of those installed, you can simply run `cargo build --release` to build an optimized release version of all links binaries, which will be located in `/target/release/`. On Windows the executables have a `.exe` file extension (e.g. `server.exe`), on Linux they don't have a file extension at all (e.g. `server`).
If you just want to try links out, you can also just run it with `cargo run`. This version will not be optimized and will include debug symbols.

To build a links redirector Docker container, run `docker build .`. The server binary in the container will be release-optimized. The container itself is made to be lightweight, using a `FROM scratch` container with a statically linked musl-libc for the final image to avoid the size overhead of Debian or Alpine. The final container image is only approximately 6 MB, about half the size of an Alpine-based image.

## Directory structure

| path          | contents                                        |
| ------------- | ----------------------------------------------- |
| `/misc/`      | miscellaneous static assets                     |
| `/proto/`     | links protobuf/gRPC files for the low-level API |
| `/src/`       | rust source for all non-website links stuff     |
| `/src/bin/`   | executable binary sources                       |
| `/src/store/` | implementation of storage backends              |
| `/.github/`   | github-specific configuration/resources         |
| `/target/`    | Rust build output <sup>_(do not commit)_</sup>  |
| `/build.rs`   | Rust build script, compiles gRPC .proto files   |
| `/Dockerfile` | links redirector server docker container        |

## Attribution

- The readme header is based on an image of the Gale crater on Mars (where there is a rock outcrop named "Link") courtesy NASA/JPL-Caltech.

## License

This project is licensed under GNU AGPLv3 or later (`AGPL-3.0-or-later`). You can find the full license text in the LICENSE file.

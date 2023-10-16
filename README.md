# Middleman

Middleman is a recording reverse proxy meant for development work.
Middleman is language and platform agnostic.
The use case is any applications talking to an API. Use middleman to replay responses from the upstream API to not overload it and easy local development.

## Installation

### Option 1: Run natively on your host OS

* Install [the rust toolchain](https://www.rust-lang.org/tools/install).
* Clone this repo and run `cargo install --path ./`
* The binary should be installed to `/usr/local/cargo/bin/middleman`

### Option 2: Docker

* Build the image with `docker build -t middleman .`
* Run the image with `docker run -p 5050:5050 middleman`.
* Add `-v $(pwd)/tapes:/middleman/tapes` to persist tapes.
* **OPTIONAL** Add ` -v $(pwd)/middleman.toml:/middleman/etc/middleman.toml` to use a custom config file. All command line arguments are ignored when using a config file.

**NOTE** that the container will bind to `0.0.0.0` by default.

## Usage
Middlemand can either be configured with command line options or a toml config file.
If you need to re-record a response simply delete the existing recording.

```shell
$ middleman -h

Starts a reverse proxy to <UPSTREAM>, listens on <BIND>:<PORT>.
Records upstream responses to <TAPES> directory.
Returns recorded response if url matches (does not call upstream in this case).

Usage: middleman [OPTIONS]

Options:
  -p, --port <PORT>                Listen port [default: 5050]
  -u, --upstream <UPSTREAM>        The upstream host to send requests to [example: http://localhost:3000]
  -t, --tapes <TAPES>              The directory where tapes will be stored [default: ./tapes]
  -b, --bind <BIND>                The address to bind to [default: 127.0.0.1]
  -c, --config-path <CONFIG_PATH>  The path to a toml config file with the same options as cli [default: middleman.toml]
  -h, --help                       Print help
  -V, --version                    Print version
```

## TODO:

- [ ] Ignore headers configurable in toml.

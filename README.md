# Middleman

Middleman is a recording reverse proxy meant for development work.
Middleman is language and platform agnostic.
The use case is any applications talking to an API. Use middleman to replay responses from the upstream API to not overload it and easy local development.

## Installation

### Option 1: Build from source

* Install [the rust toolchain](https://www.rust-lang.org/tools/install).
* Clone this repo and run `cargo install --path ./`
* The binary should be installed to `/usr/local/cargo/bin/middleman`

### Option 2: Binary releases

Pre-compiled binary releases can be found at [releases](https://github.com/A-Helberg/middleman/releases)
* Download the release for your system
* tar -xf <the download>.tar.gz
* Optionally put the executable on your path, eg. in `/usr/local/bin`

### Option 3: Docker

* Build the image with `docker build -t middleman .`
* Run the image with `docker run -p 5050:5050 middleman`.
* Add `-v $(pwd)/tapes:/middleman/tapes` to persist tapes.
* **OPTIONAL** Add ` -v $(pwd)/middleman.toml:/middleman/etc/middleman.toml` to use a custom config file. All command line arguments are ignored when using a config file.

**NOTE** that the container will bind to `0.0.0.0` by default.

## Usage
Middlemand can either be configured with command line options or a toml config file.
If you need to re-record a response simply delete the existing recording.

```text
$ middleman -h

Starts a reverse proxy to <UPSTREAM>, listens on <BIND>:<PORT>.
Records upstream responses to <TAPES> directory.
Returns recorded response if url matches (does not call upstream in this case).

The optional header `x-middleman-passthrough` can be specified in http requests to middleman to pass a request through to the <UPSTREAM>.
Any value other than the exact string "false" will be considered Truthy.
The `--replay-only` config flag takes precedence over the `x-middleman-passthrough` header.

Usage: middleman [OPTIONS]

Options:
  -p, --port <PORT>
          Listen port [default: 5050]
  -u, --upstream <UPSTREAM>
          The upstream host to send requests to [example: http://localhost:3000]
      --upstream-port <UPSTREAM_PORT>
          The Upstream port to connect to [default: 443 when --upstream-tls] [default: 80]
      --upstream-tls
          Should we use TLS when connection to the upstream [default: false]
  -t, --tapes <TAPES>
          The directory where tapes will be stored [default: ./tapes]
  -b, --bind <BIND>
          The address to bind to [default: 127.0.0.1]
  -c, --config-path <CONFIG_PATH>
          The path to a toml config file with the same options as cli [default: middleman.toml]
      --replay-only
          Only replay responses. If specified middleman will not attempt to contact the upstream
      --listen-tls
          Should we listen for TLS connections? [default: false]
      --tls-port <TLS_PORT>
          The TLS Listen Port [default: 5443]
      --cert-file <CERT_FILE>
          The TLS cert file
      --private-key-file <PRIVATE_KEY_FILE>
          The TLS private key file
  -h, --help
          Print help
  -V, --version
          Print version
```

### TLS

To list for TLS(https) connection you would need to generate a certificate and private key file.
The easiest way to do this is with [makecert](https://github.com/FiloSottile/mkcert).

## TODO:

- [ ] Ignore headers configurable in toml.

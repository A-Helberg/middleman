FROM rust:1.73.0-bookworm AS build

RUN     mkdir --parents /build
WORKDIR /build
COPY    . .
RUN     cargo build --release

# Smol little debian image to run middleman
FROM debian:bookworm AS runtime

RUN apt-get update \
    && apt-get install --assume-yes --quiet openssl \
    && rm --recursive --force /var/lib/cache /var/lib/apt/lists

RUN  mkdir --parents /middleman/tapes /middleman/etc

COPY --from=build /build/target/release/middleman /bin/
COPY --from=build /build/docker/entrypoint.sh     /bin/

WORKDIR    /middleman
ENTRYPOINT [ "/bin/entrypoint.sh" ]

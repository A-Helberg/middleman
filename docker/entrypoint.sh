#!/bin/bash
set -e

if [ -f /middleman/etc/middleman.toml ]; then
  exec /bin/middleman --config-path /middleman/etc/middleman.toml "$@"
else
  exec /bin/middleman --tapes /middleman/tapes --bind 0.0.0.0 "$@"
fi

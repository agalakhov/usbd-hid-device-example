#!/bin/sh
set -e
cargo build --release
arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabi/release/usbd-hid-device-example usbd-hid-device-example.hex
st-flash --format ihex write usbd-hid-device-example.hex

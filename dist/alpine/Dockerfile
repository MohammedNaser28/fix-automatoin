FROM alpine:3.21 AS builder

RUN apk add --no-cache \
    wget cpio gzip xorriso grub grub-efi grub-bios \
    rust cargo \
    musl-dev linux-headers

# The binary is built separately in CI and mounted at /build
# This container is for the ISO assembly pipeline only
WORKDIR /build

VOLUME ["/build"]

ENTRYPOINT ["bash", "/build/dist/alpine/build.sh"]

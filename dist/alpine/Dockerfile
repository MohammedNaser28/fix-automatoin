FROM alpine:3.21 AS builder

RUN apk add --no-cache \
    wget cpio gzip xorriso grub grub-efi grub-bios \
    curl coreutils grep dosfstools mtools \
    linux-lts

# Pre-installed kernel from linux-lts package
RUN cp /boot/vmlinuz-lts /vmlinuz

# The binary is built separately in CI and mounted at /build
# This container is for the ISO assembly pipeline only
WORKDIR /build

VOLUME ["/build"]

ENTRYPOINT ["sh", "/build/dist/alpine/build.sh"]

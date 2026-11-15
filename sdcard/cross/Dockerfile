# Debian 13 Trixie on amd64, with arm64 multiarch libraries.
# rustc runs natively. The link is against the same Trixie sonames as the Pi.
FROM debian:trixie

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN dpkg --add-architecture arm64 \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        build-essential \
        pkg-config \
        gcc-aarch64-linux-gnu \
        libc6-dev-arm64-cross \
        libclang-dev \
        libasound2-dev:arm64 \
        libxkbcommon-dev:arm64 \
        libwayland-dev:arm64 \
        libx11-dev:arm64 \
        libxcursor-dev:arm64 \
        libxrandr-dev:arm64 \
        libxi-dev:arm64 \
        libxcb-render0-dev:arm64 \
        libxcb-shape0-dev:arm64 \
        libxcb-xfixes0-dev:arm64 \
        libgl1-mesa-dev:arm64 \
        libegl1-mesa-dev:arm64 \
        libgbm-dev:arm64 \
        libgles2-mesa-dev:arm64 \
        libdrm-dev:arm64 \
        libinput-dev:arm64 \
        libudev-dev:arm64 \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable --profile minimal \
            --target aarch64-unknown-linux-gnu \
    && rustc --version

WORKDIR /src

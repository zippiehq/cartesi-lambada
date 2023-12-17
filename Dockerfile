FROM --platform=linux/amd64 zippiehq/cartesi-lambada-base-image:1.0 AS lambada-image

FROM debian:bookworm-20230725-slim AS build
RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y curl build-essential libssl-dev pkg-config netcat-traditional
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |  sh -s -- --default-toolchain stable -y
WORKDIR /build
COPY ./.cargo /build/.cargo
COPY ./Cargo.toml /build/Cargo.toml
COPY ./Cargo.lock /build/Cargo.lock
COPY ./cartesi_lambda /build/cartesi_lambda
COPY ./lambada /build/lambada
ARG RELEASE=--release
RUN --mount=type=cache,target=/usr/local/cargo/registry PATH=~/.cargo/bin:$PATH cargo build $RELEASE

FROM debian:bookworm-20230725-slim AS image

RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y --no-install-recommends curl netcat-traditional
ARG ARCH=amd64
RUN curl -LO https://github.com/cartesi/machine-emulator/releases/download/v0.15.2/cartesi-machine-v0.15.2_$ARCH.deb
RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y \
    ./cartesi-machine-v0.15.2_$ARCH.deb \
    && rm -rf /var/lib/apt/lists/* \
    && rm cartesi-machine-v0.15.2_$ARCH.deb

RUN curl -LO https://github.com/ipfs/kubo/releases/download/v0.24.0/kubo_v0.24.0_linux-$ARCH.tar.gz
RUN tar -xvzf kubo_v0.24.0_linux-$ARCH.tar.gz
RUN bash kubo/install.sh && rm -rf kubo kubo_v0.24.0_linux-$ARCH.tar.gz

COPY --from=lambada-image /lambada-base-machine.tar.gz /lambada-base-machine.tar.gz
COPY --from=build /build/target/release/lambada /bin/lambada
COPY ./entrypoint.sh /entrypoint.sh
COPY ./sample /sample
RUN mkdir -p /data

FROM scratch
COPY --from=image / /
CMD sh /entrypoint.sh

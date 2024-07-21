FROM --platform=linux/amd64 ghcr.io/zippiehq/cartesi-lambada-base-machine:1.4.1 AS lambada-image

FROM debian:bookworm-20230725-slim AS build
RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y curl build-essential libssl-dev pkg-config protobuf-compiler cpp-riscv64-linux-gnu gcc-riscv64-linux-gnu binutils-riscv64-linux-gnu bison flex bc libclang-dev
ARG ARCH=amd64
RUN curl -LO https://github.com/cartesi/machine-emulator/releases/download/v0.17.0/cartesi-machine-v0.17.0_$ARCH.deb
RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y \
    ./cartesi-machine-v0.17.0_$ARCH.deb \
    && rm -rf /var/lib/apt/lists/* \
    && rm cartesi-machine-v0.17.0_$ARCH.deb
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |  sh -s -- --default-toolchain stable -y
WORKDIR /build
COPY ./.cargo /build/.cargo

FROM build AS build-lambada-cache
WORKDIR /build
COPY ./Cargo.toml /build/Cargo.toml
COPY ./Cargo.lock /build/Cargo.lock
COPY ./cartesi_lambda/Cargo.toml /build/cartesi_lambda/Cargo.toml
COPY ./lambada/Cargo.toml /build/lambada/Cargo.toml
COPY ./lambada-worker/Cargo.toml /build/lambada-worker/Cargo.toml
RUN mkdir -p /build/cartesi_lambda/src && touch /build/cartesi_lambda/src/lib.rs
RUN mkdir -p /build/lambada/src && touch /build/lambada/src
RUN mkdir -p /build/lambada/src && echo 'fn main() {}' > /build/lambada/src/main.rs
RUN mkdir -p /build/lambada-worker/src &&  echo 'fn main() { panic!("bad build"); }' > /build/lambada-worker/src/main.rs

ARG RELEASE=--release
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE
RUN mkdir -p /build/lambada/tests && echo 'mod lambada_functions_test {}' > /build/lambada/tests/lambada_test.rs
WORKDIR /build/lambada/tests
RUN PATH=~/.cargo/bin:$PATH cargo rustc $RELEASE --test lambada_test  -- --emit link="lambada_test"

FROM build-lambada-cache AS build-lambada
WORKDIR /build
RUN rm -rf /build/cartesi_lambda/src /build/lambada/src /build/lambada-worker/src
COPY ./cartesi_lambda/src /build/cartesi_lambda/src
COPY ./lambada/src /build/lambada/src
COPY ./lambada-worker/src /build/lambada-worker/src
RUN touch /build/cartesi_lambda/src/lib.rs /build/lambada/src/main.rs /build/lambada/src/lib.rs /build/lambada-worker/src/main.rs
ARG RELEASE=--release
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE

WORKDIR /build/lambada
COPY ./lambada/tests /build/lambada/tests
RUN touch /build/lambada/tests/lambada_test.rs
RUN PATH=~/.cargo/bin:$PATH cargo rustc --release --test lambada_test  -- --emit link="lambada_test"

FROM build AS build-espresso-cache
ARG RELEASE=--release
COPY ./subscribe-espresso/Cargo.toml /build/subscribe-espresso/Cargo.toml
COPY ./lambada/Cargo.toml /build/lambada/Cargo.toml
COPY ./cartesi_lambda/Cargo.toml /build/cartesi_lambda/Cargo.toml
RUN mkdir -p /build/cartesi_lambda/src && touch /build/cartesi_lambda/src/lib.rs
RUN mkdir -p /build/lambada/src && touch /build/lambada/src
RUN mkdir -p /build/subscribe-espresso/src && echo 'fn main() {}' > /build/subscribe-espresso/src/main.rs
WORKDIR /build/subscribe-espresso
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE

FROM build-espresso-cache AS build-espresso
COPY ./subscribe-espresso/src /build/subscribe-espresso/src
COPY ./cartesi_lambda/src /build/cartesi_lambda/src
COPY ./lambada/src /build/lambada/src
RUN touch /build/subscribe-espresso/src/main.rs

ARG RELEASE=--release
WORKDIR /build/subscribe-espresso
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE

FROM build AS build-celestia-cache
ARG RELEASE=--release
COPY ./subscribe-celestia/Cargo.toml /build/subscribe-celestia/Cargo.toml
COPY ./lambada/Cargo.toml /build/lambada/Cargo.toml
COPY ./cartesi_lambda/Cargo.toml /build/cartesi_lambda/Cargo.toml
RUN mkdir -p /build/cartesi_lambda/src && touch /build/cartesi_lambda/src/lib.rs
RUN mkdir -p /build/lambada/src && touch /build/lambada/src
RUN mkdir -p /build/subscribe-celestia/src && echo 'fn main() {}' > /build/subscribe-celestia/src/main.rs
WORKDIR /build/subscribe-celestia
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE

FROM build-celestia-cache AS build-celestia
COPY ./subscribe-celestia/src /build/subscribe-celestia/src
COPY ./cartesi_lambda/src /build/cartesi_lambda/src
COPY ./lambada/src /build/lambada/src
RUN touch /build/subscribe-celestia/src/main.rs

ARG RELEASE=--release
WORKDIR /build/subscribe-celestia
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE

FROM build AS build-avail-cache
ARG RELEASE=--release
COPY ./subscribe-avail/Cargo.toml /build/subscribe-avail/Cargo.toml
COPY ./lambada/Cargo.toml /build/lambada/Cargo.toml
COPY ./cartesi_lambda/Cargo.toml /build/cartesi_lambda/Cargo.toml
RUN mkdir -p /build/cartesi_lambda/src && touch /build/cartesi_lambda/src/lib.rs
RUN mkdir -p /build/lambada/src && touch /build/lambada/src
RUN mkdir -p /build/subscribe-avail/src && echo 'fn main() {}' > /build/subscribe-avail/src/main.rs
WORKDIR /build/subscribe-avail
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE

FROM build-avail-cache AS build-avail
COPY ./subscribe-avail/src /build/subscribe-avail/src
COPY ./cartesi_lambda/src /build/cartesi_lambda/src
COPY ./lambada/src /build/lambada/src
RUN touch /build/subscribe-avail/src/main.rs

ARG RELEASE=--release
WORKDIR /build/subscribe-avail
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE

FROM build AS build-evm-blocks-cache
ARG RELEASE=--release
COPY ./subscribe-evm-blocks/Cargo.toml /build/subscribe-evm-blocks/Cargo.toml
COPY ./lambada/Cargo.toml /build/lambada/Cargo.toml
COPY ./cartesi_lambda/Cargo.toml /build/cartesi_lambda/Cargo.toml
RUN mkdir -p /build/cartesi_lambda/src && touch /build/cartesi_lambda/src/lib.rs
RUN mkdir -p /build/lambada/src && touch /build/lambada/src
RUN mkdir -p /build/subscribe-evm-blocks/src && echo 'fn main() {}' > /build/subscribe-evm-blocks/src/main.rs
WORKDIR /build/subscribe-evm-blocks
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE

FROM build-evm-blocks-cache AS build-evm-blocks
COPY ./subscribe-evm-blocks/src /build/subscribe-evm-blocks/src
COPY ./cartesi_lambda/src /build/cartesi_lambda/src
COPY ./lambada/src /build/lambada/src
RUN touch /build/subscribe-evm-blocks/src/main.rs

ARG RELEASE=--release
WORKDIR /build/subscribe-evm-blocks
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE

FROM build AS build-evm-da-cache
ARG RELEASE=--release
COPY ./subscribe-evm-da/Cargo.toml /build/subscribe-evm-da/Cargo.toml
COPY ./lambada/Cargo.toml /build/lambada/Cargo.toml
COPY ./cartesi_lambda/Cargo.toml /build/cartesi_lambda/Cargo.toml
RUN mkdir -p /build/cartesi_lambda/src && touch /build/cartesi_lambda/src/lib.rs
RUN mkdir -p /build/lambada/src && touch /build/lambada/src
RUN mkdir -p /build/subscribe-evm-da/src && echo 'fn main() {}' > /build/subscribe-evm-da/src/main.rs
WORKDIR /build/subscribe-evm-da
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE

FROM build-evm-da-cache AS build-evm-da
COPY ./subscribe-evm-da/src /build/subscribe-evm-da/src
COPY ./cartesi_lambda/src /build/cartesi_lambda/src
COPY ./lambada/src /build/lambada/src
RUN touch /build/subscribe-evm-da/src/main.rs

ARG RELEASE=--release
WORKDIR /build/subscribe-evm-da
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE


FROM debian:bookworm-20230725-slim AS image
RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y --no-install-recommends netcat-traditional curl ca-certificates
RUN mkdir -p /run/sshd
ARG ARCH=amd64
ARG RELEASE_DIR=release
RUN curl -LO https://github.com/cartesi/machine-emulator/releases/download/v0.17.0/cartesi-machine-v0.17.0_$ARCH.deb
RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y \
    ./cartesi-machine-v0.17.0_$ARCH.deb \
    && rm -rf /var/lib/apt/lists/* \
    && rm cartesi-machine-v0.17.0_$ARCH.deb

RUN curl -LO https://github.com/ipfs/kubo/releases/download/v0.24.0/kubo_v0.24.0_linux-$ARCH.tar.gz
RUN tar -xvzf kubo_v0.24.0_linux-$ARCH.tar.gz
RUN bash kubo/install.sh && rm -rf kubo kubo_v0.24.0_linux-$ARCH.tar.gz

COPY --from=lambada-image /lambada-base-machine.car.gz /lambada-base-machine.car.gz
COPY --from=build-lambada /build/target/$RELEASE_DIR/lambada /bin/lambada
COPY --from=build-lambada /build/target/$RELEASE_DIR/lambada-worker /bin/lambada-worker
COPY --from=build-espresso /build/subscribe-espresso/target/$RELEASE_DIR/subscribe-espresso /bin/subscribe-espresso
COPY --from=build-celestia /build/subscribe-celestia/target/$RELEASE_DIR/subscribe-celestia /bin/subscribe-celestia
COPY --from=build-avail /build/subscribe-avail/target/$RELEASE_DIR/subscribe-avail /bin/subscribe-avail
COPY --from=build-evm-blocks /build/subscribe-evm-blocks/target/$RELEASE_DIR/subscribe-evm-blocks /bin/subscribe-evm-blocks
COPY --from=build-evm-da /build/subscribe-evm-da/target/$RELEASE_DIR/subscribe-evm-da /bin/subscribe-evm-da
COPY --from=build-lambada /build/lambada_test /bin/lambada_test
COPY ./cartesi-build.sh /usr/bin/cartesi-build
COPY ./wait-for-callback.pl /usr/bin/wait-for-callback.pl
RUN chmod +x /usr/bin/cartesi-build
COPY ./entrypoint-devkit.sh /entrypoint-devkit.sh
COPY ./install-devkit.sh /install-devkit.sh
COPY ./entrypoint.sh /entrypoint-lambada.sh
COPY ./sample /sample
ARG DEVKIT=-lambada
RUN if [ x$DEVKIT = x-devkit ]; then bash /install-devkit.sh; fi
RUN cp /entrypoint$DEVKIT.sh /entrypoint.sh
RUN mkdir -p /data

FROM scratch
ENV IPFS_PATH=/data/ipfs
COPY --from=image / /
CMD bash /entrypoint.sh

FROM --platform=linux/amd64 zippiehq/test-image:0.16 AS lambada-image

FROM debian:bookworm-20230725-slim AS build
RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y curl build-essential libssl-dev pkg-config protobuf-compiler cpp-riscv64-linux-gnu gcc-riscv64-linux-gnu binutils-riscv64-linux-gnu bison flex bc libclang-dev
ARG ARCH=amd64
RUN curl -LO https://github.com/cartesi/machine-emulator/releases/download/v0.16.0/cartesi-machine-v0.16.0_$ARCH.deb
RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y \
    ./cartesi-machine-v0.16.0_$ARCH.deb \
    && rm -rf /var/lib/apt/lists/* \
    && rm cartesi-machine-v0.16.0_$ARCH.deb
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |  sh -s -- --default-toolchain stable -y

WORKDIR /build
COPY ./.cargo /build/.cargo
COPY ./Cargo.toml /build/Cargo.toml
COPY ./Cargo.lock /build/Cargo.lock
COPY ./cartesi_lambda /build/cartesi_lambda
COPY ./lambada /build/lambada
COPY ./lambada-worker /build/lambada-worker
ARG RELEASE=--release
RUN PATH=~/.cargo/bin:$PATH cargo build $RELEASE

WORKDIR /kernel
COPY ./config-riscv64 ./.config
RUN curl -OL https://cdn.kernel.org/pub/linux/kernel/v5.x/linux-5.15.63.tar.xz && tar xf linux-5.15.63.tar.xz && cd linux-5.15.63 && cp ../.config . && \
    make CROSS_COMPILE=riscv64-linux-gnu-  ARCH=riscv -j$(nproc) oldconfig && \
    make CROSS_COMPILE=riscv64-linux-gnu-  ARCH=riscv -j$(nproc) && cp arch/riscv/boot/Image ../Image && cd .. && rm -rf linux-5.15.63

FROM debian:bookworm-20230725-slim AS image
RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y --no-install-recommends netcat-traditional curl ca-certificates
RUN mkdir -p /run/sshd
ARG ARCH=amd64
ARG RELEASE_DIR=release
RUN curl -LO https://github.com/cartesi/machine-emulator/releases/download/v0.16.0/cartesi-machine-v0.16.0_$ARCH.deb
RUN apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get install -y \
    ./cartesi-machine-v0.16.0_$ARCH.deb \
    && rm -rf /var/lib/apt/lists/* \
    && rm cartesi-machine-v0.16.0_$ARCH.deb

RUN curl -LO https://github.com/ipfs/kubo/releases/download/v0.24.0/kubo_v0.24.0_linux-$ARCH.tar.gz
RUN tar -xvzf kubo_v0.24.0_linux-$ARCH.tar.gz
RUN bash kubo/install.sh && rm -rf kubo kubo_v0.24.0_linux-$ARCH.tar.gz

COPY --from=lambada-image /lambada-base-machine.car.gz /lambada-base-machine.car.gz
COPY --from=build /build/target/$RELEASE_DIR/lambada /bin/lambada
COPY --from=build /kernel/Image /Image-riscv64
COPY --from=build /build/target/$RELEASE_DIR/lambada-worker /bin/lambada-worker
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

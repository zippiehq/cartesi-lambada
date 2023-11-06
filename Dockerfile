FROM --platform=linux/amd64 cartesi/machine-emulator:0.15.2 as build-emulator
USER root
RUN apt-get -y update; apt-get -y install curl
RUN curl -sSL https://github.com/foundry-rs/foundry/releases/download/nightly/foundry_nightly_linux_$(dpkg --print-architecture).tar.gz | \
    tar -zx -C /usr/local/bin

RUN apt-get install -y \
    build-essential \
    curl

RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
RUN echo 'source $HOME/.cargo/env' >> $HOME/.bashrc
ENV PATH="/root/.cargo/bin:${PATH}"
RUN \
    apt-get update && \
    apt-get install --no-install-recommends -y cmake unzip && \
    rm -rf /var/lib/apt/lists/*

RUN export ARCH=$(uname -m | sed 's/aarch64/aarch_64/') && \
   curl -LO https://github.com/protocolbuffers/protobuf/releases/download/v3.20.1/protoc-3.20.1-linux-$ARCH.zip && \
   unzip protoc-3.20.1-linux-$ARCH.zip -d $HOME/.local

RUN apt-get update && \
    apt-get install -y protobuf-compiler
RUN apt-get install -y pkg-config

FROM ubuntu:jammy

COPY --from=build-emulator / /

RUN apt-get install wget
RUN wget https://dist.ipfs.io/go-ipfs/v0.9.0/go-ipfs_v0.9.0_linux-amd64.tar.gz
RUN tar -xvzf go-ipfs_v0.9.0_linux-amd64.tar.gz
RUN bash go-ipfs/install.sh

RUN ipfs init --profile=server

RUN apt-get update && apt-get install -y --no-install-recommends libcurl4 curl \
    &&  rm -rf /var/lib/apt/lists/*

RUN apt-get update && \
    apt-get install -y protobuf-compiler
RUN apt-get install -y pkg-config
RUN apt-get install -y openssl
RUN apt-get install libssl-dev
COPY target/release/blocks_stream /bin/blocks_stream
COPY target/release/blocks_stream_celestia /bin/blocks_stream_celestia
COPY target/release/cartesi_lambda /bin/cartesi_lambda
COPY program program
COPY program/test-files /root/share/images
CMD sh -c "ipfs daemon & sleep 10 && /usr/bin/jsonrpc-remote-cartesi-machine --server-address=127.0.0.1:50051 & sleep 10 && RUST_LOG=info /bin/blocks_stream_celestia --height 0 --machine-dir /machines/foo1"

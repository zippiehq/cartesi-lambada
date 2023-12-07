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

#FROM ubuntu:jammy

#COPY --from=build-emulator / /

RUN apt-get install wget
RUN wget https://github.com/ipfs/kubo/releases/download/v0.24.0/kubo_v0.24.0_linux-amd64.tar.gz
RUN tar -xvzf kubo_v0.24.0_linux-amd64.tar.gz
RUN bash kubo/install.sh

RUN ipfs init --profile=server

RUN apt-get update && apt-get install -y --no-install-recommends libcurl4 curl \
    &&  rm -rf /var/lib/apt/lists/*

RUN apt-get update && \
    apt-get install -y protobuf-compiler
RUN apt-get install -y pkg-config
RUN apt-get install -y openssl
RUN apt-get install libssl-dev
COPY target/debug/cartesi_lambda /bin/cartesi_lambda
COPY target/debug/lambada /bin/lambada
COPY ./state /state
RUN mkdir /data
RUN mkdir /data/snapshot
CMD sh -c "ipfs daemon & sleep 30 && ipfs add --cid-version=1 -r /state && /usr/bin/jsonrpc-remote-cartesi-machine --server-address=127.0.0.1:50051 & sleep 60 && RUST_LOG=info RUST_BACKTRACE=full /bin/lambada --sequencer-url https://query.cortado.espresso.network/  --l1-provider wss://eth-sepolia.g.alchemy.com/v2/ynVGpb2sD3HhbMBR4aGbYTw5Sd2aLUQh --hotshot-address 0xed15e1fe0789c524398137a066ceb2ef9884e5d8 --machine-dir /machines/ipfs-using2 --appchain bafybeietvxuf5ymb4la6ctbso2qmp4zg5n7jljkn6icalmjkk5ee6pmytm"
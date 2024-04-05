#!/bin/bash
if [ x$IPFS_URL = x ]; then
  echo "Running container-local IPFS instance"
  if [ ! -e /data/ipfs ]; then
    IPFS_PATH=/data/ipfs ipfs init --profile=server
  fi
  IPFS_PATH=/data/ipfs ipfs config Addresses.API /ip4/0.0.0.0/tcp/5001
  IPFS_PATH=/data/ipfs ipfs config Addresses.Gateway /ip4/0.0.0.0/tcp/8080
  IPFS_PATH=/data/ipfs ipfs daemon &
  IPFS_HOST="127.0.0.1"
  IPFS_PORT="5001"

  while true; do
     nc -z "$IPFS_HOST" "$IPFS_PORT"
     RET=$?
     echo $RET
     if [ x$RET = x0 ]; then
       break
     fi
     sleep 0.5
  done
  echo "IPFS up"
  IPFS_URL=http://127.0.0.1:5001

  if [ -e /data/preload ]; then
     (
        # "live reload"
        while true; do
          touch /preload-before
          IPFS_PATH=/data/ipfs ipfs add --cid-version=1 -Q -r /data/preload > /preload-after
          diff -u /preload-before /preload-after
          cp /preload-after /preload-before
          sleep 5
        done
     ) &
  fi
  IPFS_PATH=/data/ipfs ipfs add --cid-version=1 -r /sample
  (zcat /lambada-base-machine.car.gz | IPFS_PATH=/data/ipfs ipfs dag import &> /tmp/ipfs-base.log) &
fi


if [ -z "$IPFS_WRITE_URL" ]; then
  IPFS_WRITE_URL=$IPFS_URL
fi
export IPFS_WRITE_URL

if [ x$ESPRESSO_TESTNET_SEQUENCER_URL = x ]; then
   ESPRESSO_TESTNET_SEQUENCER_URL=https://query.gibraltar.aws.espresso.network
fi

if [ x$CELESTIA_TESTNET_SEQUENCER_URL = x ]; then
   CELESTIA_TESTNET_SEQUENCER_URL=http://0.0.0.0:26658
fi

if [ x$EVM_DA_URL = x ]; then
   EVM_DA_URL=http://127.0.0.1:8545
fi

mkdir -p /data/db
mkdir -p /data/db/chains/
mkdir -p /data/snapshot


LAMBADA_WORKER=/bin/lambada-worker RUST_LOG=info RUST_BACKTRACE=full /bin/lambada --espresso-testnet-sequencer-url $ESPRESSO_TESTNET_SEQUENCER_URL \
	--celestia-testnet-sequencer-url $CELESTIA_TESTNET_SEQUENCER_URL \
	--machine-dir=/data/base-machines/lambada-base-machine \
	--ipfs-url $IPFS_URL \
	--evm-da-url $EVM_DA_URL \
   --db-path /data/db/  2>&1 > /tmp/lambada.log
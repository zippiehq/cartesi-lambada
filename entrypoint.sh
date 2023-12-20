#!/bin/sh
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
fi

if [ x$CARTESI_MACHINE_URL = x ]; then
   echo "Running container-local cartesi machine"
   /usr/bin/jsonrpc-remote-cartesi-machine --server-address=127.0.0.1:50051 &
   JSONRPC_HOST="127.0.0.1"
   JSONRPC_PORT="50051"
   while true; do
        nc -z "$JSONRPC_HOST" "$JSONRPC_PORT"
        RET=$?
        echo $RET
        if [ x$RET = x0 ]; then
           break
        fi
        sleep 0.5
   done
   echo "Cartesi Machine up"
   CARTESI_MACHINE_URL=http://127.0.0.1:50051
fi

# XXX this probably does not work on remote cartesi machine and should be handled differently
# also forked urls may need adjusting in the code
if [ ! -e /data/base-machines/lambada-base-machine ]; then
   echo "Unpacking base machine"
   mkdir -p /data/base-machines
   cd /data/base-machines
   tar -zxvf /lambada-base-machine.tar.gz 
   cd /
fi

if [ x$SEQUENCER_URL = x ]; then
   SEQUENCER_URL=https://espresso.tspre.org
fi

mkdir -p /data/db
mkdir -p /data/snapshot

export CELESTIA_NODE_AUTH_TOKEN_READ=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJBbGxvdyI6WyJwdWJsaWMiLCJyZWFkIl19.7ZUno9U-S6MfAc4m-5HpgHxt_yLmY1xGUjVqeglC-DI

RUST_LOG=info RUST_BACKTRACE=full /bin/lambada --sequencer-url $SEQUENCER_URL \
	--machine-dir=/data/base-machines/lambada-base-machine \
	--ipfs-url $IPFS_URL \
	--cartesi-machine-url $CARTESI_MACHINE_URL \
	--db-path /data/db/
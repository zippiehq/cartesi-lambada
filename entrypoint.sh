#!/bin/sh
if [ x$IPFS_URL = x ]; then
   echo "Running container-local IPFS instance"
  if [ ! -e /data/ipfs ]; then
    IPFS_PATH=/data/ipfs ipfs init --profile=server
    gzip -dc /bafybeietvxuf5ymb4la6ctbso2qmp4zg5n7jljkn6icalmjkk5ee6pmytm.car.gz | IPFS_PATH=/data/ipfs ipfs dag import
  fi

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
     IPFS_PATH=/data/ipfs ipfs add --cid-version=1 -r /data/preload
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

COMPUTE_ONLY_OPT=

if [ x$COMPUTE_ONLY = x1 ]; then
   COMPUTE_ONLY_OPT=--compute-only
   APPCHAIN=bafybeiczsscdsbs7ffqz55asqdf3smv6klcw3gofszvwlyarci47bgf354 # empty directory
fi

if [ x$SEQUENCER_URL = x ]; then
   SEQUENCER_URL=https://query.cortado.espresso.network/
fi

if [ x$APPCHAIN = x ]; then
   echo -n "No appchain specified and not in compute only mode -- sample app: bafybeietvxuf5ymb4la6ctbso2qmp4zg5n7jljkn6icalmjkk5ee6pmytm"
   exit 1
fi

mkdir -p /data/db
mkdir -p /data/snapshot

RUST_LOG=info RUST_BACKTRACE=full /bin/lambada --sequencer-url $SEQUENCER_URL \
	--machine-dir=/data/base-machines/lambada-base-machine \
	--ipfs-url $IPFS_URL \
	--cartesi-machine-url $CARTESI_MACHINE_URL \
	--db-file /data/db/lambada.db \
	--appchain $APPCHAIN $COMPUTE_ONLY_OPT
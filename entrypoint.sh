#!/bin/bash
if [ x$IPFS_URL = x ]; then
  echo "Running container-local IPFS instance"
  if [ ! -e /data/ipfs ]; then
    IPFS_PATH=/data/ipfs ipfs init --profile=server
  fi
  IPFS_PATH=/data/ipfs ipfs config Addresses.API /ip4/0.0.0.0/tcp/5001
  IPFS_PATH=/data/ipfs ipfs config Addresses.Gateway /ip4/0.0.0.0/tcp/8080
  IPFS_PATH=/data/ipfs ipfs config --json Peering.Peers '[{"ID": "bafzbeibhqavlasjc7dvbiopygwncnrtvjd2xmryk5laib7zyjor6kf3avm", "Addrs": ["/dnsaddr/elastic.dag.house"]}]'
  if [ ! -z "$IPFS_GATEWAY_NOFETCH" ]; then
    IPFS_PATH=/data/ipfs ipfs config --json Gateway.NoFetch true
  fi
  if [ ! -z "$IPFS_DAEMON_OFFLINE" ]; then
    IPFS_PATH=/data/ipfs ipfs daemon --offline &
  else
    IPFS_PATH=/data/ipfs ipfs daemon &
  fi
  IPFS_HOST="127.0.0.1"
  IPFS_PORT="5001"
  
if [ -z "$LAMBADA_LOGS_DIR" ]; then
  LAMBADA_LOGS_DIR=/tmp
fi
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

  IPFS_PATH=/data/ipfs ipfs add --cid-version=1 -r /sample
  if [ -e /data/dev-machine ]; then
	  for x in /data/dev-machine/*.car; do
		IPFS_PATH=/data/ipfs ipfs dag import $x
	  done
  fi
  (zcat /lambada-base-machine.car.gz | IPFS_PATH=/data/ipfs ipfs dag import &> $LAMBADA_LOGS_DIR/ipfs-base.log) &
fi

if [ -z "$IPFS_WRITE_URL" ]; then
  IPFS_WRITE_URL=$IPFS_URL
fi
export IPFS_WRITE_URL

if [ x$ESPRESSO_TESTNET_SEQUENCER_URL = x ]; then
   ESPRESSO_TESTNET_SEQUENCER_URL=https://query.decaf.testnet.espresso.network
fi

if [ x$CELESTIA_TESTNET_SEQUENCER_URL = x ]; then
   CELESTIA_TESTNET_SEQUENCER_URL=http://0.0.0.0:26658
fi

if [ x$AVAIL_TESTNET_SEQUENCER_URL = x ]; then
   AVAIL_TESTNET_SEQUENCER_URL=wss://turing-rpc.avail.so/ws
fi

if [ x$AVAIL_MAINNET_SEQUENCER_URL = x ]; then
   AVAIL_MAINNET_SEQUENCER_URL=wss://mainnet.avail-rpc.com
fi

if [ x$EVM_DA_URL = x ]; then
   EVM_DA_URL=http://127.0.0.1:8545
fi

if [ x$WORKERS_LIMIT = x ]; then
   WORKERS_LIMIT=$(nproc)
fi

if [ x$SEQUENCER_MAP = x ]; then
   SEQUENCER_MAP=$(cat <<EOF
   {
      "avail": {
         "testnet": {
            "endpoint": "$AVAIL_TESTNET_SEQUENCER_URL"
         },
         "mainnet": {
            "endpoint": "$AVAIL_MAINNET_SEQUENCER_URL"
         }
      },
      "espresso": {
         "testnet": {
            "endpoint": "$ESPRESSO_TESTNET_SEQUENCER_URL"
         }
      },
      "celestia": {
         "testnet": {
            "rpc_endpoint": "$CELESTIA_TESTNET_SEQUENCER_URL"
         }
      },
      "evm-da": {
         "testnet": {
            "endpoint": "$EVM_DA_URL"
         }
      }
   }
EOF
)
fi

mkdir -p /data/db
mkdir -p /data/db/chains/
mkdir -p /data/snapshot

if [ "$RUN_TESTS" = "true" ]; then
   export LAMBADA_WORKER=/bin/lambada-worker
   export RUST_LOG=info
   export RUST_BACKTRACE=full
   export LAMBADA_LOGS_DIR=$LAMBADA_LOGS_DIR
   export WORKERS_LIMIT=$WORKERS_LIMIT
   export CELESTIA_TESTNET_NODE_AUTH_TOKEN_READ=dummy
   /bin/lambada --machine-dir=/data/base-machines/lambada-base-machine \
	   --ipfs-url $IPFS_URL \
      --sequencer-map "$SEQUENCER_MAP" \
      --db-path /data/db/ 2>&1 > $LAMBADA_LOGS_DIR/lambada.log &

   sleep 240
      export SERVER_ADDRESS=http://127.0.0.1:3033
   /bin/lambada_test --test-threads=1 --nocapture 

else
   export LAMBADA_LOGS_DIR=$LAMBADA_LOGS_DIR
   export WORKERS_LIMIT=$WORKERS_LIMIT
   export CELESTIA_TESTNET_NODE_AUTH_TOKEN_READ=dummy

   LAMBADA_WORKER=/bin/lambada-worker RUST_LOG=info RUST_BACKTRACE=full /bin/lambada --machine-dir=/data/base-machines/lambada-base-machine \
	   --ipfs-url $IPFS_URL \
      --sequencer-map "$SEQUENCER_MAP" \
      --db-path /data/db/  2>&1 > $LAMBADA_LOGS_DIR/lambada.log
fi
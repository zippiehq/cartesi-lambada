#!/bin/bash
IPFS_PATH=/data/ipfs

WORK_DIR=`mktemp -d`
APP_DIR=`mktemp -u --tmpdir=/`
truncate -s 4G $WORK_DIR/scratch.img
echo "#!/bin/bash" > $WORK_DIR/app-boot-script
echo "while ! systemctl is-active --quiet containerd.service; do" >> $WORK_DIR/app-boot-script
echo "  echo Waiting for containerd.service to become active..." >> $WORK_DIR/app-boot-script
echo "  sleep 1" >> $WORK_DIR/app-boot-script
echo "done" >> $WORK_DIR/app-boot-script
echo "echo running container"  >> $WORK_DIR/app-boot-script
echo "nerdctl --snapshotter=stargz run --rm --security-opt seccomp=unconfined --net host ipfs://CID" >>  $WORK_DIR/app-boot-script
buildctl build --frontend dockerfile.v0 --local context=. --local dockerfile=. --output type=oci,compression=estargz,oci-mediatypes=true,name=app:1.0 > $WORK_DIR/app.tar
if [ x$? != x0 ]; then echo "Build failed."; exit 1; fi

ctr i import --no-unpack $WORK_DIR/app.tar
nerdctl push --ipfs-ensure-image=false --estargz ipfs://app:1.0 > $WORK_DIR/cid
IPFS_CID=`cat $WORK_DIR/cid`
ipfs files mkdir --cid-ver=1 $APP_DIR
ipfs files mkdir $APP_DIR/container-artifacts
ipfs files cp /ipfs/$IPFS_CID $APP_DIR/container-artifacts/$IPFS_CID
ipfs cat $IPFS_CID | jq -r '.urls[]' | sed 's/ipfs:\/\///' > $WORK_DIR/urls
cat $WORK_DIR/urls
xargs -I@ ipfs files cp /ipfs/@ $APP_DIR/container-artifacts/@ < $WORK_DIR/urls
head -n 1 $WORK_DIR/urls | xargs -I@ ipfs cat @ | jq -r '.config.urls[]' | sed 's/ipfs:\/\///' >> $WORK_DIR/urls2
head -n 1 $WORK_DIR/urls | xargs -I@ ipfs cat @ | jq -r '.layers[].urls[]' | sed 's/ipfs:\/\///' >> $WORK_DIR/urls2
cat $WORK_DIR/urls2
xargs -I@ ipfs files cp /ipfs/@ $APP_DIR/container-artifacts/@ < $WORK_DIR/urls2
sed s/CID/$IPFS_CID/g < $WORK_DIR/app-boot-script | ipfs files write -e $APP_DIR/boot-script
echo -n '{"base_image_cid":"bafybeihce4lf6o27hunier4fxemynrkdctrz4trueb5ufjqlh5w7ga4lte"}' | ipfs files write -e $APP_DIR/info.json

echo $APP_DIR
echo "App can be found in IPFS MFS $APP_DIR"
echo "root:"
ipfs files ls -l $APP_DIR

echo "Container artifacts:"
ipfs files ls -l $APP_DIR/container-artifacts

echo "Boot script"

ipfs files read $APP_DIR/boot-script

APP_CID=`ipfs files stat --hash $APP_DIR`

echo "APP CID: $APP_CID"
CHAIN_DIR=`mktemp -u --tmpdir=/`

ipfs files mkdir --cid-ver=1 $CHAIN_DIR
ipfs files mkdir --cid-ver=1 $CHAIN_DIR/gov
curl http://localhost:3033/chain_info_template/espresso | ipfs files write -e $CHAIN_DIR/gov/chain-info.json
ipfs files cp /ipfs/$APP_CID $CHAIN_DIR/gov/app

CHAIN_CID=`ipfs files stat --hash $CHAIN_DIR`

echo "CHAIN CID: $CHAIN_CID"
DATE=`date`
echo "$DATE - Testing if the container succesfully initializes and requests a transaction when in a Cartesi Machine .."
curl -d 'no' -X POST -H "Content-Type: application/octet-stream" "http://localhost:3033/compute_with_callback/$CHAIN_CID?callback=http://localhost:9800/post&only_warmup=true" &> /dev/null
echo "Tailing cartesi machine log:"
tail -f /tmp/cartesi-machine.log &
TAIL_PID=$!
perl /usr/bin/wait-for-callback.pl
sleep 1
kill $TAIL_PID

echo "Done! Chain CID is $CHAIN_CID"
echo
echo "You can now make your Lambada node:"
echo "- subscribe to your chain with: curl http://127.0.0.1:3303/subscribe/$CHAIN_CID"
echo "- send a transaction to your chain: curl -X POST -d 'transaction data' -H \"Content-type: application/octet-stream\" http://127.0.0.1:3033/submit/$CHAIN_CID"
echo "- read latest state CID: curl http://127.0.0.1:3033/latest/$CHAIN_CID"
echo ""

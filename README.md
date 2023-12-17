# Cartesi Lambada

<img src="https://web3.link/lambada.png" width=50% height=50%>


Release build:

docker build -t cartesi-lambada:1.0 .

Debug build:

docker build -t cartesi-lambada:1.0 --build-arg RELEASE= --build-arg RELEASE_DIR=debug .

Start it up:

docker run -p 127.0.0.1:3033:3033 -v $PWD/data:/data cartesi-lambada:1.0

other terminal:

curl -X POST -d 'echo hello world' -H "Content-Type: application/octet-stream" -v http://127.0.0.1:3033/compute/bafybeigt3ajnts6tvfppdfhrhcibmpkuk2vfkttaua5vsyl4hxztqeo2ia

# Run chain:

Sample <appchain> is bafybeigt3ajnts6tvfppdfhrhcibmpkuk2vfkttaua5vsyl4hxztqeo2ia

Subscribe to a chain:

curl http://127.0.0.1:3033/subscribe/<appchain>

other terminal

curl -X POST -d 'transaction data' -H "Content-type: application/octet-stream" http://127.0.0.1:3033/submit/<appchain>


curl http://127.0.0.1:3033/block/<appchain>/<height>

curl http://127.0.0.1:3033/latest/<appchain>

# Run compute:

docker build -t cartesi-lambda:1.0
docker run -p 3033:3033 -e COMPUTE_ONLY=1 -v $PWD/data:/data cartesi-lambda:1.0

other terminal:

curl -X POST -d 'echo hello world' -H "Content-Type: application/octet-stream" -v http://0.0.0.0:3033/compute/bafybeigt3ajnts6tvfppdfhrhcibmpkuk2vfkttaua5vsyl4hxztqeo2ia

# Run chain:

docker run -p 3033:3033 -v $PWD/data:/data -e APPCHAIN=bafybeigt3ajnts6tvfppdfhrhcibmpkuk2vfkttaua5vsyl4hxztqeo2ia  cartesi-lambda:1.0
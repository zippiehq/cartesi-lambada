# Run compute:

docker build -t cartesi-lambada:1.0 .
docker run -p 127.0.0.1:3033:3033 -e COMPUTE_ONLY=1 -v $PWD/data:/data cartesi-lambada:1.0

other terminal:

curl -X POST -d 'echo hello world' -H "Content-Type: application/octet-stream" -v http://127.0.0.1:3033/compute/bafybeicgxhvvrhu6anwlozqgydpfp3qhn67zqoiv4ivjw2nndgwzbrztce

# Run chain:

docker run -p 127.0.0.1:3033:3033 -v $PWD/data:/data -e APPCHAIN=bafybeigt3ajnts6tvfppdfhrhcibmpkuk2vfkttaua5vsyl4hxztqeo2ia  cartesi-lambada:1.0

other terminal

curl -X POST -d '{"payload":[1,2,3,4],"vm":1000}' -H "Content-type: application/json" http://127.0.0.1:3033/submit/bafybeigt3ajnts6tvfppdfhrhcibmpkuk2vfkttaua5vsyl4hxztqeo2ia


curl http://127.0.0.1:3033/block/<appchain>/<height>

curl http://127.0.0.1:3033/latest/<appchain>

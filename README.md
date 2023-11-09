# Run

RUSTFLAGS="--cfg async_executor_impl=\"async-std\" --cfg async_channel_impl=\"async-std\"" cargo build --release && docker build -f Dockerfile -t ghcr.io/espressosystems/espresso-sequencer/blocks-stream:main . && docker run --network host -v /home/dymchenko/m:/machines -t ghcr.io/espressosystems/espresso-sequencer/blocks-stream:main
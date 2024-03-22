#!/bin/bash
#
set -e

# Copy NDI library to the local directory
#if [ ! -f "libndi.dylib" ]; then
#    curl -L https://github.com/nariakiiwatani/ofxNDI/raw/master/libs/NDI/lib/osx/x64/libndi.4.dylib --output libndi.dylib
#fi

# Point to NDI path
if [ "$DYLD_LIBRARY_PATH" = "" ]; then
    export DYLD_LIBRARY_PATH=`pwd`:/usr/local/lib:$DYLD_LIBRARY_PATH
fi

## Build release version
cargo build \
    --release \
    --features mps,ndi,audioplayer,metavoice,fonts

# Build debug version
cargo build \
    --features mps,ndi,audioplayer,metavoice,fonts

if [ ! -f "target/release/rsllm" ]; then
    echo "Error building rsllm, please check output"
    exit 1
fi

./target/release/rsllm -h

echo "Done, rsllm works and is in target/release/rsllm"

#!/bin/bash
#
DYLD_LIBRARY_PATH=`pwd`:$DYLD_LIBRARY_PATH target/release/rsllm \
    --use-candle \
    --candle-llm gemma \
    --max-tokens 300 \
    --model-id 2b-it \
    --sd-image \
    --system-prompt "you are a story teller who tells colorful magical stories." \
    --query "tell me a story about rainbows and the psychedelic 60s in San Francisco." \
    --loglevel error \
    --daemon \
    $@

## NDI Image output to RsLLM channel on monitor
# --ndi-images

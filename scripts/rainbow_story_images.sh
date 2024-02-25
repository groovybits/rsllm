#!/bin/bash
#
target/release/rsllm \
    --use-candle \
    --candle-llm gemma \
    --max-tokens 1000 \
    --model-id 2b-it \
    --sd-image \
    --system-prompt "you are a story teller who tells colorful magical stories" \
    --query "tell me a story about rainbows" \
    --loglevel error \
    $@

## NDI Image output to RsLLM channel on monitor
# --ndi-images

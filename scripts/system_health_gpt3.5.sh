#!/bin/bash

target/release/rsllm \
    --daemon  \
    --ai-os-stats \
    --poll-interval 60000 \
    --use-openai \
    --model "gpt-3.5-turbo" $@


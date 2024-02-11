#!/bin/bash

target/release/rsllm \
    --daemon  \
    --ai-os-stats \
    --poll-interval 60000 \
    --use-openai \
    --llm-history-size 3500 \
    --model "gpt-3.5-turbo" $@


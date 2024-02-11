#!/bin/bash
#
sudo target/release/rsllm \
    --daemon  \
    --use-openai \
    --model "gpt-3.5-turbo" \
    --ai-network-stats \
    --pcap-stats \
    --llm-history-size 3500 \
    --poll-interval 120000 \
    --ai-network-hexdump $@


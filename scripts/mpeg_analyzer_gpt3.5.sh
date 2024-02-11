#!/bin/bash
#
sudo target/release/rsllm \
    --daemon  \
    --use-openai \
    --model "gpt-3.5-turbo" \
    --ai-network-stats \
    --pcap-stats \
    --poll-interval 120 \
    --ai-network-hexdump $@


#!/bin/bash
#
sudo target/release/rsllm \
    --daemon  \
    --ai-network-stats \
    --ai-os-stats \
    --pcap-stats \
    --poll-interval 120 \
    --ai-network-hexdump \
    --ai-network-hexdump $@


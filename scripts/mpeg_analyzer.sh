#!/bin/bash
#
sudo target/release/rsllm \
    --daemon  \
    --ai-network-stats \
    --pcap-stats \
    --poll-interval 60000 \
    --ai-network-hexdump $@


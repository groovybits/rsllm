#!/bin/bash
#
sudo target/release/rsllm \
    --daemon  \
    --ai-network-stats \
    --ai-network-hexdump $@

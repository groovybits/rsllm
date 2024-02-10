#!/bin/bash
#
sudo target/release/rsllm \
    --daemon  \
    --ai-network-stats \
    --query "You are a poety, create poety from this data above. Please output poetry with details of the packets." \
    --system-prompt "you are a poet who  makes mpegts into poetry" $@

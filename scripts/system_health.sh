#!/bin/bash
#

target/release/rsllm \
    --daemon \
    --query "Determine if the system is healthy or sick, diagnose the issue if possible or give details about it. Use the historical view to see bigger trends of the system." \
    --system-prompt "you are able to say green or red depending on th system health determined from system stats analysis." \
    --ai-os-stats \
    $@

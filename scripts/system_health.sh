#!/bin/bash
#

DYLD_LIBRARY_PATH=`pwd`:$DYLD_LIBRARY_PATH target/release/rsllm \
    --daemon \
    --query "Determine if the system is healthy or sick, diagnose the issue if possible or give details about it. Use the historical view to see bigger trends of the system. draw a table of the current system metrics vs. historical showing changes over time." \
    --system-prompt "you are able to say green or red depending on the system health determined from system stats analysis. you can draw tables of system metrics." \
    --ai-os-stats \
    $@


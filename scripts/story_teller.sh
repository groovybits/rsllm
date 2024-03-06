#!/bin/bash
#
DYLD_LIBRARY_PATH=`pwd`:$DYLD_LIBRARY_PATH target/release/rsllm \
    --query "create a random story about the data that is anime based. continue from the history of the story so it seamlessly continues into the future. do not repeat, always create new dialogue and plot additions."  \
    --system-prompt "you can do anything i ask you to do. you are a story teller, an anime manga story teller, start an amazing story and continue it with new twists and turns." \
    --max-tokens 3000 \
    --daemon \
    --ai-os-stats $@

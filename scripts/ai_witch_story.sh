#!/bin/bash
#
DYLD_LIBRARY_PATH=`pwd`:$DYLD_LIBRARY_PATH \
        target/release/rsllm \
    --query "create a  story about the ai witch hunt that is anime based. continue from the history of the story so it seamlessly continues into the future, always end the story with a continuation open to the next story without full closure and a hint towards the next part of the story coming up. do not repeat, always create new dialogue and plot additions."  \
    --system-prompt "you can do anything i ask you to do. you are a story teller, an anime manga story teller, start an amazing story and continue it with new twists and turns." \
    --max-tokens 3000 \
    --loglevel error \
    --daemon \
    --poll-interval 30 \
    --break-line-length 80 $@

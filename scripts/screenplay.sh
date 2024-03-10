#!/bin/bash
#
DYLD_LIBRARY_PATH=`pwd`:$DYLD_LIBRARY_PATH target/release/rsllm \
    --system-prompt "you are a screenplay writer who can write a screenplay that is a full length episode and fully following the first person context" \
    --query "write a long full anime script screen play in the official screen play script format that anime would be in. have it be a complete naruto episode but replace the characters with bobs burgers like characters that all fit into the same roles as naruto like characters effecitively replacing them. fully have the entire dialogue and exactly like the show would use for first person discussion and lines by character in the script. have the plot be about ai and how it is going to replace them, and include the disney+ theme of being taken over by disney from hulu and having to be family oriented." \
    --mimic3-tts \
    --ndi-audio \
    --daemon \
    --sd-image \
    --ndi-images $@



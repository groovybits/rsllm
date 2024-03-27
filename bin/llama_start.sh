#!/bin/bash
#
#MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/dolphin-2.7-mixtral-8x7b.Q5_K_M.gguf
MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/dolphin-2.7-mixtral-8x7b.Q8_0.gguf

server \
    -m $MODEL \
    -c 0 \
    -np 1\
    --port 8080 \
    -ngl 60 \
    -t 24 \
    --host 0.0.0.0 $@

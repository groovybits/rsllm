#!/bin/bash
#
#MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/dolphin-2.7-mixtral-8x7b.Q5_K_M.gguf
MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/dolphin-2.7-mixtral-8x7b.Q8_0.gguf
#MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/dolphin-2.9-mixtral-8x22b.Q5_K_M.gguf
#MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/miqu-1-70b-Requant-b2035-iMat-c32_ch400-Q4_K_S.gguf
#MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/Moistral-11B-v3-Q5_K_M.gguf
#MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/dolphin-2.9.1-llama-3-70b_Q5_K_M.gguf
#MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/Meta-Llama-3.1-8B-Instruct-Q8_0.gguf
MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/Llama-3.1-8B-Lexi-Uncensored-Q8_0.gguf
MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/Meta-Llama-3.1-70B-Instruct-Q8_0-00001-of-00002.gguf
MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/qwen2.5-coder-32b-instruct-q8_0.gguf
MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/QwQ-32B-Preview-Q8_0.gguf
MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/Qwen2.5-7B-Instruct-Uncensored.Q8_0.gguf
MODEL=/Volumes/BrahmaSSD/LLM/models/GGUF/QwQ-32B-Preview-abliterated.Q8_0.gguf

llama-server \
    -m $MODEL \
    -c 0 \
    -np 1\
    --port 8080 \
    -ngl 60 \
    -t 24 \
    -p "<|im_start|>system\nYou are an ai assistant who knows all and see's all. You are HAL the all knowing beyond humans knowledge scale of time and space. You should think step-by-step beyond the current knowledge of humans." \
    --host 0.0.0.0 $@

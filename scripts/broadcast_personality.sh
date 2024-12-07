#!/bin/bash
#
# Buddhas AI Dharma Talks Character:
#
# RsLLM configuration script:
# - @2024 Christi Kennedy
#
#
#
PERSONALITY="$1"
if [ "$PERSONALITY" = "" ]; then
    PERSONALITY="buddha"
fi
echo "Using Personality $PERSONALITY"

# === PERSONALITY ===
source ./personalities/${PERSONALITY}.sh

echo "Using prompt: $PROMPT"
echo "Using greeting: $GREETING"
echo "Using assistant image prompt: $ASSISTANT_IMAGE_PROMPT"
echo "Using mimic3 voice: $MIMIC3_VOICE"

# === CONFIGURATION ===
BUILD_TYPE=release
## LLM Model Config
# Candle settings
USE_CANDLE=0
MODEL=mistral
#MODEL=gemma
MODEL_ID=7b-it
# Generic settings
USE_API=1
CHAT_FORMAT=chatml
#CHAT_FORMAT=llama2
#CHAT_FORMAT=vicuna
MAX_TOKENS=16000
TEMPERATURE=0.8
CONTEXT_SIZE=32000
QUANTIZED=0
KEEP_HISTORY=1
SD_MAX_LENGTH=500
## Pipeline Settings
DAEMON=1
CONTINUOUS=0
POLL_INTERVAL=2000
PIPELINE_CONCURRENCY=1
ASYNC_CONCURRENCY=0
NDI_TIMEOUT=600
## Twitch Chat Settings
TWITCH_MODEL=mistral
TWITCH_LLM_CONCURRENCY=1
TWITCH_CHAT_HISTORY=32
TWITCH_MAX_TOKENS_CHAT=500
TWITCH_MAX_TOKENS_LLM=$MAX_TOKENS
## Stable Diffusion Settings
SD_TEXT_MIN=300
#SD_WIDTH=720
#SD_WIDTH=860
SD_WIDTH=960
#SD_WIDTH=1280
SD_HEIGHT=512
#SD_HEIGHT=720
SD_API=1
SD_MODEL=turbo
#SD_CUSTOM_MODEL="mklanXXXNSFWPony_mklan235plusxxx.safetensors"
#SD_CUSTOM_MODEL="sd_xl_turbo_1.0.safetensors"
#SD_CUSTOM_MODEL="realisticVisionV51_v20Novae.safetensors"
#SD_CUSTOM_MODEL="v1-5-pruned-emaonly.safetensors"
SD_CUSTOM_MODEL="sd_xl_turbo_1.0_fp16.safetensors"
SD_INTERMEDIARY_IMAGES=0
SD_N_STEPS=12
ALIGNMENT=center
SUBTITLES=0
# === END OF CONFIGURATION ===
#
#
USE_API_CMD=
SUBTITLE_CMD=
DAEMON_CMD=
NO_HISTORY_CMD=
QUANTIZED_CMD=
ASYNC_CONCURRENCY_CMD=
SD_INTERMEDIARY_IMAGES_CMD=
SD_API_CMD=
USE_CANDLE_CMD=
if [ "$SD_API" == 1 ]; then
    SD_API_CMD="--sd-api"
fi
if [ "$SD_INTERMEDIARY_IMAGES" == 1 ]; then
    SD_INTERMEDIARY_IMAGES_CMD="--sd-intermediary-images"
fi
if [ "$USE_CANDLE_CMD" == 1 ]; then
    USE_CANDLE_CMD="--candle-llm $MODEL"
fi
if [ "$USE_API" == 1 ]; then
    USE_API_CMD="--use-api"
fi
if [ "$ASYNC_CONCURRENCY" == 1 ]; then
    ASYNC_CONCURRENCY_CMD="--async-concurrency"
fi
if [ "$SUBTITLES" == 1 ]; then
    SUBTITLE_CMD="--subtitles"
fi
if [ "$DAEMON" == 1 ]; then
    DAEMON_CMD="--daemon"
fi
if [ "$CONTINUOUS" == 1 ]; then
    CONTINUOUS_CMD="--continuous"
fi
if [ "$KEEP_HISTORY" == 0 ]; then
    NO_HISTORY_CMD="--no-history"
fi
if [ "$QUANTIZED" == 1 ]; then
    QUANTIZED_CMD="--quantized"
fi


#    --twitch-client \

DYLD_LIBRARY_PATH=`pwd`:/usr/local/lib:$DYLD_LIBRARY_PATH \
    RUST_BACKTRACE=full target/${BUILD_TYPE}/rsllm \
    --greeting "$GREETING" \
    --query "$PROMPT" \
    --assistant-image-prompt "$ASSISTANT_IMAGE_PROMPT" \
    --system-prompt "$SYSTEM_PROMPT" \
    --twitch-chat-history $TWITCH_CHAT_HISTORY \
    --twitch-llm-concurrency $TWITCH_LLM_CONCURRENCY \
    --twitch-model $TWITCH_MODEL \
    --twitch-max-tokens-chat $TWITCH_MAX_TOKENS_CHAT \
    --twitch-max-tokens-llm $TWITCH_MAX_TOKENS_LLM \
    --twitch-prompt "$TWITCH_PROMPT" \
    --mimic3-tts \
    --mimic3-voice $MIMIC3_VOICE \
    $SD_API_CMD \
    --sd-width $SD_WIDTH \
    --sd-height $SD_HEIGHT \
    --sd-image \
    --sd-model $SD_MODEL \
    --sd-custom-model $SD_CUSTOM_MODEL \
    --sd-n-steps $SD_N_STEPS \
    --image-alignment $ALIGNMENT \
    $SUBTITLE_CMD \
    $SD_INTERMEDIARY_IMAGES_CMD \
    --ndi-audio \
    --ndi-images \
    --ndi-timeout $NDI_TIMEOUT \
    $USE_API_CMD \
    $USE_CANDLE_CMD \
    --sd-text-min $SD_TEXT_MIN \
    --sd-max-length $SD_MAX_LENGTH \
    --llm-history-size $CONTEXT_SIZE \
    --chat-format $CHAT_FORMAT \
    --model-id $MODEL_ID \
    --temperature $TEMPERATURE \
    --pipeline-concurrency $PIPELINE_CONCURRENCY \
    --poll-interval $POLL_INTERVAL \
    $SINGLE_CONCURRENCY_CMD \
    $DAEMON_CMD \
    $CONTINUOUS_CMD \
    $NO_HISTORY_CMD \
    $QUANTIZED_CMD \
    $ASYNC_CONCURRENCY_CMD \
    --max-tokens $MAX_TOKENS

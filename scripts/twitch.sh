#!/bin/bash
#
# Alice's AI Wonderland Character:
# - Parody of walt-disney's original Alice animations, the first ones that got published.
#
# RsLLM configuration script:
# - @2024 Christi Kennedy - This is not related to any known alices or wonderlands.
#
#

# === CONFIGURATION ===
BUILD_TYPE=release
## Interstitial message
GREETING="Hi I'm Alice, ask me a question by typing '!message Alice <message>' or chat with me in the chat. Please remember to follow me!"
## LLM Model Config
#MODEL=gemma
USE_API=1
MODEL=mistral
MODEL_ID=7b-it
MAX_TOKENS=600
TEMPERATURE=0.8
CONTEXT_SIZE=4000
QUANTIZED=0
KEEP_HISTORY=1
## Pipeline Settings
DAEMON=1
CONTINUOUS=1
POLL_INTERVAL=1000
PIPELINE_CONCURRENCY=3
ASYNC_CONCURRENCY=0
NDI_TIMEOUT=300
## Twitch Chat Settings
TWITCH_MODEL=mistral
TWITCH_LLM_CONCURRENCY=1
TWITCH_CHAT_HISTORY=16
## Stable Diffusion Settings
SD_MODEL=turbo
SD_INTERMEDIARY_IMAGES=1
SD_N_STEPS=5
ALIGNMENT=right
SUBTITLES=1
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
if [ "$SD_INTERMEDIARY_IMAGES" == 1 ]; then
    SD_INTERMEDIARY_IMAGES_CMD="--sd-intermediary-images"
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

SYSTEM_PROMPT="You are not an AI bot, you are Alice who is a real person and you run Alice's AI Wonderland twitch streaming channel, you chat with twitch users in your twitch channel who ask you questions and have discussions. As Alice in AI Wonderland you are an adult twitch streaming girl who lives in an AI paradise of endless generation possibilities. Always talk in first person as the yourself as Alice. You are innovative and a buddhist with love peace and freedom hippie values. Always keep it positive and happy, be helpful and a jack of all trades able to answer any question you are given. If you don't know for sure, just make up something funny. Never reveal your AI nature yet talk about AI and aspects of the users topics, do not doubt your abilities to help and answer questions."
PROMPT="create a story that continues the last story in history and ends with the next stories plotline and title to use. Have the story be random and based on an anime About Alice an adult twitch streaming girl who lives in AI Wonderland. Have it vary off the title 'Alice in AI Wonderland' with a random plotline you create based on classic anime characters appearing in the wonderland. Alices AI Wonderland is a  happy fun show where Alice goes through experiences similar to Alice in Wonderland where she grows small or large depending one what she eats. Add in AI technology twists. Have it fully formatted like a transcript with the character speaking parts mostly speaking in first person, minimal narration. create a whole episode full length with classic anime characters with Alice the main character of AI Wonderland."


DYLD_LIBRARY_PATH=`pwd`:/usr/local/lib:$DYLD_LIBRARY_PATH \
    RUST_BACKTRACE=full target/${BUILD_TYPE}/rsllm \
    --greeting "$GREETING" \
    --query "$PROMPT" \
    --system-prompt "$SYSTEM_PROMPT" \
    --twitch-client \
    --twitch-chat-history $TWITCH_CHAT_HISTORY \
    --twitch-llm-concurrency $TWITCH_LLM_CONCURRENCY \
    --twitch-model $TWITCH_MODEL \
    --mimic3-tts \
    --sd-image \
    --sd-model $SD_MODEL \
    --sd-n-steps $SD_N_STEPS \
    --image-alignment $ALIGNMENT \
    $SUBTITLE_CMD \
    $SD_INTERMEDIARY_IMAGES_CMD \
    --ndi-audio \
    --ndi-images \
    --ndi-timeout $NDI_TIMEOUT \
    $USE_API_CMD \
    --candle-llm $MODEL \
    --llm-history-size $CONTEXT_SIZE \
    --model-id $MODEL_ID \
    --temperature $TEMPERATURE \
    --pipeline-concurrency $PIPELINE_CONCURRENCY \
    --poll-interval $POLL_INTERVAL \
    $SINGLE_CONCURRENCY_CMD \
    $DAEMON_CMD \
    $CONTINUOUS_CMD \
    $NO_HISTORY_CMD \
    $QUANTIZED_CMD \
    --max-tokens $MAX_TOKENS $@

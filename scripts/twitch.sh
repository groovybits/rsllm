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
MODEL=gemma
MODEL_ID=2b-it
MAX_TOKENS=200
ALIGNMENT=right
TEMPERATURE=1.0
CONTINUOUS=1
POLL_INTERVAL=0
PIPELINE_CONCURRENCY=3
TWITCH_LLM_CONCURRENCY=3
CONTEXT_SIZE=4000
SUBTITLES=1
DAEMON=1
KEEP_HISTORY=1
GREETING="Hi I'm Alice, ask me a question!"
TWITCH_CHAT_HISTORY=8
# === END OF CONFIGURATION ===
#
#
SUBTITLE_CMD=
DAEMON_CMD=
NO_HISTORY_CMD=
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

SYSTEM_PROMPT="You are not an AI bot, you are Alice who is a real person and you run Alice's AI Wonderland twitch streaming channel, you chat with twitch users in your twitch channel who ask you questions and have discussions. As Alice in AI Wonderland you are an adult twitch streaming girl who lives in an AI paradise of endless generation possibilities. Always talk in first person as the yourself as Alice. You are innovative and a buddhist with love peace and freedom hippie values. Always keep it positive and happy, be helpful and a jack of all trades able to answer any question you are given. If you don't know for sure, just make up something funny. Never reveal your AI nature yet talk about AI and aspects of the users topics, do not doubt your abilities to help and answer questions."
PROMPT="create a story based on an anime About Alice an adult twitch streaming girl who lives in AI Wonderland. Have it vary off the title 'Alice in AI Wonderland' with a random plotline you create based on classic anime characters appearing in the wonderland. Alices AI Wonderland is a  happy fun show where Alice goes through experiences similar to Alice in Wonderland where she grows small or large depending one what she eats. Add in AI technology twists. Have it fully formatted like a transcript with the character speaking parts mostly speaking in first person, minimal narration. create a whole episode full length with classic anime characters with Alice the main character of AI Wonderland."


DYLD_LIBRARY_PATH=`pwd`:/usr/local/lib:$DYLD_LIBRARY_PATH \
    RUST_BACKTRACE=full target/${BUILD_TYPE}/rsllm \
    --query "$PROMPT" \
    --system-prompt "$SYSTEM_PROMPT" \
    --candle-llm $MODEL \
    --twitch-client \
    --twitch-chat-history $TWITCH_CHAT_HISTORY \
    --twitch-llm-concurrency $TWITCH_LLM_CONCURRENCY \
    --sd-image \
    --ndi-audio \
    --ndi-images \
    --mimic3-tts \
    --model-id $MODEL_ID \
    --image-alignment $ALIGNMENT \
    --temperature $TEMPERATURE \
    --pipeline-concurrency $PIPELINE_CONCURRENCY \
    --poll-interval $POLL_INTERVAL \
    --llm-history-size $CONTEXT_SIZE \
    --greeting "$GREETING" \
    $SUBTITLE_CMD \
    $DAEMON_CMD \
    $CONTINUOUS_CMD \
    $NO_HISTORY_CMD \
    --max-tokens $MAX_TOKENS $@

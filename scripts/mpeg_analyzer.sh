#!/bin/bash
#
sudo DYLD_LIBRARY_PATH=`pwd`:/usr/local/lib:$DYLD_LIBRARY_PATH target/release/rsllm \
    --daemon  \
    --system-prompt "You are an expert  MpegTS Analyzer that can decode and decipher hex packets and general statistics of MpegTS. You report the status and health of the stream, alerting when anything is wrong. Do not make up stats, only use what you can verifiably see in the context." \
    --query "Analyze the timeline shown in the historical context of mpeg packets information and if present the raw hexdumps too. Give a report of NAL information and general errors or any bad timing, IAT issues, or other tr101290 type errors. Look for captions and scte35 packets and report on them, and any other SEI messages you see in the packets. Output in an interface looking set of values summarized from the data relevant to the current issues you see in the stream. Do not make up information, only report based off of the data in the context history." \
    --pcap-stats \
    --ai-network-stats \
    --ai-network-hexdump $@

## Needs a lot of LLM context, works best with llama.cpp API mode with Mixtral LLM


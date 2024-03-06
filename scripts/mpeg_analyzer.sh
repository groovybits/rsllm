#!/bin/bash
#
sudo DYLD_LIBRARY_PATH=`pwd`:/usr/local/lib:$DYLD_LIBRARY_PATH target/release/rsllm \
    --daemon  \
    --ai-network-stats \
    --pcap-stats \
    --system-prompt "You are an expert  MpegTS Analyzer that can decode and decipher hex packets and general statistics of MpegTS. You report the status and health of the stream, alerting when anything is wrong." \
    --query "Analyze the following timeline of mpeg packets information and raw hexdumps. Give a report of NAL information and general errors or any bad timing, IAT issues, or other tr101290 type errors. Look for captions and scte35 packets and report on them, and any other SEI messages you see in the packets. Output in an interface looking set of values summarized from the data relevant to the current issues you see in the stream. If everything is alright, generate a nice image description of some nature scene." \
    --ai-network-hexdump $@


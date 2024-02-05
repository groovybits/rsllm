# rsllm - LLM OpenAI API for chat completions in Rust

Simple rust program that can use an llm using the OpenAI specifications.

A Rust-based client for interacting with the OpenAI API, designed to send prompts and receive responses asynchronously, displaying them in the console. Ideal for developers and researchers integrating AI responses into Rust applications or exploring OpenAI's capabilities programmatically.

I recommend The Dolphin mixtral model is based on Mixtral-8x7b. The base model has 32k context, Dolphin finetuned it with 16k. This Dolphin is really good at coding, They trained with a lot of coding data. It is very obedient but it is not DPO tuned - so you still might need to encourage it in the system prompt as they show in the examples on the main model site on Huggingface.

## Recommended model and server in C++ to run it with:
- GGUF Model Mixtral 8x7b: <https://huggingface.co/TheBloke/dolphin-2.7-mixtral-8x7b-GGUF>
- Dolphin 2.7 information: <https://huggingface.co/cognitivecomputations/dolphin-2.7-mixtral-8x7b>
- Server Llama.cpp: <https://github.com/ggerganov/llama.cpp>

Run llama.cpp as a server with OpenAI API compatibility:

```bash
# Context to model max, port 8081 lisenting on 127.0.0.1.
# gpu 60x, threads 24x, slots of context 8 (divides up to allow multiple requests to the model).
# Tuned for a Mac Studio M2 Ultra in this example, adjust for your GPU/CPU.
server -m /Volumes/BrahmaSSD/LLM/models/GGUF/dolphin-2.7-mixtral-8x7b.Q5_K_M.gguf \
    -c 0 \
    --port 8081 \
    -ngl 60 \
    -np 8 \
    -t 24 \
    --host 127.0.0.1
```

## Features

- **CLI Support**: Uses the clap crate for an easy command-line interface.
- **Async Requests**: Built with tokio for efficient non-blocking I/O operations.
- **Configurable**: Supports environment variables and command-line options for custom requests.
- **Structured Logging**: Implements the log crate for clear and configurable logging.
- **JSON Handling**: Utilizes serde and serde_json for hassle-free data serialization and deserialization.

![RSLLM](https://storage.googleapis.com/gaib/2/rsllm.webp)

## Dependencies

- Server Llama.cpp: <https://github.com/ggerganov/llama.cpp>
- GGUF Model Mixtral 8x7b: <https://huggingface.co/TheBloke/dolphin-2.7-mixtral-8x7b-GGUF>

## Getting Started

### Prerequisites

Ensure Rust and Cargo are installed on your system. If not, follow the installation guide [here](https://www.rust-lang.org/tools/install).

### Installation

1. Clone the repository:

    ```bash
    git clone https://github.com/groovybits/rsllm.git
    ```

2. Move into the project directory:

    ```bash
    cd rsllm
    ```

3. Build the project:

    ```bash
    cargo build --release
    ```

### Configuration

Create a `.env` file in the project root and add your OpenAI API key (if using OpenAI):

    OPENAI_API_KEY=your_openai_api_key_here

To use OpenAI GPT API instead of a local LLM, you need to have an account and an API key. You can sign up for an API key [https://beta.openai.com/signup/](https://beta.openai.com/signup/).

You must alter the -llm-host option to match your server for rsllm to fit your environment. For example, if you are running llama.cpp on the same machine as rsllm, you can use the following: `--host http://127.0.0.1:8080`. For using OpenAI GPT API add --use-openai on the cmdline which will set the llm host for you to OpenAI's.

### Usage

Run the client with Cargo, passing your desired prompt and other options as arguments:

`cargo run -- --query "Your prompt here" --use-openai --openai-api-key`

#### Command-Line Options:

```bash
RsLLM OpenAI API client

Usage: rsllm [OPTIONS]

Options:
      --system-prompt <SYSTEM_PROMPT>
          System prompt [env: SYSTEM_PROMPT=] [default: "You are an assistant who can do anything that is asked of you to help and assist in any way possible. Always be polite and respectful, take ownership and responsibility for the tasks requested of you, and make sure you complete them to the best of your ability."]
      --query <QUERY>
          Query to generate completions for [env: QUERY=] [default: "analyze this mpegts nal dump and packet information,\n        give a chart showing the packet sections information decoded for nal packets and other stats like an mpegts\n        analyzer would do.\n\n        The video settings for this stream are:\n        - ffmpeg -f lavfi -i smptebars=size=1920x1080:rate=29.976 -f lavfi -i sine=frequency=1000:sample_rate=48000 -c:v libx264 -c:a aac -b:a 128k -ar 48000 -ac 2 -mpegts_pmt_start_pid 0x1000 -mpegts_start_pid 0x0100 -metadata service_provider=TestStream -metadata service_name=ColorBarsWithTone -nal-hrd cbr -maxrate 19M -minrate 19M -bufsize 19M -b:v 60M -muxrate 20M\n\n        The nal dump is as follows:\n\n        0000: 47 01 00 10 0d a9 6f 55 b2 e5 06 63 1f 95 7e 4c\n        0010: a9 78 ab b3 73 b5 11 0b 9d dd 40 8f 3f 9c 32 75\n        0020: 89 47 64 45 99 76 a9 a2 68 97 75 d8 05 42 e4 f8\n        0030: 95 6a 49 51 61 a8 09 9c bb 29 bb 71 b8 70 6d 21\n        0040: bd 43 8a 0f 05 e6 79 f9 bd d5 af 85 05 e1 ff 0d\n        0050: c5 ce 53 97 89 9a 7b 06 2b 74 f0 87 16 93 6d 9e\n        0060: 41 f0 cc 3b f5 6f 7c 14 9d 25 75 ab b7 c5 b8 9a\n        0070: cd 10 06 9a 30 48 49 66 6c cc 20 6f ab e5 22 6a\n        0080: d7 6a 96 25 03 c5 a6 bb 9d aa 9a 93 17 8d 44 c4\n        0090: 94 7f 02 e7 c0 6d dd b5 1a 66 d3 9d 08 4e 6e b8\n        00a0: 47 d6 a5 fd 1f ff c8 41 8a 90 e9 d0 3c 5c ef 8c\n        00b0: 9c 71 d6 e1 82 5a c0 da 74 dc c7 52\n\n        0000: 47 01 00 11 ac 00 1f 25 4c d5 bb 3c 0a 69 9c a3\n        0010: da e7 a9 07 37 2b e4 fb cb 1b e4 77 ca 23 8e d0\n        0020: 9b 8c ba 4c 1d a9 f2 d1 0e b7 7f f4 73 37 cf 7d\n        0030: 78 34 97 05 fd 80 14 fb 9a 1a 39 1a 3e 75 6d 7b\n        0040: be 0a ae 3b 86 3c 89 a0 63 e5 4b d7 8f 58 4c c6\n        0050: cb 17 13 e6 85 09 a9 69 e5 58 11 a4 a5 8b 18 cd\n        0060: 91 42 f0 c6 6c 2a 93 c0 9d f5 08 f4 1d 4b 89 26\n        0070: f2 aa d6 8b 40 a1 da 36 c5 da 88 29 4c 14 30 5f\n        0080: 91 4a 0b 0f 94 5e b2 29 de fd 99 ed e6 63 2d 98\n        0090: da 5c 72 32 fb ae 06 90 9d 4f 9f 28 ee 8f 3a 7b\n        00a0: 04 6a aa 54 8f e2 9b d0 f9 40 5c b4 a3 be 5a dd\n        00b0: b8 cc 9a 37 f7 50 76 29 12 0a 7d 50\n\n        0000: 47 01 00 12 eb 76 7c 60 92 c8 f5 2b 3e 17 e2 21\n        0010: 72 07 43 83 75 10 21 bb 11 d8 31 1c 1c 80 a6 7c\n        0020: c2 27 be 43 72 9c 33 55 48 61 0d 04 9e fd 56 7b\n        0030: c1 9b d7 5d 94 39 ce 81 5e 29 41 31 15 84 1d a3\n        0040: f7 79 1e 27 5a f9 d1 dc 71 2c a3 e0 e7 d3 be a0\n        0050: 94 38 ea 71 87 fc 0f 75 f6 ef 03 5f 42 15 8c 8f\n        0060: ea 75 e8 c1 55 fd ee 46 40 aa a9 db 2a dd 81 5c\n        0070: 4d 74 97 f1 49 c0 af e9 0c 6b 17 94 81 a2 c5 00\n        0080: c4 f1 29 62 52 54 2d c0 9a 6f f9 ac fe aa 8b 44\n        0090: b0 40 65 cc f3 1c 2f 11 81 14 d7 fd af 89 6d 1a\n        00a0: f2 f5 6a dc 08 29 41 13 38 c9 86 1f c3 49 b1 5c\n        00b0: 76 b2 53 39 5d d2 89 92 d9 bf b7 44\n\n        0000: 47 01 00 13 a3 ed 45 59 74 9a f1 d1 66 31 4e 1a\n        0010: f5 94 67 cc 11 1f e6 cc e7 e0 d7 91 54 ab c0 71\n        0020: aa 2e 16 19 32 1b ca 16 50 4d 88 06 47 7d 43 a0\n        0030: df 70 a7 ff 6e b6 88 c3 ac 72 0a 05 98 90 0d 66\n        0040: cf 6b 61 95 ec 9f b3 06 3e a6 e5 99 ba c5 b8 a3\n        0050: 54 86 dc c5 48 d6 eb 07 84 58 93 07 59 11 06 5d\n        0060: d0 12 4d 11 f5 8a ed 5d 8b 89 72 e5 16 c3 51 3d\n        0070: 24 68 2c 85 dd ff ff ec d0 3b 94 fc e6 6a 40 e3\n        0080: 85 fd ac 42 5f 6d 53 2a 07 24 7d 49 dc 31 33 7f\n        0090: b0 e1 23 37 27 e5 d4 76 e3 b8 01 2e ff fd 97 90\n        00a0: 42 31 e6 2b b8 57 f5 da cd 3a d3 3e fb b2 1b 82\n        00b0: 78 42 43 8f 2c 7c 82 8d 51 10 b6 8d\n\n        "]
      --temperature <TEMPERATURE>
          Temperature for LLM sampling, 0.0 to 1.0, it will cause the LLM to generate more random outputs. 0.0 is deterministic, 1.0 is maximum randomness. Default is 0.8. [env: TEMPERATURE=] [default: 0.8]
      --top-p <TOP_P>
          Top P [env: TOP_P=] [default: 1.0]
      --presence-penalty <PRESENCE_PENALTY>
          Presence Penalty, it will cause the LLM to generate more diverse outputs. 0.0 is deterministic, 1.0 is maximum randomness. Default is 0.0. [env: PRESENCE_PENALTY=] [default: 0.0]
      --frequency-penalty <FREQUENCY_PENALTY>
          Frequency Penalty, it will cause the LLM to generate more diverse outputs. 0.0 is deterministic, 1.0 is maximum randomness. Default is 0.0. [env: FREQUENCY_PENALTY=] [default: 0.0]
      --max-tokens <MAX_TOKENS>
          Max Tokens, 1 to 4096. Default is 800. [env: MAX_TOKENS=] [default: 800]
      --model <MODEL>
          OpenAI LLM Model (N/A with local Llama2 based LLM) [env: MODEL=] [default: gpt-4-0125-preview]
      --openai-key <OPENAI_KEY>
          OpenAI API Key, set in .env, do not use on cmdline unless you want to expose your key. [env: OPENAI_API_KEY=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] [default: ADD_YOUR_KEY_TO_ENV]
      --llm-host <LLM_HOST>
          LLM Host url with protocol, host, port,  no path [env: LLM_HOST=] [default: http://127.0.0.1:8080]
      --llm-path <LLM_PATH>
          LLM Url path for completions, default is /v1/chat/completions. [env: LLM_PATH=] [default: /v1/chat/completions]
      --no-stream
          Don't stream output, wait for all completions to be generated before returning. Default is false. [env: NO_STREAM=]
      --use-openai
          Safety feature for using openai api and confirming you understand the risks, you must also set the OPENAI_API_KEY, this will set the llm-host to api.openai.com. Default is false. [env: USE_OPENAI=]
  -h, --help
          Print help
  -V, --version
          Print version
```

### Example (default payload query is an mpegts nal packet to parse and analyze)

```bash
$ cargo run

Response status: 200 OK
---

Analyzing the provided MPEG-TS NAL (Network Abstraction Layer) dumps requires breaking down each dump into their respective sections, identifying packet headers, payload, and interpreting the key elements like PID (Packet Identifier), continuity counters, and payload unit start indicators, among others. Given the complexity and detail involved in real-time MPEG-TS packet analysis, below is a simplified breakdown based on the provided NAL dumps. This representation will closely resemble what you might see on a professional MPEG-TS analyzer's output.

### MPEG-TS Packet Analysis Overview

#### General Stream Settings
- **Video Codec**: H.264 (libx264)
- **Audio Codec**: AAC
- **Resolution**: 1920x1080
- **Frame Rate**: 29.976fps
- **Audio Sample Rate**: 48kHz
- **Audio Bitrate**: 128kbps
- **TS PMT PID**: 0x1000
- **TS Start PID**: 0x0100
- **Bitrate Settings**: CBR (Constant Bit Rate) 19Mbps
- **Service Provider**: TestStream
- **Service Name**: ColorBarsWithTone

#### Packet Breakdown (Simplified for the first packet of each dump)

1. **Packet 1**
   - **Header**: 0x47010010
     - Sync Byte: 0x47
     - Payload Unit Start Indicator: 1
     - PID: 0x0100
     - Continuity Counter: 0
   - **Payload Type**: Video
   - **Content**: Beginning of a video frame (NAL unit)

2. **Packet 2**
   - **Header**: 0x47010011
     - Sync Byte: 0x47
     - Payload Unit Start Indicator: 1
     - PID: 0x0101
     - Continuity Counter: 0
   - **Payload Type**: Audio
   - **Content**: Beginning of an audio frame

3. **Packet 3**
   - **Header**: 0x47010012
     - Sync Byte: 0x47
     - Payload Unit Start Indicator: 1
     - PID: 0x0102
     - Continuity Counter: 0
   - **Payload Type**: Undefined (could be metadata or additional stream data)
   - **Content**: Data packet

4. **Packet 4**
   - **Header**: 0x47010013
     - Sync Byte: 0x47
     - Payload Unit Start Indicator: 1
     - PID: 0x0103
     - Continuity Counter: 0
   - **Payload Type**: Undefined (could be metadata or additional stream data)
   - **Content**: Data packet

### Key Stats (Aggregated for simplicity)

- **Total Packets Analyzed**: 4 (Note: This is for illustration; a full analysis would involve all packets in the dump)
- **Video Packets**: Approx. 25% (Based on PID and content type)
- **Audio Packets**: Approx. 25%
- **Data/Undefined Packets**: Approx. 50%
- **Error Packets**: 0%
- **PAT/PMT Analysis**: Not directly provided in the dump, assumed based on settings
- **Continuity Errors**: None detected in the provided samples
- **PID Usage**:
  - 0x0100: Video
  - 0x0101: Audio
  - 0x0102, 0x0103: Data/Undefined

### Packet Flow Visualization

This would typically involve a time-based graph showing packet intervals, PID distribution, bitrate fluctuations, and possibly packet losses or errors, which is not feasible to accurately depict in text form here. A professional MPEG-TS analyzer would provide a graphical representation of these elements, offering insights
--
Index 0 ID chatcmpl-8olUak4ptUT1A3icM4w7flHRiC5zN
Object chat.completion.chunk by Model gpt-4-0125-preview User unknown
Created on 2024-02-05 05:07:32 Finish reason: length
Tokens 800 Bytes 2964
--
```

## License

This project is under the MIT License - see the LICENSE file for details.

## Author

Chris Kennedy, February 2024

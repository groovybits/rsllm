# rsllm - LLM OpenAI API for chat completions in Rust

Simple rust program that can use an llm using the OpenAI specifications.

A Rust-based client for interacting with the OpenAI API, designed to send prompts and receive responses asynchronously, displaying them in the console. Ideal for developers and researchers integrating AI responses into Rust applications or exploring OpenAI's capabilities programmatically.

## Recommended model and server in C++ to run it with:
- GGUF Model Mixtral 8x7b: <https://huggingface.co/TheBloke/dolphin-2.7-mixtral-8x7b-GGUF>
- Dolphin 2.7 information <https://huggingface.co/cognitivecomputations/dolphin-2.7-mixtral-8x7b>
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

```markdown
The Dolphin mixtral model is based on Mixtral-8x7b

The base model has 32k context, Dolphin finetuned it with 16k.

This Dolphin is really good at coding, They trained with a lot of coding data. It is very obedient but it is not DPO tuned - so you still might need to encourage it in the system prompt as they show in the examples on the main model site on Huggingface.
```

## Features

- **CLI Support**: Uses the clap crate for an easy command-line interface.
- **Async Requests**: Built with tokio for efficient non-blocking I/O operations.
- **Configurable**: Supports environment variables and command-line options for custom requests.
- **Structured Logging**: Implements the log crate for clear and configurable logging.
- **JSON Handling**: Utilizes serde and serde_json for hassle-free data serialization and deserialization.

![RSLLM](https://storage.googleapis.com/gaib/2/rsllm.webp)

## Dependencies

To run RsLLM, you'll need the following crates:
- `reqwest` for HTTP requests.
- `clap` for parsing command-line arguments.
- `serde`, `serde_json` for JSON serialization.
- `log` for logging support.
- `tokio` for asynchronous programming.
- `chrono` for date and time operations.
- `dotenv` for loading environment variables.

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
    cd RsLLM-OpenAI-API-client
    ```

3. Build the project:

    ```bash
    cargo build --release
    ```

### Configuration

Create a `.env` file in the project root and add your OpenAI API key:

    OPENAI_API_KEY=your_openai_api_key_here

### Usage

Run the client with Cargo, passing your desired prompt and other options as arguments:

    cargo run -- --query "Your prompt here"

#### Command-Line Options:

```bash
RsLLM OpenAI API client

Usage: rsllm [OPTIONS]

Options:
      --system-prompt <SYSTEM_PROMPT>
          System prompt [env: SYSTEM_PROMPT=] [default: "You are an assistant who is helpful."]
      --query <QUERY>
          System prompt [env: QUERY=] [default: "analyze this mpegts nal dump and packet information, give a short summary of the stats like an mpegts\n        analyzer would do. The nal dump is as follows:\n\n    --- Packet Offset 0 Packet Length 88 ---\n\n    0000: 00 00 01 01 9f 70 74 41 9f 00 02 a6 82 1d 76 1b\n    0010: 69 92 36 f1 8c fb a9 87 5a 48 68 5d 5d bd 58 75\n    0020: 6d fd f5 32 d6 9d dc 88 b1 97 d0 40 79 39 f0 ea\n    0030: f0 b1 61 34 c4 2e d1 b1 ab f5 95 c5 b6 20 58 bb\n    0040: e8 95 f5 22 86 88 bc 09 7b 79 0e fe c1 81 14 85\n    0050: 9a 26 9f 58 d4 cc 1e 2e\n    ---"]
      --temperature <TEMPERATURE>
          Temperature [env: TEMPERATURE=] [default: 0.8]
      --top-p <TOP_P>
          Top P [env: TOP_P=] [default: 1.0]
      --presence-penalty <PRESENCE_PENALTY>
          Presence Penalty [env: PRESENCE_PENALTY=] [default: 0.0]
      --frequency-penalty <FREQUENCY_PENALTY>
          Frequency Penalty [env: FREQUENCY_PENALTY=] [default: 0.0]
      --max-tokens <MAX_TOKENS>
          Max Tokens [env: MAX_TOKENS=] [default: 800]
      --stream
          Stream [env: STREAM=]
      --model <MODEL>
          Model [env: MODEL=] [default: gpt-3.5-turbo]
      --openai-key <OPENAI_KEY>
          OpenAI API Key [env: OPENAI_API_KEY=FAKE_KEY] [default: FAKE_KEY]
      --llm-host <LLM_HOST>
          LLM Host url with protocol, host, port,  no path [env: LLM_HOST=] [default: http://earth.groovylife.ai:8081]
      --llm-path <LLM_PATH>
          LLM Url path [env: LLM_PATH=] [default: /v1/chat/completions]
  -h, --help
          Print help
  -V, --version
          Print version
```

#### Options:
- `--system-prompt` to set the system's initial prompt, defaulting to "You are an assistant who is helpful."
- `--query` for the prompt you wish to send to the OpenAI API.
- `--temperature` for controlling randomness in the response, defaulting to 0.8.
- `--top_p` for response diversity control, defaulting to 1.0.
- `--presence_penalty` and `--frequency_penalty` for context and repetition control, both defaulting to 0.0.
- `--max_tokens` to set the maximum number of tokens to generate, defaulting to 800.
- `--stream` for streaming the response, defaulted to true.
- `--model` to choose the model, defaulting to "gpt-3.5-turbo".

### Example (default payload query is an mpegts nal packet to parse and analyze)

```bash
$ cargo run

Response status: 200 OK
---

 Based on the given video settings and NAL dump, we can analyze the MPEG-TS stream as follows:

1. Packet Section Information for NAL Packets:

0000: 47 01 00 10 (start of an access unit)
0010: 0d a9 6f 55 b2 e5 06 63 1f 95 7e 4c (NAL unit - Start of sequence)
0020: a9 78 ab b3 73 b5 11 0b 9d dd 40 8f 3f 9c 32 75 (NAL unit - Sequence parameter set)
0030: 89 47 64 45 99 76 a9 a2 68 97 75 d8 05 42 e4 f8 (NAL unit - Picture parameter set)

The first four bytes of each NAL unit are the same: `47 01 00`, which is an MPEG-TS PES packet header. The next byte represents the NAL unit type:

- `10` corresponds to the start of an access unit (SEI)
- `0b` corresponds to sequence parameter set (SPS)
- `0c` corresponds to picture parameter set (PPS)

2. Other Stats like MPEG-TS Analyzer would do:

- Video Codec: H.264/AVC
- Frame Rate: 29.976 fps (from the lavfi smptebars source filter)
- Bitrate: 60 Mbps (`-b:v 60M`)
- Mux Rate: 20 Mbps (`-muxrate 20M`)
- Audio Codec: AAC (`-c:a aac`)
- Audio Bitrate: 128 kbps (`-b:a 128k`)
- Sample Rate: 48000 Hz (from the lavfi sine source filter)
- Channels: Stereo (`-ac 2`)
- PID for Video: 0x1000 (from `-mpegts_start_pid 0x0100`)
- PID for Audio: 0x1001 (derived from the video PID)
- Program Map Table start PID: 0x1000 (from `-mpegts_pmt_start_pid 0x1000`)
- MPEG Transport Stream mode enabled (derived from other options provided)

Note: The provided NAL dump contains four different sequences, each with its respective sequence parameter set (SPS) and picture parameter set (PPS). In a real-world MPEG-TS stream, these would be interleaved within the PES packets.
--
Index 0 ID chatcmpl-sPW7MhQL3SNf6jB1ePdmVgLlx7EBQOiU
Object chat.completion.chunk by Model gpt-3.5-turbo
Created on 2024-02-04 15:52:02 Finish reason: stop
Tokens 677 Bytes 1619
--
```

## License

This project is under the MIT License - see the LICENSE file for details.

## Author

Chris Kennedy, February 2024

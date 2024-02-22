# MacOS Metal GPU Rust TextGen/ImageGen/SpeechGen - AI System/Network/Stream Analyzer

This is focused on a MacOS M1/M2/M3 ARM GPU since that is what I have to test with.
If CPU/Nvidia are needed please help test and contribute the configuration to allow this easier.

Uses Candle with pure Rust LLM Mistral and Gemma. You can also use the OpenAI specifications for the LLM to any server that supports OpenAI API. Mixtral support is coming soon!

Analyze data from realtime captures of network devices or systems proc values or arbitrary streams of data. It can also be used to send prompts to the llm and display the results in the console. If you don't have Candle/Rust and a Metal Mac you will need to use llama.cpp server and the GGUF model Mixtral 8x7b. It can also be used with the OpenAI API.

Rust-based client for interacting with an LLM, designed to send prompts and receive responses asynchronously, displaying them in the console. Ideal for developers and researchers integrating AI responses into Rust applications or exploring OpenAI's capabilities programmatically. It also includes a system and network analyzer that can be used to capture and analyze network packets and system stats.

## Use Candle pure Rust LLM with Meta Llama2 based Mistral or Google Gemma
Use the command line args `--use-candle --use-gemma` or `--use-candle --use-mistral`, build with metal feature too or it will not work well..

## Stable Diffusion and NDI output of images (WIP)
Stable diffusion with Candle native Rust Diffusers/Transformers/Tensors. LLM support in pure Rust directly no server or Python.

Add --features metal to the cargo build command for MacOS GPU usage. `cargo build --features=metal,ndi`

NDI output of images WIP (and TTS speech audio TODO). You need the NDI SDK for this. <https://ndi.video/download-ndi-sdk/> add --features ndi to the cargo build command. This is what needs to be done too: <https://digitaldrummerj.me/obs29-ndi-apple-silicon/> basically get <https://ndi.video/tools/ndi-core-suite/> and move the libndi.dynlib into /usr/local/lib so it can be found. like `sudo cp "/Applications/NDI Video Monitor.app/Contents/Frameworks/libndi_advanced.dylib" "/usr/local/lib/libndi.4.dylib"`
 Then use `export DYLD_LIBRARY_PATH=/usr/local/lib:$DYLD_LIBRARY_PATH` at runtime. It's unfortunate NDI sdk libs aren't easier to deal with. Also logging into Huggingface Hub with the cli fixes some warnings you will get otherwise... `huggingface-cli login`.

## MetaVoice TTS Text to Speech Speaking (TODO)
Candle will support MetaVoice soon (PR a WIP is in the Candle project `https://github.com/huggingface/candle/compare/main...metavoice` which will allow pure Rust based LLM + TTI + TTS with Candle and Metal GPU.

## Recommended model when using llama.cpp OpenAI API, it is in C++ to run it with:
- Server Llama.cpp: <https://github.com/ggerganov/llama.cpp>
- GGUF Model Mixtral 8x7b: <https://huggingface.co/TheBloke/dolphin-2.7-mixtral-8x7b-GGUF>
- Dolphin 2.7 information: <https://huggingface.co/cognitivecomputations/dolphin-2.7-mixtral-8x7b>

I recommend The Dolphin mixtral model for llama.cpp, it is based on Mixtral-8x7b. The base model has 32k context, Dolphin finetuned it with 16k. This Dolphin is really good at coding, They trained with a lot of coding data. It is very obedient but it is not DPO tuned - so you still might need to encourage it in the system prompt as they show in the examples on the main model site on Huggingface.

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

- **Stable Diffusion**: Generates images based on output with Candle pure rust SD directly using any of the main diffusion models which are auto-loaded.
- **Pure Rust LLM with Candle**: Gemma and Mistral naive support direct with Metal GPU optimizations, no Python.
- **LLM OpenAI API Client**: with OpenAI API compatibility that is simple for use without dependencies or complexity with async threading of stream output token by token.
- **LLM Analysis of OS**: System Stats.
- **LLM Analysis of Network**: Packet Capture (MpegTS support currently).
- **CLI Support**: Uses the clap crate for an easy command-line interface.
- **Async Requests**: Built with tokio for efficient non-blocking I/O operations.
- **Configurable**: Supports environment variables and command-line options for custom requests.
- **Structured Logging**: Implements the log crate for clear and configurable logging.
- **JSON Handling**: Utilizes serde and serde_json for hassle-free data serialization and deserialization.

![RSLLM](https://storage.googleapis.com/gaib/2/rsllm.webp)

## Dependencies

- Candle Rust Transformers/Tensors for AI models <https://github.com/huggingface/candle>
- Optional: (if no Candle)  Server Llama.cpp: <https://github.com/ggerganov/llama.cpp>
- Optional: (if no Candle) GGUF Model Mixtral 8x7b: <https://huggingface.co/TheBloke/dolphin-2.7-mixtral-8x7b-GGUF>

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

Copy `.env.example` to `.env` file in the project root and add your OpenAI API key (if using OpenAI):

```bash
# .env
OPENAI_API_KEY=your_openai_api_key_here
```

To use OpenAI GPT API instead of a local LLM, you need to have an account and an API key. You can sign up for an API key [https://beta.openai.com/signup/](https://beta.openai.com/signup/).

You must alter the -llm-host option to match your server for rsllm to fit your environment. For example, if you are running llama.cpp on the same machine as rsllm, you can use the following: `--host http://127.0.0.1:8080`. For using OpenAI GPT API add --use-openai on the cmdline which will set the llm host for you to OpenAI's.

### Usage

Use the scripts in the [./scripts](./scripts/) directory.
```
./scripts/mpeg_analyzer.sh --llm-host http://your.llm.host:8080
./scripts/mpeg_poetry.sh --llm-host http://your.llm.host:8080
./scripts/system_health.sh --llm-host http://your.llm.host:8080
```

#### Command-Line Options:

```bash
cargo run --release --features ndi,metal -- -h
```

### Example:

- Using with Candle and OS Stats as a AI system analyzer.

```bash
$ cargo run --release --features ndi,metal -- \
    --use-candle --candle_llm mistral \
    --quantized \
    --max-tokens 300 \
    --temperature 0.8 \
    --ai-os-stats \
    --ndi-images \ # You need to get the NDI SDK installed first for ndi
    --system-prompt "you are helpful" \
    --query "How is my system doing?"
```

- Using with OpenAI API and OS Stats as a AI system analyzer.

```bash
$ cargo run --release -- \
    --use-openai \
    --max-tokens 300 \
    --temperature 0.8 \
    --ai-os-stats \
    --system-prompt "you are helpful" \
    --query "How is my system doing?"
```

## TODO

* Priority:
- Fix images for SD incrementally as we go.
- Text NLS chunking.

* Sooner or later:
- use ffmpeg-next-sys to process video and audio in real-time, use for generating frames/audio/text to video etc / transforming video, creating mood videos or themes and stories. Experiment to see what an LLM + FFmpeg can do together.
- Improve into a good MpegTS Analyzer for real-time analysis of mpegts streams and reporting, with AI to detect issues and report them.
- Use Perceptual Hashes DCT64 based frame fingerprints from video input to detect changes in video frames, recognize and learn repeating frames / content sequences, commercial break verification, and ad insertion detection. Plug in SCTE35 and have database of content fingerprinted to compare to and various quality checks on iput and confirmation of break/logo fidelity and presence.
- Improve network and system analyzers.
- preserve history as a small db possibly sqlite or mongodb locally. feed history into chroma db for RAG.
- use chroma db to do RAG with documents for augmenting the prompt with relevant information.
- allow daemon mode to run and listent for requests via zmq input and pass to output.
- segment output via NLP into smaller chunks for realtime processing downstream.
- fill out options for the LLM and openai api.
- capnproto for serialization and deserialization of data.
- improve stable diffusion for image generation for visualizing results in incremental steps.
- add MetaVoice via Candle (TODO, waiting on it to be avaiable, in a PR from someone) text to speech for audio output of results.
- add MetaMusic music generation for mood enhancement based on results.
- add talking head video generation with consistent frame context of objects staying same in frame.
- speech to text via Whisper Candle for audio input for llm ingestion and subtitling of video.
- freeform input options for the LLM to figure out what the user wants to do.
- dynamic code generation of python for new tasks on the fly like video processing? risks?
- iterations and multi-generational output with outlines leading to multiple passes till a final result is reached.

## License

This project is under the MIT License - see the LICENSE file for details.

## Author

Chris Kennedy, February 2024

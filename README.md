# RsLLM: Rust AI Stream Analyzer Twitch Bot

RsLLM is AI pipeline 100% in Rust for Transformer/Tensor code that is leveraging the Candle framework from Huggingface. It represents a systems programming language approach to AI model interaction and stream analysis. It can run locally on GPUs, it is focused on support of MacOS devices equipped with M1/M2/M3 ARM GPUs. This AI pipeline is designed for developers and researchers aiming to integrate local large language models (LLMs) with Rust, bypassing the need for external dependencies and Python code for chatbots and other AI programs. At its core, RsLLM emphasizes the utilization of local LLMs for generating text, images, and speech within a Rust environment, offering a robust suite of features for real-time data stream analysis and AI-driven content creation. RsLLM can run a Twitch channell chat bot and NDI video/audio with generated Stable Diffusion images and TTS Speech output through software compatible with NDI. You can setup OBS to take the NDI feed and setup your Twitch channel then have a full chatting and speaking/image generating customizable Twitch channel. Fully AI driven, you can automate a Twitch Streamer somewhat. Also you can analyze MpegTS streams or OS System stats too, if desired you can combine the two and have chat users comment and query the stream analyzing it effectively.

## Key Features

-   **Local LLM Focus**: Utilizes Candle's Rust-based LLMs, Mistral and Gemma, for direct and efficient AI interactions, prioritizing local execution to harness the full power of MacOS Metal GPUs.
-   **Comprehensive AI Analyzer**: Embeds a sophisticated AI analyzer capable of processing inputs and generating outputs across text, voice, speech, and images, facilitating a seamless flow of AI-generated content. (Work in Progress)
-   **Voice and Speech Integration**: Plans to incorporate Whisper for voice-driven interactions, akin to Alexa, allowing users to communicate with the toolkit using voice commands and receive streaming text inputs in response. (Planned Feature)
-   **Image Generation and NDI Output**: Supports generating images from text descriptions and outputting through NDI for a wide range of applications, including real-time content creation and broadcasting. (In Beta Testing)
-   **TTS Support**: Candle implements TTS using MetaVoice (default, WIP), OpenAI TTS API (high-quality, real-time), and Mimic3 TTS API (local, free). MetaVoice is being optimized for Metal GPUs, while OpenAI TTS API generates premium speech at a cost. Mimic3 TTS API requires running the mimic3-server but offers a good alternative to OpenAI TTS API. [Mimic3 GitHub](https://github.com/MycroftAI/mimic3)
-   **Twitch Chat Interactive AI**: Integrated Twitch chat for real-time AI interactions, enabling users to engage with the toolkit through chat commands and receive AI-generated responses.

![RSLLM](https://storage.googleapis.com/groovybits/images/rsllm/rsllm.webp)

## Core Components

### Candle Framework Integration

Candle, a project by Huggingface, offers Rust-native LLMs like Mistral and Gemma, optimized for Metal GPUs on MacOS. This integration facilitates local execution of LLMs, ensuring high performance and low latency in AI model interactions.

### OpenAI API Support

While RsLLM's primary focus is on running local LLMs, it also provides support for the OpenAI API, enabling users to leverage external AI models when necessary. This feature ensures versatility and broad applicability in various AI-driven projects.

### Real-time AI Analysis and Content Generation

RsLLM excels in analyzing real-time data streams and generating AI-driven content, including text, images, and speech. It aims to create a dynamic interaction model where voice inputs can be converted into text commands for the LLM, and the generated outputs can be streamed back as voice or visual content.

## Installation and Configuration

### Prerequisites

-   Ensure Rust and Cargo are installed. [Rust Installation Guide](https://www.rust-lang.org/tools/install).
-   MacOS system with an M1/M2/M3 ARM GPU.

### Setup Guide

1. **Clone the Repository**:

    ```bash
    git clone https://github.com/groovybits/rsllm.git
    ```

2. **Navigate to the Project Directory**:

    ```bash
    cd rsllm
    ```

3. **Compile with Metal GPU Support and NDI SDK support**:

    ```bash
    ./scripts/compile.sh # Script handles NDI SDK dependency and DYLD_LIBRARY_PATH
    ```

### Configuration

-   Copy `.env.example` to `.env` and customize the settings, including the OpenAI API key if intending to use external AI models.

## Usage

RsLLM is designed to facilitate a wide range of AI-driven operations, from generating text-based content to analyzing network streams and processing visual and audio inputs. Advanced features like NDI audio output and voice-to-text input processing are in development, aiming to enhance the interactive capabilities of the toolkit.

### Example Commands

-   Use the scripts in the [./scripts](./scripts/) directory.

    ```bash
    ./scripts/compile.sh # Build RsLLM
    ./scripts/mpeg_analyzer.sh
    ./scripts/mpeg_poetry.sh
    ./scripts/system_health.sh
    ```

-   **Running with Candle and OS Stats for AI System Analysis**:
    ```bash
    cargo run --release --features fonts,ndi,mps,metavoice,audioplayer -- \
      --candle_llm gemma \
      --model-id "2b-it" \
      --max-tokens 1000 \
      --temperature 0.8 \
      --ai-os-stats \
      --sd-image \
      --ndi-images \
      --ndi-audio \
      --system-prompt "You create image prompts from OS system stats health state." \
      --query "How is my system doing? Create a report on the system health as visual image descriptions."
    ```

## Enhanced Output Capabilities and Upcoming Features

### NDI Output for Images and TTS Speech Audio

RsLLM is enhancing its output capabilities to include NDI (Network Device Interface) support for images and TTS (Text-to-Speech) audio, facilitating high-quality, low-latency video streaming over IP networks.

-   **(OPTIONAL) NDI SDK Installation**: The [compile.sh](scripts/compile.sh) script will download the libndi.dylib for you. If desired, you can download and install the NDI SDK from [here](https://ndi.video/download-ndi-sdk/). This SDK is useful for viewing the NDI output and exploring other tools.
-   **Configuration Steps**:
    1. Add `--features ndi` to the Cargo build command to include NDI support in your build.
    2. Run `scripts/compile.sh`, which will retrieve the libndi.dylib that works best for MacOS.
    3. To ensure the library is correctly recognized when building with `cargo --features=ndi`, set the `DYLD_LIBRARY_PATH` environment variable:
    ```bash
    export DYLD_LIBRARY_PATH=`pwd`:$DYLD_LIBRARY_PATH
    ```
-   **Additional Configuration**: Logging into the Huggingface Hub via the CLI can resolve some potential warnings. Execute `huggingface-cli login` to authenticate.

### MetaVoice TTS Text to Speech (WIP)

Candle, our core Rust framework for LLM interaction, now includes MetaVoice, a groundbreaking Text-to-Speech (TTS) technology. As this feature improves, it will enable pure Rust-based LLM, TTI (Text-to-Image), and TTS functionalities, seamlessly integrated with Metal GPU optimizations for MacOS.

-   **Development Status**: The MetaVoice integration is done and being optimized in Candle to match the quality of the reference implementation.
-   **Anticipated Benefits**: Once fully optimized, MetaVoice will significantly enhance the toolkit's ability to generate lifelike speech from text without cost and with low latency on a local LLM. It will also provide the ability to one-shot learn a voice from a small clip and generate speech from it. For now, you may want to use OpenAI for quality/real-time generation of speech audio.

RsLLM's mission is to research and explore the implementation of a versatile, high-performance toolkit for AI-driven content creation and analysis on MacOS, leveraging the full potential of Metal GPUs and Rust's efficiency.

## Roadmap

### Priority:

-   MpegTS Chat for freeform analysis over current and historical MPEG-TS stream data.
-   Improve Image/TTS latency and async coordination of output. Use an NDI pre-queue for images and audio to ensure synchronization and minimize latency.
-   Implement RAG (Retrieval Augmented Generation) using Chromium for document caching and embeddings, providing augmented documentation-based LLM context.
-   Merge MetaVoice fixes from Candle as they become available (WIP).

### Future Enhancements:

-   Utilize ffmpeg-next-sys to process video and audio in real-time for generating frames, audio, and text-to-video, as well as transforming video and creating mood videos or themed stories. Explore the possibilities of combining an LLM with FFmpeg.
-   Enhance MpegTS Analyzer for real-time analysis of MPEG-TS streams, reporting, and AI-driven issue detection.
-   Implement Perceptual Hashes (DCT64-based) for frame fingerprinting to detect changes in video frames, recognize and learn repeating content sequences, verify commercial breaks, and detect ad insertions. Integrate SCTE-35 and maintain a database of fingerprinted content for various quality checks, break/logo fidelity confirmation, and presence detection.
-   Improve network and system analyzers.
-   Preserve history using a local database (e.g., SQLite or MongoDB) and feed it into a Chroma DB for RAG.
-   Utilize Chroma DB for RAG with documents to augment prompts with relevant information.
-   Enable daemon mode to run and listen for requests via ZeroMQ input and pass to output.
-   Expand options for LLMs and the OpenAI API.
-   Implement Cap'n Proto for serialization, deserialization, and modular ZeroMQ protocol communication.
-   Integrate MetaMusic for mood-based music generation based on results.
-   Develop talking head video generation with consistent frame context, ensuring objects remain the same within frames.
-   Implement speech-to-text using Whisper Candle for audio input, LLM ingestion, and video subtitling.
-   Allow freeform input options for the LLM to interpret user intentions.
-   Explore dynamic code generation in Python for new tasks like video processing (consider risks).
-   Implement iterative and multi-generational output with outlines leading to multiple passes until a final result is reached.
-   Utilize Speech to Text with Whisper Candle for audio input, enabling voice commands to the LLM for conversational AI.

## Contributing

Contributions are warmly welcomed, especially in areas such as feature development, performance optimization, and documentation. Your expertise can significantly enhance the toolkit's capabilities and user experience.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for detailed information.

## Acknowledgments

-   Candle Rust Transformers/Tensors by Huggingface: [Candle](https://github.com/huggingface/candle)
-   OpenAI for API Specifications: [OpenAI](https://openai.com/)
-   OpenAI for TTS Integration: [OpenAI](https://openai.com/)
-   MetaVoice for TTS Integration: [MetaVoice](https://metavoice.com/)
-   Mimic3 for TTS Integration: [Mimic3](https://github.com/MycroftAI/mimic3)
-   Whisper for Speech to Text: [Whisper](https://openai.com/research/whisper)
-   Google Gemini LLM: [Gemini](https://ai.googleblog.com/2023/02/introducing-gemini-language-model-for.html)
-   Mistral LLM: [Mistral](https://huggingface.co/mistralai/Mistral-7B-v0.1)

## Author

Chris Kennedy, leading the development of innovative AI solutions with the MacOS Metal GPU Rust LLM Toolkit. February 2024.

We are committed to pushing the boundaries of AI integration with Video Technology and Multimodal input/output on MacOS Arm CPUs in pure Rust, ensuring media developers and researchers have access to powerful, efficient, and versatile tools for their AI-driven projects.

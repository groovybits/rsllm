# Rust LLM Stream Analyzer and Content Generator

The Rust LLM Stream Analyzer and Content Generator is optimized for MacOS Metal GPUs leveraging the Candle framework from Huggingface, represents a cutting-edge approach to AI model interaction and stream analysis on MacOS devices equipped with M1/M2/M3 ARM GPUs. This toolkit is meticulously designed for developers and researchers aiming to integrate local large language models (LLMs) with high efficiency, bypassing the need for external dependencies and Python servers. At its core, it emphasizes the utilization of local LLMs for generating text, images, and speech within a Rust environment, offering a robust suite of features for real-time data stream analysis and AI-driven content creation.

## Highlighted Features

- **Local LLM Focus**: Utilizes Candle's Rust-based LLMs, Mistral and Gemma, for direct and efficient AI interactions, prioritizing local execution to harness the full power of MacOS Metal GPUs.
- **Comprehensive AI Analyzer**: Embeds a sophisticated AI analyzer capable of processing inputs and generating outputs across text, voice, speech, and images, facilitating a seamless flow of AI-generated content. (Work in Progress)
- **Voice and Speech Integration**: Plans to incorporate Whisper for voice-driven interactions, akin to Alexa, allowing users to communicate with the toolkit using voice commands and receive streaming text inputs in response. (Planned Feature)
- **Image Generation and NDI Output**: Supports generating images from text descriptions and outputting through NDI for a wide range of applications, including real-time content creation and broadcasting. (In Beta Testing)

![RSLLM](https://storage.googleapis.com/gaib/2/rsllm.webp)

## Core Components

### Candle Framework Integration
Candle, a project by Huggingface, offers Rust-native LLMs like Mistral and Gemma, optimized for Metal GPUs on MacOS. This integration facilitates local execution of LLMs, ensuring high performance and low latency in AI model interactions.

### OpenAI API Support
While the toolkit’s primary focus is on running local LLMs, it also provides support for the OpenAI API, enabling users to leverage external AI models when necessary. This feature ensures versatility and broad applicability in various AI-driven projects.

### Real-time AI Analysis and Content Generation
The toolkit excels in analyzing real-time data streams and generating AI-driven content, including text, images, and speech. It aims to create a dynamic interaction model where voice inputs can be converted into text commands for the LLM, and the generated outputs can be streamed back as voice or visual content.

## Installation and Configuration

### Prerequisites
- Ensure Rust and Cargo are installed. [Installation Guide](https://www.rust-lang.org/tools/install).
- MacOS system with M1/M2/M3 ARM GPU.

### Setup Guide
1. **Clone the Repository**:
   ```bash
   git clone https://github.com/groovybits/rsllm.git
   ```

2. **Navigate to the Project Directory**:
   ```bash
   cd rsllm
   ```

3. **Compile with Metal GPU Support**:
   ```bash
   cargo build --release --features=metal,ndi
   ```

### Configuration
- Copy `.env.example` to `.env` and customize the settings, including the OpenAI API key if intending to use external AI models.

## Usage

The toolkit is designed to facilitate a wide range of AI-driven operations, from generating text-based content to analyzing network streams and processing visual and audio inputs. Advanced features like NDI audio output and voice-to-text input processing are in development, aiming to enhance the interactive capabilities of the toolkit.

### Example Commands

- Use the scripts in the [./scripts](./scripts/) directory.
    ```bash
    ./scripts/mpeg_analyzer.sh
    ./scripts/mpeg_poetry.sh
    ./scripts/system_health.sh
    ```

- **Running with Candle and OS Stats for AI System Analysis**:
   ```bash
   cargo run --release --features ndi,metal -- \
     --use-candle --candle_llm mistral \
     --quantized \
     --max-tokens 300 \
     --temperature 0.8 \
     --ai-os-stats \
     --ndi-images \
     --system-prompt "you are helpful" \
     --query "How is my system doing?"
   ```

## Enhanced Output Capabilities and Upcoming Features

### NDI Output for Images and TTS Speech Audio (Audio is a Work in Progress)

The toolkit is enhancing its output capabilities to include NDI (Network Device Interface) support for images and TTS (Text-to-Speech) audio, facilitating high-quality, low-latency video streaming over IP networks. To leverage these capabilities, the NDI SDK is required:

- **NDI SDK Installation**: Download and install the NDI SDK from [here](https://ndi.video/download-ndi-sdk/). This SDK is essential for enabling NDI output functionalities within the toolkit.
- **Configuration Steps**:
    1. Add `--features ndi` to the Cargo build command to include NDI support in your build.
    2. Obtain the NDI Core Suite from [NDI Tools](https://ndi.video/tools/ndi-core-suite/).
    3. Copy the `libndi.dynlib` file into your `./rsllm/` and `/usr/local/lib` directory for easy accessibility. For instance, you can use the command:
    ```bash
    sudo cp "/Applications/NDI Video Monitor.app/Contents/Frameworks/libndi_advanced.dylib" "./rsllm/libndi.dylib"
    ```
    4. To ensure the library is correctly recognized when building with `cargo --features=ndi`, set the `DYLD_LIBRARY_PATH` environment variable:
    ```bash
    export DYLD_LIBRARY_PATH=/usr/local/lib:./:$DYLD_LIBRARY_PATH # include ./ for rsllm directory current path
    ```
- **Additional Configuration**: Logging into the Huggingface Hub via the CLI can resolve some potential warnings. Execute `huggingface-cli login` to authenticate.

These steps aim to streamline the process of setting up NDI output capabilities, despite the complexities associated with handling NDI SDK libraries.

### MetaVoice TTS Text to Speech (Planned Feature)

Candle, our core Rust framework for LLM interaction, is poised to introduce support for MetaVoice, a groundbreaking Text-to-Speech (TTS) technology. This upcoming feature will enable pure Rust-based LLM, TTI (Text-to-Image), and TTS functionalities, seamlessly integrated with Metal GPU optimizations for MacOS.

- **Development Status**: The MetaVoice integration is currently in the works, with a pull request under review within the Candle project. For the latest updates, refer to the PR [here](https://github.com/huggingface/candle/compare/main...metavoice).
- **Anticipated Benefits**: Once deployed, MetaVoice will significantly enhance the toolkit's ability to generate lifelike speech from text, opening new avenues for interactive applications and content creation.

These enhancements and planned features underscore our commitment to providing a versatile, high-performance toolkit for AI-driven content creation and analysis on MacOS, leveraging the full potential of Metal GPUs and Rust's efficiency.

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
- Speech to Tet with Whisper Candle for audio input for sending commands to the LLM for conversational AI.

## Contributing

Contributions are warmly welcomed, especially in areas such as feature development, performance optimization, and documentation. Your expertise can significantly enhance the toolkit's capabilities and user experience.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for detailed information.

## Acknowledgments

- Candle Rust Transformers/Tensors by Huggingface: [Candle](https://github.com/huggingface/candle)
- NDI SDK for Image Output: [NDI SDK](https://ndi.video/download-ndi-sdk/)
- OpenAI for API Specifications: [OpenAI](https://openai.com/)
- MetaVoice for TTS Integration: [MetaVoice](https://metavoice.com/)
- Whisper for Speech to Text: [Whisper](https://whisper.com/)
- Google Gemma LLM: [Gemma](https://huggingface.co/blog/gemma#prompt-format)
- Mistral LLM: [Mistral](https://huggingface.co/mistralai/Mistral-7B-v0.1)

## Author

Chris Kennedy, leading the development of innovative AI solutions with the MacOS Metal GPU Rust LLM Toolkit. February 2024.

We are committed to pushing the boundaries of AI integration with Video Technology and Multimodal input/output on MacOS Arm CPUs in pure Rust, ensuring media developers and researchers have access to powerful, efficient, and versatile tools for their AI-driven projects.

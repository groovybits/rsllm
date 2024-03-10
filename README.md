# Rust LLM Stream Analyzer and Content Generator

The Rust LLM Stream Analyzer and Content Generator is optimized for MacOS Metal GPUs leveraging the Candle framework from Huggingface, represents a cutting-edge approach to AI model interaction and stream analysis on MacOS devices equipped with M1/M2/M3 ARM GPUs. This toolkit is meticulously designed for developers and researchers aiming to integrate local large language models (LLMs) with high efficiency, bypassing the need for external dependencies and Python servers. At its core, it emphasizes the utilization of local LLMs for generating text, images, and speech within a Rust environment, offering a robust suite of features for real-time data stream analysis and AI-driven content creation.

## Highlighted Features

-   **Local LLM Focus**: Utilizes Candle's Rust-based LLMs, Mistral and Gemma, for direct and efficient AI interactions, prioritizing local execution to harness the full power of MacOS Metal GPUs.
-   **Comprehensive AI Analyzer**: Embeds a sophisticated AI analyzer capable of processing inputs and generating outputs across text, voice, speech, and images, facilitating a seamless flow of AI-generated content. (Work in Progress)
-   **Voice and Speech Integration**: Plans to incorporate Whisper for voice-driven interactions, akin to Alexa, allowing users to communicate with the toolkit using voice commands and receive streaming text inputs in response. (Planned Feature)
-   **Image Generation and NDI Output**: Supports generating images from text descriptions and outputting through NDI for a wide range of applications, including real-time content creation and broadcasting. (In Beta Testing)
-   **TTS MetaVoice / Mimic3 TTS API / OpenAI TTS API**: Candle implements TTS using MetaVoice which is the default but a WIP as the author is shoring up the implementation quality and optimizing for Metal GPUs (isn't realtime currently, sounds very "wavy"). OpenAI TTS API support generates very nice speech for a prie if wanting quality/realtime speech generation. Mimic3 TTS API requires running the mimic3-server and is a bit more involved but is a good alternative to OpenAI TTS API since it is free if you have a local system. <https://github.com/MycroftAI/mimic3>

![RSLLM](https://storage.googleapis.com/groovybits/images/rsllm/rsllm.webp)

## Core Components

### Candle Framework Integration

Candle, a project by Huggingface, offers Rust-native LLMs like Mistral and Gemma, optimized for Metal GPUs on MacOS. This integration facilitates local execution of LLMs, ensuring high performance and low latency in AI model interactions.

### OpenAI API Support

While the toolkit’s primary focus is on running local LLMs, it also provides support for the OpenAI API, enabling users to leverage external AI models when necessary. This feature ensures versatility and broad applicability in various AI-driven projects.

### Real-time AI Analysis and Content Generation

The toolkit excels in analyzing real-time data streams and generating AI-driven content, including text, images, and speech. It aims to create a dynamic interaction model where voice inputs can be converted into text commands for the LLM, and the generated outputs can be streamed back as voice or visual content.

## Installation and Configuration

### Prerequisites

-   Ensure Rust and Cargo are installed. [Installation Guide](https://www.rust-lang.org/tools/install).
-   MacOS system with M1/M2/M3 ARM GPU.

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
    # export DYLD_LIBRARY_PATH=`pwd`:$DYLD_LIBRARY_PATH
    # cargo build --release --features=metal,ndi
    ## run the compile.sh which basically does the above command + gets the libndi.dylib.
    ./scripts/compile.sh # Script helps handle the NDI SDK dependency and DYLD_LIBRARY_PATH
    ```

### Configuration

-   Copy `.env.example` to `.env` and customize the settings, including the OpenAI API key if intending to use external AI models.

## Usage

The toolkit is designed to facilitate a wide range of AI-driven operations, from generating text-based content to analyzing network streams and processing visual and audio inputs. Advanced features like NDI audio output and voice-to-text input processing are in development, aiming to enhance the interactive capabilities of the toolkit.

### Example Commands

-   Use the scripts in the [./scripts](./scripts/) directory.

    ```bash
    [./scripts/compile.sh](./scripts/compile.sh) # build rsllm

    ./scripts/mpeg_analyzer.sh
    ./scripts/mpeg_poetry.sh
    ./scripts/system_health.sh
    ```

-   **Running with Candle and OS Stats for AI System Analysis**:
    ```bash
    cargo run --release --features ndi,metal -- \
      --candle_llm gemma \
      --model-id "2b-it" \
      --max-tokens 1000 \
      --temperature 0.8 \
      --ai-os-stats \
      --sd-image \
      --ndi-images \
      --ndi-audio \
      --system-prompt "you create image prompts from os system stats health state." \
      --query "How is my system doing? Create a report on the system health as visual image descriptions."
    ```

## Enhanced Output Capabilities and Upcoming Features

### NDI Output for Images and TTS Speech Audio

The toolkit is enhancing its output capabilities to include NDI (Network Device Interface) support for images and TTS (Text-to-Speech) audio, facilitating high-quality, low-latency video streaming over IP networks.

-   **(OPTIONAL) NDI SDK Installation**: The [compile.sh](scripts/compile.sh] script will download hte libndi.dylib for you. If you want to, you can Download and install the NDI SDK from [here](https://ndi.video/download-ndi-sdk/). This SDK is useful for viewing the NDI output and other tools to explore.
-   **Configuration Steps**:
    1. Add `--features ndi` to the Cargo build command to include NDI support in your build.
    2. Run scripts/compile.sh which will retreive the libndi.dylib which works best for MacOS.
    ````
    3. To ensure the library is correctly recognized when building with `cargo --features=ndi`, set the `DYLD_LIBRARY_PATH` environment variable:
    ```bash
    export DYLD_LIBRARY_PATH=`pwd`:$DYLD_LIBRARY_PATH
    ````
-   **Additional Configuration**: Logging into the Huggingface Hub via the CLI can resolve some potential warnings. Execute `huggingface-cli login` to authenticate.

### MetaVoice TTS Text to Speech (WIP)

Candle, our core Rust framework for LLM interaction has MetaVoice now, a groundbreaking Text-to-Speech (TTS) technology. As this feature improves it will enable pure Rust-based LLM, TTI (Text-to-Image), and TTS functionalities, seamlessly integrated with Metal GPU optimizations for MacOS.

-   **Development Status**: The MetaVoice integration is done and being optimized in Candle so that quality matches the reference implementation.
-   **Anticipated Benefits**: Once full quality/optimized, MetaVoice will significantly enhance the toolkit's ability to generate lifelike speech from text without costing and low latency on a local LLM. Also it will give the ability to one shot learn a voice from a small clip and generate speech from it. For now you may want to use OpenAI for quality/realtime generation of the speech audio.

RsLLM has a mission to research and explore implementing a versatile, high-performance toolkit for AI-driven content creation and analysis on MacOS, leveraging the full potential of Metal GPUs and Rust's efficiency.

## TODO

### Priority:

-   Twitch Chat integration fixes for threading and input/output through mpsc channels async (WIP).
-   MpegTS Chat for analysis freeform over current and historical mpegts streams data.
-   Improve Image/TTS Latency and async cooridaation of output. Use an NDI pre-Queue for images and audio to ensure they are in sync and non-latent.
-   RAG document chromium use and caching of embeddings for augmented documentation based LLM context.
-   Merge fixes for Metavoice as they are done in Candle - WIP.

### Sooner or later:

-   use ffmpeg-next-sys to process video and audio in real-time, use for generating frames/audio/text to video etc / transforming video, creating mood videos or themes and stories. Experiment to see what an LLM + FFmpeg can do together.
-   Improve into a good MpegTS Analyzer for real-time analysis of mpegts streams and reporting, with AI to detect issues and report them.
-   Use Perceptual Hashes DCT64 based frame fingerprints from video input to detect changes in video frames, recognize and learn repeating frames / content sequences, commercial break verification, and ad insertion detection. Plug in SCTE35 and have database of content fingerprinted to compare to and various quality checks on iput and confirmation of break/logo fidelity and presence.
-   Improve network and system analyzers.
-   preserve history as a small db possibly sqlite or mongodb locally. feed history into chroma db for RAG.
-   use chroma db to do RAG with documents for augmenting the prompt with relevant information.
-   allow daemon mode to run and listent for requests via zmq input and pass to output.
-   fill out options for the LLM and openai api.
-   capnproto for serialization and deserialization of data with modular zmq protocol communication.
-   add MetaMusic music generation for mood enhancement based on results.
-   add talking head video generation with consistent frame context of objects staying same in frame.
-   speech to text via Whisper Candle for audio input for llm ingestion and subtitling of video.
-   freeform input options for the LLM to figure out what the user wants to do.
-   dynamic code generation of python for new tasks on the fly like video processing? risks?
-   iterations and multi-generational output with outlines leading to multiple passes till a final result is reached.
-   Speech to Text with Whisper Candle for audio input for sending commands to the LLM for conversational AI.

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
-   Whisper for Speech to Text: [Whisper](https://whisper.com/)
-   Google Gemma LLM: [Gemma](https://huggingface.co/blog/gemma#prompt-format)
-   Mistral LLM: [Mistral](https://huggingface.co/mistralai/Mistral-7B-v0.1)

## Author

Chris Kennedy, leading the development of innovative AI solutions with the MacOS Metal GPU Rust LLM Toolkit. February 2024.

We are committed to pushing the boundaries of AI integration with Video Technology and Multimodal input/output on MacOS Arm CPUs in pure Rust, ensuring media developers and researchers have access to powerful, efficient, and versatile tools for their AI-driven projects.

## rsllm - LLM OpenAI API for chat completions in Rust

Simple rust program that can use an llm using the OpenAI specifications.

```
cargo build
target/debug/rsllm
```

# RsLLM OpenAI API client

A Rust-based client for interacting with the OpenAI API, designed to send prompts and receive responses asynchronously, displaying them in the console. Ideal for developers and researchers integrating AI responses into Rust applications or exploring OpenAI's capabilities programmatically.

![RSLLM](https://storage.googleapis.com/gaib/2/rsllm.webp)

## Features

- **CLI Support**: Uses the clap crate for an easy command-line interface.
- **Async Requests**: Built with tokio for efficient non-blocking I/O operations.
- **Configurable**: Supports environment variables and command-line options for custom requests.
- **Structured Logging**: Implements the log crate for clear and configurable logging.
- **JSON Handling**: Utilizes serde and serde_json for hassle-free data serialization and deserialization.

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
    git clone https://github.com/your-repository/RsLLM-OpenAI-API-client.git
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

Based on the provided NAL dump, we can derive several statistics:

1. The MPEG-TS Packet is 88 bytes in length.
2. There are no headers or other metadata included.
3. The NAL unit types found within this packet include:
   - IDR_W_RADL (0x01): Indicates an IDR picture with a random access point, meaning it can be used for decoding starting from any frame without requiring prior frames.
   - VPS (Video Parameter Set) (0x02): Contains information about the video encoding parameters such as resolution, frame rate, and other settings.

Note that these statistics are based on the NAL unit types present within the packet and not actual video content analysis. Further examination of the video content itself would be required to determine more specific details about its contents or quality.
--
Index 0 ID chatcmpl-xdlMWl3qBAWSsJ1QcKJ9pdOU7cViEtOS
Object chat.completion.chunk by Model gpt-3.5-turbo
Created on 2024-02-04 15:29:06 Finish reason: stop
Tokens 186 Bytes 794
--
```

## License

This project is under the MIT License - see the LICENSE file for details.

## Author

Chris Kennedy, February 2024

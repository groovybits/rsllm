## rsllm - LLM OpenAI API for chat completions in Rust

Simple rust program that can use an llm using the OpenAI specifications.

```
cargo build
target/debug/rsllm
```

# RsLLM OpenAI API client

A Rust-based client for interacting with the OpenAI API, designed to send prompts and receive responses asynchronously, displaying them in the console. Ideal for developers and researchers integrating AI responses into Rust applications or exploring OpenAI's capabilities programmatically.

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

- `--system-prompt` to set the system's initial prompt, defaulting to "You are an assistant who is helpful."
- `--query` for the prompt you wish to send to the OpenAI API.
- `--temperature` for controlling randomness in the response, defaulting to 0.8.
- `--top_p` for response diversity control, defaulting to 1.0.
- `--presence_penalty` and `--frequency_penalty` for context and repetition control, both defaulting to 0.0.
- `--max_tokens` to set the maximum number of tokens to generate, defaulting to 800.
- `--stream` for streaming the response, defaulted to true.
- `--model` to choose the model, defaulting to "gpt-3.5-turbo".

### Example

    cargo run -- --query "Explain the significance of Rust's ownership system" --max_tokens 300

## License

This project is under the MIT License - see the LICENSE file for details.

## Author

Chris Kennedy, February 2024

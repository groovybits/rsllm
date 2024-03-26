use crate::args::Args;
use crate::candle_gemma::gemma;
use crate::candle_mistral::mistral;
use anyhow::Result;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{self};

pub async fn daemon(
    nick: String,
    token: String,
    channel: Vec<String>,
    running: Arc<AtomicBool>,
    twitch_tx: mpsc::Sender<String>,
    args: Args,
) -> Result<()> {
    let credentials = match Some(nick).zip(Some(token)) {
        Some((nick, token)) => tmi::client::Credentials::new(nick, token),
        None => tmi::client::Credentials::anon(),
    };

    let channels = channel
        .into_iter()
        .map(tmi::Channel::parse)
        .collect::<Result<Vec<_>, _>>()?;

    log::info!("Connecting as {}", credentials.nick);
    let mut client = tmi::Client::builder()
        .credentials(credentials)
        .connect()
        .await?;

    client.join_all(&channels).await?;
    log::info!("Joined the following channels: {}", channels.join(", "));

    run(client, channels, running, twitch_tx, args).await
}

async fn run(
    mut client: tmi::Client,
    channels: Vec<tmi::Channel>,
    running: Arc<AtomicBool>,
    twitch_tx: mpsc::Sender<String>,
    args: Args,
) -> Result<()> {
    let mut chat_messages = Vec::new();
    // create a semaphore so no more than one message is sent to the AI at a time
    let semaphore = tokio::sync::Semaphore::new(args.twitch_llm_concurrency as usize);
    while running.load(Ordering::SeqCst) {
        let msg = client.recv().await?;

        match msg.as_typed()? {
            tmi::Message::Privmsg(msg) => {
                // acquire the semaphore to send a message to the AI
                let _chat_lock = semaphore.acquire().await.unwrap();
                on_msg(
                    &mut client,
                    msg,
                    &twitch_tx,
                    &mut chat_messages,
                    args.clone(),
                )
                .await?
            }
            tmi::Message::Reconnect => {
                client.reconnect().await?;
                client.join_all(&channels).await?;
            }
            tmi::Message::Ping(ping) => client.pong(&ping).await?,
            _ => {}
        };
    }
    Ok(())
}

async fn on_msg(
    client: &mut tmi::Client,
    msg: tmi::Privmsg<'_>,
    tx: &mpsc::Sender<String>,
    chat_messages: &mut Vec<String>,
    args: Args,
) -> Result<()> {
    log::debug!("\nTwitch Message: {:?}", msg);
    log::info!(
        "Twitch Message from {}: {}",
        msg.sender().name(),
        msg.text()
    );

    if client.credentials().is_anon() {
        return Ok(());
    }

    // send message to the LLM and get an answer to send back to the user.
    // also send the message to the main LLM loop to keep history context of the conversation
    if !msg.text().starts_with("!help") && !msg.text().starts_with("!message") {
        // LLM Thread
        let (external_sender, mut external_receiver) = tokio::sync::mpsc::channel::<String>(100);
        let max_tokens = args.twitch_max_tokens;
        let temperature = 0.8;
        let quantized = false;
        let max_messages = args.twitch_chat_history;

        let system_start_token = if args.twitch_model == "gemma" {
            "<start_of_turn>"
        } else {
            "<<SYS>>"
        };

        let system_end_token = if args.twitch_model == "gemma" {
            "<end_of_turn>"
        } else {
            "<</SYS>>"
        };

        let assistant_start_token = if args.twitch_model == "gemma" {
            "<start_of_turn>"
        } else {
            ""
        };

        let assistant_end_token = if args.twitch_model == "gemma" {
            "<end_of_turn>"
        } else {
            ""
        };

        let start_token = if args.twitch_model == "gemma" {
            "<start_of_turn>"
        } else {
            "[INST]"
        };

        let end_token = if args.twitch_model == "gemma" {
            "<end_of_turn>"
        } else {
            "[/INST]"
        };

        let bos_token = if args.twitch_model == "gemma" {
            ""
        } else {
            "<s>"
        };

        let eos_token = if args.twitch_model == "gemma" {
            ""
        } else {
            "</s>"
        };

        let user_name = if args.twitch_model == "gemma" {
            "user"
        } else {
            ""
        };

        let assistant_name = if args.twitch_model == "gemma" {
            "model"
        } else {
            ""
        };

        // Truncate the chat_messages array to 3 messages max messages
        if chat_messages.len() > max_messages {
            chat_messages.truncate(max_messages);
        }

        // build a string out of the chat_messages array of strings
        // that have each message in the format <s><start_of_turn>user {message}<end_of_turn></s>
        let mut chat_messages_history = String::new();
        for message in chat_messages.iter() {
            chat_messages_history.push_str(&format!("{}", message));
        }

        // Send message to the AI through mpsc channels format to model specs
        let msg_text = format!(
            "{}{}{} {}{}{}{}{}{}{}{} twitch chat user {} asked {}{}{}{} ",
            bos_token,
            system_start_token,
            assistant_name,
            args.twitch_prompt.clone(),
            system_end_token,
            eos_token,
            bos_token,
            chat_messages_history,
            bos_token,
            start_token,
            user_name,
            msg.sender().name(),
            msg.text().to_string(),
            end_token,
            assistant_start_token,
            assistant_name,
        ); // Clone the message text

        println!("\nTwitch sending msg_text:\n{}\n", msg_text);

        let llm_thread = if args.twitch_model == "gemma" {
            tokio::spawn(async move {
                if let Err(e) = gemma(
                    msg_text,
                    max_tokens,
                    temperature,
                    quantized,
                    Some("2b-it".to_string()),
                    external_sender,
                ) {
                    eprintln!("Error running twitch gemma: {}", e);
                }
            })
        } else if args.twitch_model == "mistral" {
            tokio::spawn(async move {
                if let Err(e) = mistral(
                    msg_text,
                    max_tokens,
                    temperature,
                    quantized,
                    Some("auto".to_string()),
                    external_sender,
                ) {
                    eprintln!("Error running twitch mistral: {}", e);
                }
            })
        } else {
            // print message and error out
            eprintln!(
                "Error: Invalid model specified for twitch chat {}",
                args.twitch_model
            );
            tokio::spawn(async move {
                external_sender
                    .send("Error: Invalid model specified for twitch chat".to_string())
                    .await
                    .unwrap();
            })
        };

        // thread token collection and wait for it to finish
        let token_thread = tokio::spawn(async move {
            let mut tokens = String::new();
            while let Some(received) = external_receiver.recv().await {
                tokens.push_str(&received);
            }
            tokens
        });

        // wait for llm thread to finish
        llm_thread.await?;

        let answer = token_thread.await?;

        // remove all new lines from answer:
        let answer = answer.replace("\n", " ");

        println!("\nTwitch received answer:\n{}\n", answer);

        // truncate to 500 characters and remove any urls
        let answer = answer
            .chars()
            .take(500)
            .collect::<String>()
            .replace("http", "hxxp");

        // Send message to the twitch channel
        client
            .privmsg(msg.channel(), &format!("{}", answer.clone(),))
            .reply_to(msg.message_id())
            .send()
            .await?;

        // add message to the chat_messages history of strings
        let full_message = format!(
            "{}{}{} {} asked {}{}{}{} {}{}{}",
            bos_token,
            start_token,
            user_name,
            msg.sender().name(),
            msg.text().to_string(),
            end_token,
            assistant_start_token,
            assistant_name,
            answer.clone(),
            assistant_end_token,
            eos_token
        );
        chat_messages.push(full_message);

        // Send message to the main loop through mpsc channels
        tx.send(format!(
            "!chat {} said {}",
            msg.sender().name(),
            msg.text().to_string()
        ))
        .await?;

        return Ok(());
    }

    if msg.text().starts_with("!message") {
        let message = msg.text().splitn(2, ' ').nth(1).unwrap_or("");

        std::io::stdout().flush().unwrap();
        log::info!(
            "Twitch recieved an LLM message from {}: {}",
            msg.sender().name(),
            message
        );
        std::io::stdout().flush().unwrap();

        // Send message to the LLM through mpsc channels
        tx.send(format!(
            "!message {} said {}",
            msg.sender().name(),
            message.to_string()
        ))
        .await?;

        client
            .privmsg(
                msg.channel(),
                &format!(
                    "Thank you for your message {}. I will speak about it in a moment!",
                    msg.sender().name()
                ),
            )
            .reply_to(msg.message_id())
            .send()
            .await?;

        return Ok(());
    }

    std::io::stdout().flush().unwrap();
    log::info!(
        "Twitch recieved a help message from {}",
        msg.sender().name()
    );
    std::io::stdout().flush().unwrap();

    client
        .privmsg(
            msg.channel(),
            "To send a message to Alice type !message Alice <question>. You can also conversate with me by free typing in the chat! Enjoy the stories!",
        )
        .reply_to(msg.message_id())
        .send()
        .await?;

    Ok(())
}

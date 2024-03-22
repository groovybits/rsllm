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

    run(client, channels, running, twitch_tx).await
}

async fn run(
    mut client: tmi::Client,
    channels: Vec<tmi::Channel>,
    running: Arc<AtomicBool>,
    twitch_tx: mpsc::Sender<String>,
) -> Result<()> {
    while running.load(Ordering::SeqCst) {
        let msg = client.recv().await?;
        match msg.as_typed()? {
            tmi::Message::Privmsg(msg) => on_msg(&mut client, msg, &twitch_tx).await?,
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

    if !msg.text().starts_with("!help") && !msg.text().starts_with("!message") {
        // Send message to the LLM through mpsc channels
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
                &format!("Thank you for your message {}. I will speak about it in a moment!", msg.sender().name()),
                /*"Currently building a Rust based system which will be way better with realtime streaming my chat! Right now I am a WIP experimental Rust based AI with Candle. Will allow chat input again within a few days, enjoy the stories.",*/
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
            "Sorry for the lack of interactive messaging, for now please chat with me directly in chat :)", // How to use the chat: !help, !message <message>.",
        )
        .reply_to(msg.message_id())
        .send()
        .await?;

    Ok(())
}

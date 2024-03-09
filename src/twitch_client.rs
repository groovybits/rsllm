use anyhow::Result;
use tokio::select;

pub async fn setup(nick: String, token: String, channel: Vec<String>) -> Result<()> {
    let credentials = match Some(nick).zip(Some(token)) {
        Some((nick, token)) => tmi::client::Credentials::new(nick, token),
        None => tmi::client::Credentials::anon(),
    };

    let channels = channel
        .into_iter()
        .map(tmi::Channel::parse)
        .collect::<Result<Vec<_>, _>>()?;

    println!("Connecting as {}", credentials.nick);
    let mut client = tmi::Client::builder()
        .credentials(credentials)
        .connect()
        .await?;

    client.join_all(&channels).await?;
    println!("Joined the following channels: {}", channels.join(", "));

    run(client, channels).await
}

async fn run(mut client: tmi::Client, channels: Vec<tmi::Channel>) -> Result<()> {
    loop {
        let msg = client.recv().await?;
        match msg.as_typed()? {
            tmi::Message::Privmsg(msg) => on_msg(&mut client, msg).await?,
            tmi::Message::Reconnect => {
                client.reconnect().await?;
                client.join_all(&channels).await?;
            }
            tmi::Message::Ping(ping) => client.pong(&ping).await?,
            _ => {}
        };
    }
}

async fn on_msg(client: &mut tmi::Client, msg: tmi::Privmsg<'_>) -> Result<()> {
    println!("\nTwitch Message: {:?}", msg);
    log::info!(
        "Twitch Message from {}: {}",
        msg.sender().name(),
        msg.text()
    );

    if client.credentials().is_anon() {
        return Ok(());
    }

    if !msg.text().starts_with("!help") && !msg.text().starts_with("!message") {
        return Ok(());
    }

    if msg.text().starts_with("!message") {
        let message = msg.text().splitn(2, ' ').nth(1).unwrap_or("");
        // TODO: send message to the LLM through mpsc channels
        log::info!(
            "Twitch recieved an LLM message from {}: {}",
            msg.sender().name(),
            message
        );

        client
            .privmsg(
                msg.channel(),
                "Currently playing experimental Rust based AI with Google Gemma. Will be back to regular chat soon, enjoy the stories.",
            )
            .reply_to(msg.message_id())
            .send()
            .await?;

        return Ok(());
    }

    log::info!(
        "Twitch recieved a help message from {}",
        msg.sender().name()
    );

    client
        .privmsg(
            msg.channel(),
            "How to use the chat: !help, !message <message> (Currently under construction, will be back to regular chat soon).",
        )
        .reply_to(msg.message_id())
        .send()
        .await?;

    Ok(())
}

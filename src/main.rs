use clear_urls_bot::{
    bot,
    config::Config,
    db::Db,
    logging,
    sanitizer::{AiEngine, RuleEngine},
};
use std::time::Duration;
use teloxide::Bot;
use tokio::time::interval;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init_logging();

    let pid = std::process::id();
    tracing::info!(pid = %pid, "Avvio ClearURLs Bot");

    let config = Config::from_env()?;
    config.validate()?;

    let db = Db::new(&config.database_url).await?;
    let rules = RuleEngine::new_lazy(&config.clearurls_source);
    let ai = AiEngine::new(&config);

    // Create a custom reqwest client with a longer timeout for Telegram polling
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;
    let bot = Bot::with_client(&config.bot_token, client);

    // Canale per eventi real-time (SSE) - kept for bot logic, though not used in GraphQL yet
    let (event_tx, _) = tokio::sync::broadcast::channel::<serde_json::Value>(100);

        let admin_id = config.admin_id;
        let bot_clone = bot.clone();
        let panic_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            panic_hook(info);
            if admin_id != 0 {
                let msg = format!("[PANIC] {}", info);
                let _ = tokio::spawn(bot_clone.send_message(ChatId(admin_id), msg));
            }
        }));
        let bot_task = tokio::spawn(bot::run_bot(
        bot,
        db.clone(),
        rules.clone(),
        ai,
        config.clone(),
        event_tx.clone(),
    ));

    let rules_refresh = rules.clone();
    let refresh_task = tokio::spawn(async move {
        if let Err(e) = rules_refresh.refresh().await {
            tracing::error!("Errore nel download iniziale delle regole: {}", e);
        }
        let mut interval = interval(Duration::from_secs(86400));
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(e) = rules_refresh.refresh().await {
                tracing::error!("Errore durante l'aggiornamento delle regole: {}", e);
            }
        }
    });

    tokio::select! {
        res = bot_task => tracing::error!("Task bot terminato: {:?}", res),
        res = refresh_task => tracing::error!("Task aggiornamento terminato: {:?}", res),
    }

    Ok(())
}

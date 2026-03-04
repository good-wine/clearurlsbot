use clear_urls_bot::{
    bot,
    config::Config,
    db::Db,
    logging,
    sanitizer::{AiEngine, RuleEngine},
};
use std::time::Duration;
use teloxide::prelude::*;
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

    // Check VirusTotal configuration
    if let Ok(vt_key) = std::env::var("VIRUSTOTAL_API_KEY") {
        if !vt_key.is_empty() && vt_key != "your_virustotal_api_key_here" {
            tracing::info!("✅ VirusTotal: Scansione malware ABILITATA");
            let vt_alert_only = std::env::var("VIRUSTOTAL_ALERT_ONLY")
                .ok()
                .map(|value| {
                    let normalized = value.trim().to_ascii_lowercase();
                    !matches!(normalized.as_str(), "0" | "false" | "no" | "off")
                })
                .unwrap_or(true);
            if vt_alert_only {
                tracing::info!("✅ VirusTotal: modalità SOLO ALLERTA attiva (default)");
            } else {
                tracing::info!("ℹ️  VirusTotal: modalità report completa attiva");
            }
        } else {
            tracing::info!(
                "⚠️  VirusTotal: Scansione malware DISABILITATA (API key non configurata)"
            );
        }
    } else {
        tracing::info!("⚠️  VirusTotal: Scansione malware DISABILITATA (API key non configurata)");
    }

    // Check URLScan.io configuration
    if let Ok(us_key) = std::env::var("URLSCAN_API_KEY") {
        if !us_key.is_empty() && us_key != "your_urlscan_api_key_here" {
            tracing::info!("✅ URLScan.io: Scansione web reputation ABILITATA");
            let us_alert_only = std::env::var("URLSCAN_ALERT_ONLY")
                .ok()
                .map(|value| {
                    let normalized = value.trim().to_ascii_lowercase();
                    !matches!(normalized.as_str(), "0" | "false" | "no" | "off")
                })
                .unwrap_or(true);
            if us_alert_only {
                tracing::info!("✅ URLScan.io: modalità SOLO ALLERTA attiva (default)");
            } else {
                tracing::info!("ℹ️  URLScan.io: modalità report completa attiva");
            }
        } else {
            tracing::info!(
                "⚠️  URLScan.io: Scansione web reputation DISABILITATA (API key non configurata)"
            );
        }
    } else {
        tracing::info!(
            "⚠️  URLScan.io: Scansione web reputation DISABILITATA (API key non configurata)"
        );
    }

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
            let bot_for_panic = bot_clone.clone();
            let _ = std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let _ = bot_for_panic.send_message(ChatId(admin_id), msg).await;
                });
            });
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

use crate::{
    db::{Db, models::UserConfig},
    i18n,
    sanitizer::{AiEngine, RuleEngine},
    security::{RATE_LIMITER, sanitize_input},
};
use regex::Regex;
use teloxide::prelude::*;
use teloxide::types::{
    CallbackQuery, ChosenInlineResult, InlineKeyboardButton, InlineKeyboardMarkup, InlineQuery,
    InlineQueryResult, InlineQueryResultArticle, InputMessageContent, InputMessageContentText,
    KeyboardButton, KeyboardMarkup, KeyboardRemove, MessageEntityKind, MessageId, ParseMode,
    ReplyParameters,
};
use teloxide::utils::html;
use whatlang::{Lang, detect};

const MAX_MESSAGE_LENGTH: usize = 4000;

pub async fn run_bot(
    bot: Bot,
    db: Db,
    rules: RuleEngine,
    ai: AiEngine,
    config: crate::config::Config,
    event_tx: tokio::sync::broadcast::Sender<serde_json::Value>,
) {
    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle_message))
        .branch(Update::filter_edited_message().endpoint(handle_edited_message))
        .branch(Update::filter_inline_query().endpoint(handle_inline_query))
        .branch(Update::filter_chosen_inline_result().endpoint(handle_chosen_inline_result))
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![db, rules, ai, config, event_tx])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

#[tracing::instrument(skip(bot, db, rules, ai, config), fields(user_id = %q.from.id.0))]
pub async fn handle_inline_query(
    bot: Bot,
    q: InlineQuery,
    db: Db,
    rules: RuleEngine,
    ai: AiEngine,
    config: crate::config::Config,
) -> ResponseResult<()> {
    let user_id = i64::try_from(q.from.id.0).unwrap_or(0);
    // Rate limiting anti-flood
    if !RATE_LIMITER.check(user_id) {
        return Ok(()); // Silenziosamente ignora richieste flood
    }
    let query = sanitize_input(q.query.trim());
    let lang_code = get_user_language(&db, user_id, q.from.language_code.as_deref()).await;

    if query.is_empty() {
        let article = InlineQueryResultArticle::new(
            "inline-help",
            if lang_code == "it" {
                "Incolla un URL da pulire"
            } else {
                "Paste a URL to clean"
            },
            InputMessageContent::Text(InputMessageContentText::new(if lang_code == "it" {
                "Incolla un URL dopo @botusername per pulirlo in linea."
            } else {
                "Paste a URL after @botusername to clean it inline."
            })),
        );

        bot.answer_inline_query(q.id, vec![InlineQueryResult::Article(article)])
            .cache_time(1)
            .is_personal(true)
            .await?;
        return Ok(());
    }

    let user_config = db.get_user_config(user_id).await.unwrap_or_default();
    let ignored_domains: Vec<String> = user_config
        .ignored_domains
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    let custom_rules = db.get_custom_rules(user_id).await.unwrap_or_default();

    let urls = extract_url_candidates(&query);
    let mut ranked_cleaned: Vec<(usize, String, String, usize)> = Vec::new();

    for (idx, original) in urls.iter().enumerate() {
        let expanded = rules.expand_url(original).await;
        let mut final_url = expanded.clone();

        if let Some((cleaned, _provider)) = rules.sanitize(&expanded, &custom_rules, &ignored_domains)
        {
            final_url = cleaned;
        }

        if user_config.is_ai_enabled() && config.ai_api_key.is_some() {
            if let Ok(Some(ai_cleaned)) = ai.sanitize(&final_url).await {
                final_url = ai_cleaned;
            }
        }

        if final_url == *original {
            continue;
        }

        let removed_params = removed_query_params_count(&expanded, &final_url);
        ranked_cleaned.push((idx, original.clone(), final_url, removed_params));
    }

    ranked_cleaned.sort_by(|a, b| {
        b.3.cmp(&a.3)
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut results: Vec<InlineQueryResult> = Vec::new();

    for (rank, (_source_idx, _original, cleaned, removed_params)) in ranked_cleaned
        .iter()
        .take(config.inline_max_results)
        .enumerate()
    {
        let title = if lang_code == "it" {
            if *removed_params > 0 {
                format!("URL pulito #{} (−{} param)", rank + 1, removed_params)
            } else {
                format!("URL pulito #{}", rank + 1)
            }
        } else if *removed_params > 0 {
            format!("Clean URL #{} (-{} params)", rank + 1, removed_params)
        } else {
            format!("Clean URL #{}", rank + 1)
        };

        let content = InputMessageContent::Text(InputMessageContentText::new(cleaned.clone()));
        let article = InlineQueryResultArticle::new(format!("clean-{}", rank), title, content)
            .description(cleaned.clone());

        results.push(InlineQueryResult::Article(article));
    }

    if results.is_empty() {
        let article = InlineQueryResultArticle::new(
            "inline-no-results",
            if lang_code == "it" {
                "Nessun URL da pulire"
            } else {
                "No cleanable URL found"
            },
            InputMessageContent::Text(InputMessageContentText::new(query.to_string())),
        );
        results.push(InlineQueryResult::Article(article));
    }

    bot.answer_inline_query(q.id, results)
        .cache_time(1)
        .is_personal(true)
        .await?;

    Ok(())
}

#[tracing::instrument(skip(_bot), fields(user_id = %chosen.from.id.0))]
pub async fn handle_chosen_inline_result(
    _bot: Bot,
    chosen: ChosenInlineResult,
) -> ResponseResult<()> {
    tracing::info!(
        user_id = chosen.from.id.0,
        result_id = %chosen.result_id,
        query = %chosen.query,
        "Risultato inline selezionato"
    );
    Ok(())
}

#[tracing::instrument(
    skip(bot, db, rules, ai, config, event_tx),
    fields(chat_id = %msg.chat.id, user_id)
)]
pub async fn handle_edited_message(
    bot: Bot,
    msg: Message,
    db: Db,
    rules: RuleEngine,
    ai: AiEngine,
    config: crate::config::Config,
    event_tx: tokio::sync::broadcast::Sender<serde_json::Value>,
) -> ResponseResult<()> {
    tracing::info!(chat_id = %msg.chat.id, msg_id = %msg.id, "Elaborazione messaggio modificato");
    handle_message(bot, msg, db, rules, ai, config, event_tx).await
}

#[tracing::instrument(
    skip(bot, db, rules, ai, config, event_tx),
    fields(chat_id = %msg.chat.id, user_id)
)]
#[allow(clippy::too_many_lines)]
pub async fn handle_message(
    bot: Bot,
    msg: Message,
    db: Db,
    rules: RuleEngine,
    ai: AiEngine,
    config: crate::config::Config,
    event_tx: tokio::sync::broadcast::Sender<serde_json::Value>,
) -> ResponseResult<()> {
    let user_id = msg.from.as_ref().map(|u| i64::try_from(u.id.0).unwrap_or(0)).unwrap_or(0);
    let chat_id = msg.chat.id;
    let msg_text = msg.text().map(|t| t.to_string()).unwrap_or_default();
    let msg_clone = msg.clone();
    let user_config = db.get_user_config(user_id).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "Errore nel recupero config utente, uso default");
        if user_id != config.admin_id && config.admin_id != 0 {
            let admin_chat = ChatId(config.admin_id);
            let admin_msg = format!("[CRITICAL] Errore DB per user {}: {}", user_id, e);
            let bot_clone = bot.clone();
            tokio::spawn(async move {
                let _ = bot_clone.send_message(admin_chat, admin_msg).await;
            });
        }
        UserConfig::default()
    });

    let entities = msg.entities();
    let text = msg_text.as_str();

    // Detect language
    let detected_lang = if text.is_empty() {
        None
    } else {
        detect(text).map(|info| info.lang())
    };

    let telegram_lang = msg.from.as_ref().and_then(|u| u.language_code.as_deref());

    let lang_code = match (detected_lang, telegram_lang) {
        (Some(Lang::Ita), _) => "it",
        (Some(Lang::Eng), _) => "en",
        (_, Some(l)) if l.starts_with("it") => "it",
        (_, Some(l)) if l.starts_with("en") => "en",
        _ => &user_config.language,
    };

    // Save detected language to database if different from current
    if lang_code != user_config.language.as_str() {
        let mut updated_config = user_config.clone();
        updated_config.language = lang_code.to_string();
        if let Err(e) = db.save_user_config(&updated_config).await {
            tracing::warn!(error = %e, "Errore nel salvataggio lingua utente");
        } else {
            tracing::debug!(
                user_id = user_id,
                old_lang = %user_config.language,
                new_lang = lang_code,
                "Preferenza lingua utente aggiornata"
            );
        }
    }

    let tr = i18n::get_translations(lang_code);

    let mut has_urls = false;
    if let Some(_e) = entities.as_ref() {
        // Handle Commands
        if msg_text.starts_with('/') {
            let cmd_parts: Vec<&str> = msg_text.split('@').collect();
            let cmd = cmd_parts[0];
            let _is_private = msg.chat.is_private();
            let bot_username = config.bot_username.to_lowercase();

            let is_targeted = if cmd_parts.len() > 1 {
                cmd_parts[1].to_lowercase().starts_with(&bot_username)
            } else {
                true
            };

            match cmd {
                "/start" => {
                    tokio::spawn({
                        let bot = bot.clone();
                        let tr = tr.clone();
                        let chat_id = chat_id;
                        let user_id = user_id;
                        async move {
                            let _ = bot.send_message(chat_id, tr.welcome.replace("{}", &user_id.to_string()))
                                .parse_mode(ParseMode::Html)
                                .await;
                        }
                    });
                }
                "/help" => {
                    tokio::spawn({
                        let bot = bot.clone();
                        let tr = tr.clone();
                        let chat_id = chat_id;
                        async move {
                            let _ = bot.send_message(chat_id, tr.help_text)
                                .parse_mode(ParseMode::Html)
                                .await;
                        }
                    });
                }
                "/menu" => {
                    tokio::spawn({
                        let bot = bot.clone();
                        let tr = tr.clone();
                        let chat_id = chat_id;
                        async move {
                            let _ = bot.send_message(chat_id, tr.reply_keyboard_opened)
                                .reply_markup(main_reply_keyboard(&tr))
                                .parse_mode(ParseMode::Html)
                                .await;
                        }
                    });
                }
                "/hidekbd" => {
                    tokio::spawn({
                        let bot = bot.clone();
                        let tr = tr.clone();
                        let chat_id = chat_id;
                        async move {
                            let _ = bot.send_message(chat_id, tr.reply_keyboard_hidden)
                                .reply_markup(KeyboardRemove::new())
                                .parse_mode(ParseMode::Html)
                                .await;
                        }
                    });
                }
                "/settings" => {
                    handle_settings_callback(
                        bot.clone(),
                        chat_id,
                        None,
                        user_id,
                        db.clone(),
                        config.clone(),
                        &tr,
                    )
                    .await.ok();
                }
                "/language" => {
                    let mut msg_text = String::from("<b>Lingue disponibili:</b>\n\n");
                    msg_text.push_str("🇮🇹 Italiano (/setlang it)\n🇬🇧 English (/setlang en)\n");
                    bot.send_message(chat_id, msg_text)
                        .parse_mode(ParseMode::Html)
                        .await.ok();
                }
                "/setlang" => {
                    let parts: Vec<&str> = msg_text.split_whitespace().collect();
                    if parts.len() > 1 {
                        let lang = parts[1];
                        let mut updated_config = user_config.clone();
                        updated_config.language = lang.to_string();
                        db.save_user_config(&updated_config).await.ok();
                        let tr_new = i18n::get_translations(lang);
                        bot.send_message(chat_id, tr_new.s_language_updated)
                            .parse_mode(ParseMode::Html)
                            .await.ok();
                    } else {
                        bot.send_message(chat_id, "❓ Specifica la lingua: /setlang it oppure /setlang en")
                            .parse_mode(ParseMode::Html)
                            .await.ok();
                    }
                }
                "/stats" => {
                    let warning = "Nessun dato disponibile";
                    let stats_text = "Statistiche non disponibili";
                    bot.send_message(chat_id, warning).await.ok();
                    bot.send_message(chat_id, stats_text)
                        .parse_mode(ParseMode::Html)
                        .await.ok();
                }
                "/topusers" => {
                    let top = db.get_top_users(10).await.unwrap_or_default();
                    let mut msg_text = String::from("<b>Top utenti per link puliti:</b>\n\n");
                    for (idx, (uid, count)) in top.iter().enumerate() {
                        msg_text.push_str(&format!("{}. <code>{}</code> — <b>{}</b>\n", idx+1, uid, count));
                    }
                    bot.send_message(chat_id, msg_text)
                        .parse_mode(ParseMode::Html)
                        .await.ok();
                }
                "/toplinks" => {
                    let top = db.get_top_links(10).await.unwrap_or_default();
                    let mut msg_text = String::from("<b>Top link puliti:</b>\n\n");
                    for (idx, (url, count)) in top.iter().enumerate() {
                        msg_text.push_str(&format!("{}. <code>{}</code> — <b>{}</b>\n", idx+1, url, count));
                    }
                    bot.send_message(chat_id, msg_text)
                        .parse_mode(ParseMode::Html)
                        .await.ok();
                }
                _ => {
                    bot.send_message(chat_id, tr.unknown_command)
                        .parse_mode(ParseMode::Html)
                        .await.ok();
                }
            }
            has_urls = true;
        }
    }

    if let Some(text_val) = msg.text() {
        if let Some(action) = quick_reply_action(text_val, &tr) {
            match action {
                QuickReplyAction::Settings => {
                    handle_settings_callback(
                        bot.clone(),
                        chat_id,
                        None,
                        user_id,
                        db.clone(),
                        config.clone(),
                        &tr,
                    )
                    .await?;
                    return Ok(());
                }
                QuickReplyAction::Stats => {
                    let stats_text = tr
                        .stats_text
                        .replace("{}", &user_config.cleaned_count.to_string());
                    bot.send_message(chat_id, stats_text)
                        .parse_mode(ParseMode::Html)
                        .reply_markup(quick_actions_inline_keyboard(&tr, user_id))
                        .await?;
                    return Ok(());
                }
                QuickReplyAction::Help => {
                    bot.send_message(chat_id, tr.help_text)
                        .parse_mode(ParseMode::Html)
                        .reply_markup(quick_actions_inline_keyboard(&tr, user_id))
                        .await?;
                    return Ok(());
                }
                QuickReplyAction::HideKeyboard => {
                    bot.send_message(chat_id, tr.reply_keyboard_hidden)
                        .reply_markup(KeyboardRemove::new())
                        .await?;
                    return Ok(());
                }
                QuickReplyAction::Language => {
                    let language_text = format!(
                        "<b>{}</b>\n\n{} <b>{}</b>",
                        tr.s_language_title,
                        tr.s_language_current,
                        user_config.language
                    );
                    bot.send_message(chat_id, language_text)
                        .parse_mode(ParseMode::Html)
                        .reply_markup(language_inline_keyboard(&tr, user_id))
                        .await?;
                    return Ok(());
                }
            }
        }
    }

    // Persist/Update chat info
    let is_group_context = msg_clone.chat.is_group() || msg_clone.chat.is_supergroup() || msg_clone.chat.is_channel();
    let mut chat_config = db
        .get_chat_config_or_default(chat_id.0)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "Errore nel recupero config chat, uso default");
            crate::db::models::ChatConfig::default()
        });

    if is_group_context {
        let title = msg_clone.chat.title().map(ToString::to_string);
        let chat_config_db = db.get_chat_config(chat_id.0).await.unwrap_or(None);
        let chat_exists = chat_config_db.is_some();

        // Only save if it's new or title changed
        if !chat_exists || chat_config.title != title {
            chat_config.title = title.clone();
            if !chat_exists {
                chat_config.added_by = user_id;
            }
            let _ = db.save_chat_config(&chat_config).await;
        }

        if !chat_exists && user_id != 0 && has_urls {
            let notify_text = tr.group_activated.replace(
                "{}",
                &html::escape(&title.clone().unwrap_or_else(|| tr.unknown.to_string())),
            );
            let _ = bot
                .send_message(ChatId(user_id), notify_text)
                .parse_mode(ParseMode::Html)
                .await;
        }
    }

    if !has_urls {
        return Ok(());
    }

    // Logic: In groups, only check if the group enabled the bot.
    // In private, check if the user enabled the bot.
    let is_enabled = if is_group_context {
        chat_config.is_enabled()
    } else {
        user_config.is_enabled()
    };

    if !is_enabled {
        tracing::info!(is_group_context, chat_id = %chat_id, "Bot disattivato per questo contesto (skip)");
        return Ok(());
    }

    let ignored_domains: Vec<String> = user_config
        .ignored_domains
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let custom_rules = db.get_custom_rules(user_id).await.unwrap_or_default();
    let msg_id = msg.id;
    let mut cleaned_urls = Vec::new();

    let mut url_candidates = Vec::new();

    // 1. Get URLs from Telegram Entities
    if let Some(ents) = entities {
        let utf16: Vec<u16> = text.encode_utf16().collect();
        for entity in ents {
            let url_str = match &entity.kind {
                MessageEntityKind::Url => {
                    let start = entity.offset;
                    let end = start + entity.length;
                    if end > utf16.len() {
                        continue;
                    }
                    String::from_utf16_lossy(&utf16[start..end])
                }
                MessageEntityKind::TextLink { url } => url.to_string(),
                _ => continue,
            };
            if !url_candidates.contains(&url_str) {
                tracing::debug!(url = %url_str, "URL trovato tramite entita' Telegram");
                url_candidates.push(url_str);
            }
        }
    }

    // 2. Supplement with Regex Detection
    let url_pattern = r"(?i)(?:https?://|www\.)[a-zA-Z0-9\-\.]+\.[a-zA-Z]{2,}(?:/[^\s]*)?";
    if let Ok(re) = Regex::new(url_pattern) {
        for mat in re.find_iter(text) {
            let url_str = mat.as_str().to_string();
            if !url_candidates.contains(&url_str) {
                tracing::debug!(url = %url_str, "URL trovato tramite regex di fallback");
                url_candidates.push(url_str);
            }
        }
    }

    if url_candidates.is_empty() {
        tracing::debug!("Nessun URL candidato trovato nel messaggio");
        return Ok(());
    }

    // 3. Process candidates
    for url_str in url_candidates {
                // VirusTotal check
                if let Some(warning) = check_url_virustotal(&url_str).await {
                    bot.send_message(chat_id, warning).await.ok();
                }
        // 1. Expand shortened URLs first
        let expanded_url = rules.expand_url(&url_str).await;
        let original_url_str = url_str.clone();
        let mut current_url = expanded_url.clone();
        // Caching: se già pulito, usa cache
        if let Some(cached) = URL_CACHE.get(&expanded_url).await {
            current_url = cached;
            cleaned_urls.push((original_url_str, current_url.clone(), "CACHE".to_string()));
            continue;
        }
        // 2. Sanitization
        if let Some((cleaned, provider)) =
            rules.sanitize(&current_url, &custom_rules, &ignored_domains)
        {
            current_url = cleaned;
            tracing::info!(provider = %provider, "URL pulito dal motore");

            if user_config.is_ai_enabled() && config.ai_api_key.is_some() {
                if let Ok(Some(ai_cleaned)) = ai.sanitize(&current_url).await {
                    current_url = ai_cleaned;
                    let provider_name = format!("AI ({provider})");
                    URL_CACHE.insert(expanded_url.clone(), current_url.clone());
                    cleaned_urls.push((original_url_str, current_url, provider_name));
                    continue;
                }
            }

            tracing::info!(
                original = %rules.redact_sensitive(&original_url_str),
                cleaned = %current_url,
                provider = %provider,
                "URL pulito dal motore"
            );
            URL_CACHE.insert(expanded_url.clone(), current_url.clone());
            cleaned_urls.push((original_url_str, current_url, provider));
        } else {
            tracing::debug!(url = %rules.redact_sensitive(&current_url), "URL gia' pulito");
            if user_config.is_ai_enabled() && config.ai_api_key.is_some() {
                if let Ok(Some(ai_cleaned)) = ai.sanitize(&current_url).await {
                    tracing::info!("URL pulito da fallback AI");
                    URL_CACHE.insert(expanded_url.clone(), ai_cleaned.clone());
                    cleaned_urls.push((original_url_str, ai_cleaned, "AI (Deep Scan)".to_string()));
                }
            }
        }
    }

    if cleaned_urls.is_empty() {
        tracing::info!("Elaborazione completata: nessun URL da pulire");
        return Ok(());
    }

    let _ = db
        .increment_cleaned_count(user_id, cleaned_urls.len() as i64)
        .await;
    for (orig, clean, prov) in &cleaned_urls {
        let _ = db.log_cleaned_link(user_id, orig, clean, prov).await;

        let _ = event_tx.send(serde_json::json!({
            "user_id": user_id,
            "original_url": orig,
            "cleaned_url": clean,
            "provider_name": prov,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        }));
    }

    let mode = match chat_config.mode.as_str() {
        "default" | "" => user_config.mode.clone(),
        m => m.to_string(),
    };

    if mode == "delete" && bot.delete_message(chat_id, msg_id).await.is_ok() {
        let user_name = msg_clone
            .from
            .as_ref()
            .map_or_else(|| tr.fallback_user.to_string(), |u| u.first_name.clone());
        let mut response = tr.cleaned_for.replace("{}", &html::escape(&user_name));
        for (_, cleaned, _) in &cleaned_urls {
            let escaped = html::escape(cleaned);
            response.push_str(&format!("• <a href=\"{escaped}\">{escaped}</a>\n"));
        }
        bot.send_message(chat_id, response)
            .parse_mode(ParseMode::Html)
            .await?;
        return Ok(());
    }

    let mut response = if is_group_context {
        let user_name = msg_clone
            .from
            .as_ref()
            .map_or_else(|| tr.fallback_user.to_string(), |u| u.first_name.clone());
        tr.cleaned_for.replace("{}", &html::escape(&user_name))
    } else {
        String::from(tr.cleaned_links)
    };

    if !response.ends_with('\n') {
        response.push('\n');
    }

    if cleaned_urls.len() == 1 {
        let clean = cleaned_urls[0].1.trim();
        let escaped_url = html::escape(clean);
        let link_entry = format!("<a href=\"{escaped_url}\">{escaped_url}</a>");

        if response.len() + link_entry.len() < MAX_MESSAGE_LENGTH {
            response.push_str(&link_entry);
        }
    } else {
        for (_, cleaned, _) in &cleaned_urls {
            let clean = cleaned.trim();
            let escaped_url = html::escape(clean);
            let link_entry = format!("• <a href=\"{escaped_url}\">{escaped_url}</a>\n");

            if response.len() + link_entry.len() > MAX_MESSAGE_LENGTH {
                response.push_str(tr.truncated);
                break;
            }
            response.push_str(&link_entry);
        }
    }

    tracing::info!(chat_id = %chat_id, "Invio risposta con URL puliti");

    let mut request = bot
        .send_message(chat_id, response)
        .reply_parameters(ReplyParameters::new(msg_id))
        .parse_mode(ParseMode::Html);

    // Support for Supergroup topics/threads
    if let Some(thread_id) = msg_clone.thread_id {
        request = request.message_thread_id(thread_id);
    }

    if let Err(e) = request.await {
        tracing::error!(chat_id = %chat_id, error = %e, "Errore nell'invio della risposta con URL puliti");
        return Err(e);
    }

    Ok(())
}

use moka::future::Cache;
pub static URL_CACHE: once_cell::sync::Lazy<Cache<String, String>> = once_cell::sync::Lazy::new(|| Cache::new(10000));

pub async fn check_url_virustotal(url: &str) -> Option<String> {
    let api_key = std::env::var("VIRUSTOTAL_API_KEY").ok()?;
    let client = reqwest::Client::new();
    let resp = client.get("https://www.virustotal.com/api/v3/urls")
        .header("x-apikey", api_key)
        .query(&[("url", url)])
        .send().await.ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    let verdict = json["data"]["attributes"]["last_analysis_stats"]["malicious"].as_i64().unwrap_or(0);
    if verdict > 0 {
        Some(format!("⚠️ VirusTotal: il link {} è sospetto!", url))
    } else {
        None
    }
}

/// Handles the `/start` command.
///
/// # Errors
/// Returns an error if message sending fails.
async fn handle_start_command(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    tr: &crate::i18n::Translations,
    _config: &crate::config::Config,
    message_id: Option<MessageId>,
) -> ResponseResult<()> {
    let welcome_text = tr.welcome.replace("{}", &user_id.to_string());

    // Create inline keyboard with settings button
    let keyboard = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(tr.start_open_settings, format!("settings:{}", user_id)),
        InlineKeyboardButton::callback(tr.start_view_stats, format!("user_setting:stats:{}", user_id)),
    ]]);

    upsert_settings_view(
        &bot,
        chat_id,
        message_id,
        welcome_text,
        Some(keyboard),
        true,
    )
    .await?;

    if message_id.is_none() {
        bot.send_message(chat_id, tr.reply_keyboard_opened)
            .reply_markup(main_reply_keyboard(tr))
            .await?;
    }

    Ok(())
}

/// Helper function to get user's preferred language.
///
/// Retrieves language from user configuration or Telegram language code.
async fn get_user_language(
    db: &Db,
    user_id: i64,
    telegram_lang: Option<&str>,
) -> &'static str {
    // Try to get user config from database
    if let Ok(cfg) = db.get_user_config(user_id).await {
        if !cfg.language.is_empty() && cfg.language != "en" {
            // Return 'it' if language is Italian, otherwise default to 'en'
            if cfg.language == "it" || cfg.language.starts_with("it") {
                return "it";
            }
        }
    }
    
    // Fallback to Telegram language
    if let Some(l) = telegram_lang {
        if l.starts_with("it") {
            return "it";
        }
    }
    
    // Default to English
    "en"
}

/// Handles callback query from inline keyboard buttons.
///
/// # Errors
/// Returns an error if callback query handling fails.
#[tracing::instrument(skip(bot, db, config), fields(user_id, chat_id))]
async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    db: Db,
    config: crate::config::Config,
) -> ResponseResult<()> {
    let user_id = q.from.id.0 as i64;
    // Rate limiting anti-flood anche sulle callback
    if !RATE_LIMITER.check(user_id) {
        return Ok(());
    }
    let callback_data = sanitize_input(q.data.as_deref().unwrap_or(""));
    let chat_id = q
        .message
        .as_ref()
        .map(teloxide::types::MaybeInaccessibleMessage::chat)
        .map(|chat| chat.id);
    // message_id deve essere sempre propagato per editare il messaggio originale
    let message_id = q
        .message
        .as_ref()
        .map(teloxide::types::MaybeInaccessibleMessage::id);

    // Get user's preferred language
    let telegram_lang = q.from.language_code.as_deref();
    let lang_code = get_user_language(&db, user_id, telegram_lang).await;
    let tr = crate::i18n::get_translations(lang_code);

    if let Some(chat_id) = chat_id {
        if callback_data.starts_with("settings:") {
            let parts: Vec<&str> = callback_data.split(':').collect();
            let target_user_id = callback_target_user_id(&parts, user_id);
            if target_user_id != user_id {
                show_no_permission_view(&bot, chat_id, message_id, &tr).await?;
            } else {
                handle_settings_callback(
                    bot.clone(),
                    chat_id,
                    message_id,
                    user_id,
                    db,
                    config,
                    &tr,
                )
                .await?;
            }
        } else if callback_data.starts_with("user_setting:") {
            handle_user_settings_callback(
                bot.clone(),
                chat_id,
                message_id,
                user_id,
                &callback_data,
                db,
                &tr,
            )
            .await?;
        } else if callback_data.starts_with("admin_setting:") {
            handle_admin_settings_callback(
                bot.clone(),
                chat_id,
                message_id,
                user_id,
                &callback_data,
                db,
                &config,
                &tr,
            )
            .await?;
        } else if callback_data.starts_with("quick:") {
            handle_quick_callback(
                bot.clone(),
                chat_id,
                message_id,
                user_id,
                &callback_data,
                db,
                config,
                &tr,
            )
            .await?;
        } else if callback_data.starts_with("back_to_main") {
            let parts: Vec<&str> = callback_data.split(':').collect();
            let target_user_id = callback_target_user_id(&parts, user_id);
            if target_user_id != user_id {
                show_no_permission_view(&bot, chat_id, message_id, &tr).await?;
            } else {
                handle_start_command(
                    bot.clone(),
                    chat_id,
                    user_id,
                    &tr,
                    &config,
                    message_id,
                )
                .await?;
            }
        }
    }

    // Answer callback to remove loading state
    bot.answer_callback_query(q.id).await?;

    Ok(())
}

async fn handle_settings_callback(
    // message_id deve essere sempre quello della callback per editare il messaggio
    bot: Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    user_id: i64,
    db: Db,
    config: crate::config::Config,
    tr: &crate::i18n::Translations,
) -> ResponseResult<()> {
    let _user_config = db.get_user_config(user_id).await.unwrap_or_default();
    let is_admin = user_id == config.admin_id;

    let role = if is_admin {
        tr.s_role_admin
    } else {
        tr.s_role_user
    };

    let settings_text = format!(
        "<b>{}</b>\n\n{}: <code>{}</code>\n{}: {}",
        tr.s_menu_title,
        tr.s_user_id,
        user_id,
        tr.s_role,
        role
    );

    let mut keyboard_rows = vec![
        vec![
            InlineKeyboardButton::callback(
                tr.s_notifications,
                format!("user_setting:notifications:{}", user_id),
            ),
            InlineKeyboardButton::callback(
                tr.s_ai_settings,
                format!("user_setting:ai:{}", user_id),
            ),
        ],
        vec![
            InlineKeyboardButton::callback(
                tr.s_privacy,
                format!("user_setting:privacy:{}", user_id),
            ),
            InlineKeyboardButton::callback(
                tr.s_link_processing,
                format!("user_setting:links:{}", user_id),
            ),
        ],
        vec![InlineKeyboardButton::callback(
            tr.s_language,
            format!("user_setting:language:{}", user_id),
        )],
    ];

    // Add admin options if user is admin
    if is_admin {
        keyboard_rows.push(vec![
            InlineKeyboardButton::callback(
                tr.s_admin_panel,
                format!("admin_setting:panel:{}", user_id),
            ),
            InlineKeyboardButton::callback(
                tr.s_statistics,
                format!("admin_setting:stats:{}", user_id),
            ),
        ]);
    }

    keyboard_rows.push(vec![InlineKeyboardButton::callback(
        tr.s_back_to_main,
        format!("back_to_main:{}", user_id),
    )]);

    let keyboard = InlineKeyboardMarkup::new(keyboard_rows);

    // Risposta atomica: una sola edit/send
    upsert_settings_view(&bot, chat_id, message_id, settings_text, Some(keyboard), true).await
}

async fn handle_quick_callback(
    bot: Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    user_id: i64,
    callback_data: &str,
    db: Db,
    config: crate::config::Config,
    tr: &crate::i18n::Translations,
) -> ResponseResult<()> {
    let parts: Vec<&str> = callback_data.split(':').collect();
    if parts.len() < 3 {
        return Ok(());
    }

    let target_user_id = callback_target_user_id(&parts, user_id);
    if target_user_id != user_id {
        show_no_permission_view(&bot, chat_id, message_id, tr).await?;
        return Ok(());
    }

    match parts[1] {
        "settings" => {
            handle_settings_callback(bot, chat_id, message_id, user_id, db, config, tr).await
        }
        "stats" => {
            let user_config = db.get_user_config(user_id).await.unwrap_or_default();
            let stats_text = tr
                .stats_text
                .replace("{}", &user_config.cleaned_count.to_string());
            upsert_settings_view(
                &bot,
                chat_id,
                message_id,
                stats_text,
                Some(quick_actions_inline_keyboard(tr, user_id)),
                true,
            )
            .await
        }
        "help" => {
            upsert_settings_view(
                &bot,
                chat_id,
                message_id,
                tr.help_text.to_string(),
                Some(quick_actions_inline_keyboard(tr, user_id)),
                true,
            )
            .await
        }
        "language" => {
            let user_config = db.get_user_config(user_id).await.unwrap_or_default();
            let language_text = format!(
                "<b>{}</b>\n\n{} <b>{}</b>",
                tr.s_language_title,
                tr.s_language_current,
                user_config.language
            );
            upsert_settings_view(
                &bot,
                chat_id,
                message_id,
                language_text,
                Some(language_inline_keyboard(tr, user_id)),
                true,
            )
            .await
        }
        _ => Ok(())
    }
}

async fn handle_user_settings_callback(
    // message_id deve essere sempre quello della callback per editare il messaggio
    bot: Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    user_id: i64,
    callback_data: &str,
    db: Db,
    tr: &crate::i18n::Translations,
) -> ResponseResult<()> {
    let parts: Vec<&str> = callback_data.split(':').collect();
    if parts.len() < 3 {
        return Ok(());
    }

    let target_user_id = callback_target_user_id(&parts, user_id);
    if target_user_id != user_id {
        show_no_permission_view(&bot, chat_id, message_id, tr).await?;
        return Ok(());
    }

    let setting_type = parts[1];
    let user_config: crate::db::models::UserConfig =
        db.get_user_config(user_id).await.unwrap_or_default();

    let (message_text, keyboard) = match setting_type {
        "notifications" => (
            format!("<b>{}</b>\n\n{}", tr.s_notif_title, tr.s_notif_desc),
            InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback(tr.s_enabled, format!("user_setting:toggle:notif:1:{}", user_id)),
                    InlineKeyboardButton::callback(tr.s_disabled, format!("user_setting:toggle:notif:0:{}", user_id)),
                ],
                vec![InlineKeyboardButton::callback(tr.s_back, format!("settings:{}", user_id))],
            ])
        ),
        "ai" => {
            let ai_status = if user_config.is_ai_enabled() { 
                tr.s_ai_status_enabled 
            } else { 
                tr.s_ai_status_disabled 
            };
            let message = format!(
                "<b>{}</b>\n\n{} <b>{}</b>\n\n{}",
                tr.s_ai_title,
                tr.s_ai_current_status,
                ai_status,
                tr.s_ai_desc
            );
            let keyboard = InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback(tr.s_toggle_ai, format!("user_setting:toggle:ai:{}", user_id)),
                ],
                vec![InlineKeyboardButton::callback(tr.s_back, format!("settings:{}", user_id))],
            ]);
            (message, keyboard)
        },
        "privacy" => (
            format!("<b>{}</b>\n\n{}", tr.s_privacy_title, tr.s_privacy_desc),
            InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback(tr.s_clear_history, format!("user_setting:clear_history:{}", user_id)),
                ],
                vec![InlineKeyboardButton::callback(tr.s_back, format!("settings:{}", user_id))],
            ])
        ),
        "links" => {
            let mode_label = match user_config.mode.as_str() {
                "reply" => tr.s_reply_mode,
                "delete" => tr.s_delete_mode,
                _ => user_config.mode.as_str(),
            };
            let message = format!(
                "<b>{}</b>\n\n{}: <b>{}</b>\n\n{}",
                tr.s_links_title,
                tr.s_action_mode,
                mode_label,
                tr.s_links_desc
            );
            let keyboard = InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback(tr.s_reply_mode, format!("user_setting:set_mode:reply:{}", user_id)),
                    InlineKeyboardButton::callback(tr.s_delete_mode, format!("user_setting:set_mode:delete:{}", user_id)),
                ],
                vec![InlineKeyboardButton::callback(tr.s_back, format!("settings:{}", user_id))],
            ]);
            (message, keyboard)
        },
        "language" => {
            let message = format!(
                "<b>{}</b>\n\n{} <b>{}</b>",
                tr.s_language_title,
                tr.s_language_current,
                user_config.language
            );
            let keyboard = language_inline_keyboard(tr, user_id);
            (message, keyboard)
        }
        "lang" if parts.len() >= 4 => {
            let language = parts[2];
            let mut updated = user_config.clone();
            let mut ok = true;
            if language == "it" || language == "en" {
                updated.language = language.to_string();
                if let Err(e) = db.save_user_config(&updated).await {
                    tracing::error!(error = %e, user_id, "Errore nel salvataggio lingua");
                    ok = false;
                }
            }

            let text = if ok {
                format!("{}: <b>{}</b>", tr.s_language_updated, updated.language)
            } else {
                tr.s_setting_update_failed.to_string()
            };
            let keyboard = settings_back_keyboard(tr, user_id);
            (text, keyboard)
        }
        "stats" => {
            let stats_text = tr
                .stats_text
                .replace("{}", &user_config.cleaned_count.to_string());
            let keyboard = settings_back_keyboard(tr, user_id);
            (stats_text, keyboard)
        }
        "set_mode" if parts.len() >= 4 => {
            let mode = parts[2];
            let mut mode_save_ok = true;
            if mode == "reply" || mode == "delete" {
                let mut updated = user_config.clone();
                updated.mode = mode.to_string();
                if let Err(e) = db.save_user_config(&updated).await {
                    tracing::error!(error = %e, user_id, "Errore nel salvataggio modalita' link");
                    mode_save_ok = false;
                }
            }

            let refreshed = db.get_user_config(user_id).await.unwrap_or_default();
            let mode_label = match refreshed.mode.as_str() {
                "reply" => tr.s_reply_mode,
                "delete" => tr.s_delete_mode,
                _ => refreshed.mode.as_str(),
            };

            let message = format!(
                "<b>{}</b>\n\n{}: <b>{}</b>\n\n{}",
                tr.s_links_title,
                tr.s_action_mode,
                mode_label,
                if mode_save_ok {
                    tr.s_links_desc
                } else {
                    tr.s_setting_update_failed
                }
            );
            let keyboard = InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback(
                        tr.s_reply_mode,
                        format!("user_setting:set_mode:reply:{}", user_id),
                    ),
                    InlineKeyboardButton::callback(
                        tr.s_delete_mode,
                        format!("user_setting:set_mode:delete:{}", user_id),
                    ),
                ],
                vec![InlineKeyboardButton::callback(tr.s_back, format!("settings:{}", user_id))],
            ]);
            (message, keyboard)
        }
        "clear_history" => {
            let mut clear_ok = true;
            if let Err(e) = db.clear_history(user_id).await {
                tracing::error!(error = %e, user_id, "Errore nella cancellazione cronologia");
                clear_ok = false;
            }
            let mut updated = user_config.clone();
            updated.cleaned_count = 0;
            if let Err(e) = db.save_user_config(&updated).await {
                tracing::error!(error = %e, user_id, "Errore nel reset contatore pulizie");
                clear_ok = false;
            }

            let keyboard = settings_back_keyboard(tr, user_id);
            (
                if clear_ok {
                    tr.s_setting_updated.to_string()
                } else {
                    tr.s_setting_update_failed.to_string()
                },
                keyboard,
            )
        }
        "toggle" if parts.len() >= 4 => {
            let setting = parts[2];
            let value = parts[3];
            
            // Handle toggle logic here (would update database)
            handle_setting_toggle(bot, chat_id, message_id, user_id, setting, value, db, tr).await?;
            return Ok(());
        },
        _ => (
            tr.s_not_found.to_string(),
            settings_back_keyboard(tr, user_id),
        ),
    };

    upsert_settings_view(&bot, chat_id, message_id, message_text, Some(keyboard), true).await?;

    Ok(())
}

async fn handle_admin_settings_callback(
    // message_id deve essere sempre quello della callback per editare il messaggio
    bot: Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    user_id: i64,
    callback_data: &str,
    db: Db,
    config: &crate::config::Config,
    tr: &crate::i18n::Translations,
) -> ResponseResult<()> {
    // Verify admin permissions
    if user_id != config.admin_id {
        show_no_permission_view(&bot, chat_id, message_id, tr).await?;
        return Ok(());
    }

    let parts: Vec<&str> = callback_data.split(':').collect();
    if parts.len() < 3 {
        return Ok(());
    }

    let target_user_id = callback_target_user_id(&parts, user_id);
    if target_user_id != user_id {
        show_no_permission_view(&bot, chat_id, message_id, tr).await?;
        return Ok(());
    }

    let admin_action = parts[1];

    let (message_text, keyboard) = match admin_action {
        "panel" => {
            let message = format!(
                "<b>{}</b>\n\n{}",
                tr.s_admin_panel_title,
                tr.s_admin_panel_desc
            );
            let keyboard = InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback(
                        tr.s_user_management,
                        format!("admin_setting:users:{}", user_id),
                    ),
                    InlineKeyboardButton::callback(
                        tr.s_system_settings,
                        format!("admin_setting:system:{}", user_id),
                    ),
                ],
                vec![
                    InlineKeyboardButton::callback(
                        tr.s_global_stats,
                        format!("admin_setting:global_stats:{}", user_id),
                    ),
                    InlineKeyboardButton::callback(
                        tr.s_maintenance,
                        format!("admin_setting:maintenance:{}", user_id),
                    ),
                ],
                vec![InlineKeyboardButton::callback(
                    tr.s_back,
                    format!("settings:{}", user_id),
                )],
            ]);
            (message, keyboard)
        }
        "stats" => {
            let (total_cleaned, total_users) = db.get_global_stats().await.unwrap_or((0, 0));
            let message = admin_global_stats_message(tr, total_users, total_cleaned);
            let keyboard = admin_global_stats_keyboard(tr, user_id, format!("settings:{}", user_id));
            (message, keyboard)
        }
        "refresh_stats" => {
            let (total_cleaned, total_users) = db.get_global_stats().await.unwrap_or((0, 0));
            let message = admin_global_stats_message(tr, total_users, total_cleaned);
            let keyboard = admin_global_stats_keyboard(tr, user_id, format!("settings:{}", user_id));
            (message, keyboard)
        }
        "users" => {
            let (_, total_users) = db.get_global_stats().await.unwrap_or((0, 0));
            let message = admin_users_message(tr, total_users);
            let keyboard = single_back_keyboard(tr.s_back, format!("admin_setting:panel:{}", user_id));
            (message, keyboard)
        }
        "system" => {
            let message = admin_system_message(tr);
            let keyboard = single_back_keyboard(tr.s_back, format!("admin_setting:panel:{}", user_id));
            (message, keyboard)
        }
        "global_stats" => {
            let (total_cleaned, total_users) = db.get_global_stats().await.unwrap_or((0, 0));
            let message = admin_global_stats_message(tr, total_users, total_cleaned);
            let keyboard =
                admin_global_stats_keyboard(tr, user_id, format!("admin_setting:panel:{}", user_id));
            (message, keyboard)
        }
        "maintenance" => {
            let message = admin_maintenance_message(tr);
            let keyboard = admin_maintenance_keyboard(tr, user_id);
            (message, keyboard)
        }
        "clear_all_history" => {
            let message = tr.s_admin_server_only_op.to_string();
            let keyboard = single_back_keyboard(
                tr.s_back,
                format!("admin_setting:maintenance:{}", user_id),
            );
            (message, keyboard)
        }
        _ => {
            let message = tr.s_admin_option_not_found.to_string();
            let keyboard = settings_back_keyboard(tr, user_id);
            (message, keyboard)
        }
    };

    upsert_settings_view(&bot, chat_id, message_id, message_text, Some(keyboard), true).await?;

    Ok(())
}

async fn handle_setting_toggle(
    bot: Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    user_id: i64,
    setting: &str,
    value: &str,
    db: Db,
    tr: &crate::i18n::Translations,
) -> ResponseResult<()> {
    let mut user_config = db.get_user_config(user_id).await.unwrap_or_default();
    let mut save_ok = true;
    let result_message = match setting {
        "notif" => {
            user_config.enabled = if value == "1" { 1 } else { 0 };
            if let Err(e) = db.save_user_config(&user_config).await {
                tracing::error!(error = %e, user_id, "Errore nel salvataggio toggle notifiche");
                save_ok = false;
            }
            if !save_ok {
                tr.s_setting_update_failed
            } else if user_config.enabled == 1 {
                tr.s_notif_enabled
            } else {
                tr.s_notif_disabled
            }
        }
        "ai" => {
            user_config.ai_enabled = if user_config.ai_enabled == 0 { 1 } else { 0 };
            if let Err(e) = db.save_user_config(&user_config).await {
                tracing::error!(error = %e, user_id, "Errore nel salvataggio toggle AI");
                save_ok = false;
            }
            if save_ok {
                tr.s_ai_toggled
            } else {
                tr.s_setting_update_failed
            }
        }
        _ => tr.s_setting_updated,
    };

    let keyboard = settings_back_keyboard(tr, user_id);

    upsert_settings_view(
        &bot,
        chat_id,
        message_id,
        result_message.to_string(),
        Some(keyboard),
        true,
    )
    .await?;

    Ok(())
}

async fn upsert_settings_view(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    text: String,
    keyboard: Option<InlineKeyboardMarkup>,
    parse_html: bool,
) -> ResponseResult<()> {
    if let Some(message_id) = message_id {
        let mut edit = bot.edit_message_text(chat_id, message_id, text.clone());
        if parse_html {
            edit = edit.parse_mode(ParseMode::Html);
        }
        if let Some(kb) = keyboard.clone() {
            edit = edit.reply_markup(kb);
        }

        match edit.await {
            Ok(_) => return Ok(()),
            Err(err) => {
                if is_message_not_modified_error(&err.to_string()) {
                    return Ok(());
                }
            }
        }
    }

    let mut send = bot.send_message(chat_id, text);
    if parse_html {
        send = send.parse_mode(ParseMode::Html);
    }
    if let Some(kb) = keyboard {
        send = send.reply_markup(kb);
    }
    send.await?;

    Ok(())
}

fn callback_target_user_id(parts: &[&str], fallback_user_id: i64) -> i64 {
    parts
        .last()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(fallback_user_id)
}

fn extract_url_candidates(text: &str) -> Vec<String> {
    let url_pattern = r"(?i)(?:https?://|www\.)[a-zA-Z0-9\-\.]+\.[a-zA-Z]{2,}(?:/[^\s]*)?";
    let Ok(re) = Regex::new(url_pattern) else {
        return Vec::new();
    };

    let mut urls = Vec::new();
    for m in re.find_iter(text) {
        let candidate = m.as_str().to_string();
        if !urls.contains(&candidate) {
            urls.push(candidate);
        }
    }
    urls
}

fn removed_query_params_count(original: &str, cleaned: &str) -> usize {
    let original_count = query_params_count(original);
    let cleaned_count = query_params_count(cleaned);
    original_count.saturating_sub(cleaned_count)
}

fn query_params_count(raw_url: &str) -> usize {
    let normalized = if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
        raw_url.to_string()
    } else {
        format!("https://{raw_url}")
    };

    let Ok(parsed) = url::Url::parse(&normalized) else {
        return 0;
    };
    parsed.query_pairs().count()
}

fn is_message_not_modified_error(error_text: &str) -> bool {
    error_text.to_lowercase().contains("message is not modified")
}

fn main_reply_keyboard(tr: &crate::i18n::Translations) -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![
            KeyboardButton::new(tr.rk_settings),
            KeyboardButton::new(tr.rk_stats),
        ],
        vec![
            KeyboardButton::new(tr.rk_help),
            KeyboardButton::new(tr.rk_language),
        ],
        vec![
            KeyboardButton::new(tr.rk_hidekbd),
        ],
    ])
    .resize_keyboard()
}

#[derive(Clone, Copy)]
enum QuickReplyAction {
    Settings,
    Stats,
    Help,
    HideKeyboard,
    Language,
}

fn quick_reply_action(text: &str, tr: &crate::i18n::Translations) -> Option<QuickReplyAction> {
    let trimmed = text.trim();
    if trimmed == tr.rk_settings {
        Some(QuickReplyAction::Settings)
    } else if trimmed == tr.rk_stats {
        Some(QuickReplyAction::Stats)
    } else if trimmed == tr.rk_help {
        Some(QuickReplyAction::Help)
    } else if trimmed == tr.rk_hidekbd {
        Some(QuickReplyAction::HideKeyboard)
    } else if trimmed == tr.rk_language {
        Some(QuickReplyAction::Language)
    } else {
        None
    }
}

fn quick_actions_inline_keyboard(
    tr: &crate::i18n::Translations,
    user_id: i64,
) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                tr.start_open_settings,
                format!("quick:settings:{}", user_id),
            ),
            InlineKeyboardButton::callback(tr.start_view_stats, format!("quick:stats:{}", user_id)),
        ],
        vec![
            InlineKeyboardButton::callback(tr.s_language, format!("quick:language:{}", user_id)),
            InlineKeyboardButton::callback(tr.s_back_to_main, format!("back_to_main:{}", user_id)),
        ],
    ])
}

fn language_inline_keyboard(
    tr: &crate::i18n::Translations,
    user_id: i64,
) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(tr.s_language_it, format!("user_setting:lang:it:{}", user_id)),
            InlineKeyboardButton::callback(tr.s_language_en, format!("user_setting:lang:en:{}", user_id)),
        ],
        vec![InlineKeyboardButton::callback(tr.s_back, format!("settings:{}", user_id))],
    ])
}

fn single_back_keyboard(label: &str, callback_data: String) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        label,
        callback_data,
    )]])
}

fn settings_back_keyboard(tr: &crate::i18n::Translations, user_id: i64) -> InlineKeyboardMarkup {
    single_back_keyboard(tr.s_back, format!("settings:{}", user_id))
}

async fn show_no_permission_view(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    tr: &crate::i18n::Translations,
) -> ResponseResult<()> {
    upsert_settings_view(
        bot,
        chat_id,
        message_id,
        tr.s_admin_no_permission.to_string(),
        None,
        false,
    )
    .await
}

fn admin_global_stats_message(
    tr: &crate::i18n::Translations,
    total_users: i64,
    total_cleaned: i64,
) -> String {
    format!(
        "<b>{}</b>\n\n{}\n\n👥 {}: <b>{}</b>\n🔗 {}: <b>{}</b>",
        tr.s_global_stats_title,
        tr.s_global_stats_desc,
        tr.s_total_users_label,
        total_users,
        tr.s_total_cleaned_label,
        total_cleaned
    )
}

fn admin_users_message(tr: &crate::i18n::Translations, total_users: i64) -> String {
    format!(
        "<b>{}</b>\n\n{}: <b>{}</b>",
        tr.s_user_management, tr.s_admin_users_total, total_users
    )
}

fn admin_system_message(tr: &crate::i18n::Translations) -> String {
    format!("<b>{}</b>\n\n{}", tr.s_system_settings, tr.s_admin_system_note)
}

fn admin_maintenance_message(tr: &crate::i18n::Translations) -> String {
    format!("<b>{}</b>\n\n{}", tr.s_maintenance, tr.s_admin_maintenance_none)
}

fn admin_global_stats_keyboard(
    tr: &crate::i18n::Translations,
    user_id: i64,
    back_callback_data: String,
) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            tr.s_refresh,
            format!("admin_setting:refresh_stats:{}", user_id),
        )],
        vec![InlineKeyboardButton::callback(tr.s_back, back_callback_data)],
    ])
}

fn admin_maintenance_keyboard(tr: &crate::i18n::Translations, user_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            tr.s_clear_history,
            format!("admin_setting:clear_all_history:{}", user_id),
        )],
        vec![InlineKeyboardButton::callback(
            tr.s_back,
            format!("admin_setting:panel:{}", user_id),
        )],
    ])
}

#[cfg(test)]
mod tests {
    use super::{
        admin_global_stats_message, admin_maintenance_message, admin_system_message,
        admin_users_message, callback_target_user_id, is_message_not_modified_error,
        removed_query_params_count,
    };
    use crate::i18n;

    #[test]
    fn callback_target_user_id_uses_last_segment_when_numeric() {
        let parts = vec!["user_setting", "toggle", "ai", "42"];
        let user_id = callback_target_user_id(&parts, 7);
        assert_eq!(user_id, 42);
    }

    #[test]
    fn callback_target_user_id_falls_back_when_last_segment_is_not_numeric() {
        let parts = vec!["user_setting", "toggle", "ai", "abc"];
        let user_id = callback_target_user_id(&parts, 7);
        assert_eq!(user_id, 7);
    }

    #[test]
    fn callback_target_user_id_falls_back_on_empty_parts() {
        let parts: Vec<&str> = vec![];
        let user_id = callback_target_user_id(&parts, 15);
        assert_eq!(user_id, 15);
    }

    #[test]
    fn detects_message_not_modified_error_case_insensitive() {
        let error_text = "Bad Request: MESSAGE IS NOT MODIFIED";
        assert!(is_message_not_modified_error(error_text));
    }

    #[test]
    fn ignores_other_errors() {
        let error_text = "Bad Request: message to edit not found";
        assert!(!is_message_not_modified_error(error_text));
    }

    #[test]
    fn callback_target_user_id_reads_owner_from_settings_callback() {
        let parts = vec!["settings", "99"];
        let user_id = callback_target_user_id(&parts, 7);
        assert_eq!(user_id, 99);
    }

    #[test]
    fn admin_global_stats_message_includes_values_and_labels() {
        let tr = i18n::get_translations("it");
        let message = admin_global_stats_message(&tr, 12, 345);
        assert!(message.contains(tr.s_total_users_label));
        assert!(message.contains(tr.s_total_cleaned_label));
        assert!(message.contains("12"));
        assert!(message.contains("345"));
    }

    #[test]
    fn admin_users_message_includes_total_users() {
        let tr = i18n::get_translations("en");
        let message = admin_users_message(&tr, 27);
        assert!(message.contains(tr.s_user_management));
        assert!(message.contains(tr.s_admin_users_total));
        assert!(message.contains("27"));
    }

    #[test]
    fn admin_system_message_uses_localized_note() {
        let tr = i18n::get_translations("it");
        let message = admin_system_message(&tr);
        assert!(message.contains(tr.s_system_settings));
        assert!(message.contains(tr.s_admin_system_note));
    }

    #[test]
    fn admin_maintenance_message_uses_localized_note() {
        let tr = i18n::get_translations("en");
        let message = admin_maintenance_message(&tr);
        assert!(message.contains(tr.s_maintenance));
        assert!(message.contains(tr.s_admin_maintenance_none));
    }

    #[test]
    fn removed_query_params_count_detects_removed_tracking_params() {
        let original = "https://example.com/path?a=1&b=2&utm_source=x";
        let cleaned = "https://example.com/path?a=1";
        assert_eq!(removed_query_params_count(original, cleaned), 2);
    }

    #[test]
    fn removed_query_params_count_handles_schemeless_urls() {
        let original = "www.example.com/?a=1&b=2";
        let cleaned = "www.example.com/?a=1";
        assert_eq!(removed_query_params_count(original, cleaned), 1);
    }
}

use crate::{
    db::{Db, models::UserConfig},
    i18n,
    sanitizer::{AiEngine, RuleEngine},
};
use regex::Regex;
use teloxide::prelude::*;
use teloxide::types::{
    CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, MessageEntityKind, ParseMode,
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
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![db, rules, ai, config, event_tx])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

/// Handles incoming Telegram messages, detects URLs, and cleans tracking parameters.
///
/// # Errors
/// Returns an error if message sending or database operations fail.
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
    tracing::info!(chat_id = %msg.chat.id, msg_id = %msg.id, "Elaborazione messaggio in arrivo");
    let chat_id = msg.chat.id;
    let user_id = msg.from.as_ref().map_or(0, |u| i64::try_from(u.id.0).unwrap_or(0));
    tracing::Span::current().record("user_id", user_id);

    let user_config = db.get_user_config(user_id).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "Errore nel recupero config utente, uso default");
        UserConfig::default()
    });

    // 1. Detect URLs early
    let (text, entities) = if let Some(t) = msg.text() {
        (t, msg.entities())
    } else if let Some(c) = msg.caption() {
        (c, msg.caption_entities())
    } else {
        ("", None)
    };

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

    let mut has_urls = entities
        .as_ref()
        .is_some_and(|e| {
            e.iter().any(|entity| {
                matches!(
                    entity.kind,
                    MessageEntityKind::Url | MessageEntityKind::TextLink { .. }
                )
            })
        });

    // Manual fallback detection for schemeless URLs or cases where Telegram detection fails
    if !has_urls {
        // Simple but effective regex for detection
        let url_pattern = r"(?i)(?:https?://|www\.)[a-zA-Z0-9\-\.]+\.[a-zA-Z]{2,}(?:/[^\s]*)?";
        if let Ok(re) = Regex::new(url_pattern) {
            if re.is_match(text) {
                has_urls = true;
                tracing::debug!("URL rilevato tramite regex di fallback");
            }
        }
    }

    // Handle Commands
    if let Some(text_val) = msg.text() {
        if text_val.starts_with('/') {
            let cmd_parts: Vec<&str> = text_val.split('@').collect();
            let cmd = cmd_parts[0];
            let is_private = msg.chat.is_private();
            let bot_username = config.bot_username.to_lowercase();

            let is_targeted = if cmd_parts.len() > 1 {
                cmd_parts[1].to_lowercase().starts_with(&bot_username)
            } else {
                is_private
            };

            if is_targeted {
                match cmd {
                    "/start" => {
                        tracing::info!("Gestione comando /start per utente {}", user_id);
                        handle_start_command(bot.clone(), chat_id, user_id, &tr, &config).await?;
                        return Ok(());
                    }
                    "/help" => {
                        bot.send_message(chat_id, tr.help_text)
                            .parse_mode(ParseMode::Html)
                            .await?;
                        return Ok(());
                    }
                    "/stats" => {
                        let stats_text = tr
                            .stats_text
                            .replace("{}", &user_config.cleaned_count.to_string());
                        bot.send_message(chat_id, stats_text)
                            .parse_mode(ParseMode::Html)
                            .await?;
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }

    // Persist/Update chat info
    let is_group_context = msg.chat.is_group() || msg.chat.is_supergroup() || msg.chat.is_channel();
    let mut chat_config = db
        .get_chat_config_or_default(chat_id.0)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "Errore nel recupero config chat, uso default");
            crate::db::models::ChatConfig::default()
        });

    if is_group_context {
        let title = msg.chat.title().map(ToString::to_string);
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
                &html::escape(&title.unwrap_or_else(|| tr.unknown.to_string())),
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
        // 1. Expand shortened URLs first
        let expanded_url = rules.expand_url(&url_str).await;
        let original_url_str = url_str.clone();
        let mut current_url = expanded_url;

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
            cleaned_urls.push((original_url_str, current_url, provider));
        } else {
            tracing::debug!(url = %rules.redact_sensitive(&current_url), "URL gia' pulito");
            if user_config.is_ai_enabled() && config.ai_api_key.is_some() {
                if let Ok(Some(ai_cleaned)) = ai.sanitize(&current_url).await {
                    tracing::info!("URL pulito da fallback AI");
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

    if mode == "delete" && bot.delete_message(chat_id, msg.id).await.is_ok() {
        let user_name = msg
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
        let user_name = msg
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
        .reply_parameters(ReplyParameters::new(msg.id))
        .parse_mode(ParseMode::Html);

    // Support for Supergroup topics/threads
    if let Some(thread_id) = msg.thread_id {
        request = request.message_thread_id(thread_id);
    }

    if let Err(e) = request.await {
        tracing::error!(chat_id = %chat_id, error = %e, "Errore nell'invio della risposta con URL puliti");
        return Err(e);
    }

    Ok(())
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
) -> ResponseResult<()> {
    let welcome_text = tr.welcome.replace("{}", &user_id.to_string());

    // Create inline keyboard with settings button
    let keyboard = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(tr.start_open_settings, format!("settings:{}", user_id)),
        InlineKeyboardButton::callback(tr.start_view_stats, format!("user_setting:stats:{}", user_id)),
    ]]);

    bot.send_message(chat_id, welcome_text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

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
    let callback_data = q.data.as_deref().unwrap_or("");
    let chat_id = q
        .message
        .as_ref()
        .map(teloxide::types::MaybeInaccessibleMessage::chat)
        .map(|chat| chat.id);
    let user_id = q.from.id.0 as i64;

    // Get user's preferred language
    let telegram_lang = q.from.language_code.as_deref();
    let lang_code = get_user_language(&db, user_id, telegram_lang).await;
    let tr = crate::i18n::get_translations(lang_code);

    if let Some(chat_id) = chat_id {
        if callback_data.starts_with("settings:") {
            handle_settings_callback(bot.clone(), chat_id, user_id, db, config, &tr).await?;
        } else if callback_data.starts_with("user_setting:") {
            handle_user_settings_callback(bot.clone(), chat_id, user_id, callback_data, db, &tr).await?;
        } else if callback_data.starts_with("admin_setting:") {
            handle_admin_settings_callback(
                bot.clone(),
                chat_id,
                user_id,
                callback_data,
                db,
                &config,
                &tr,
            )
            .await?;
        } else if callback_data == "back_to_main" {
            handle_start_command(
                bot.clone(),
                chat_id,
                user_id,
                &tr,
                &config,
            )
            .await?;
        }
    }

    // Answer callback to remove loading state
    bot.answer_callback_query(q.id).await?;

    Ok(())
}

async fn handle_settings_callback(
    bot: Bot,
    chat_id: ChatId,
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
        "back_to_main".to_string(),
    )]);

    let keyboard = InlineKeyboardMarkup::new(keyboard_rows);

    bot.send_message(chat_id, settings_text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_user_settings_callback(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    callback_data: &str,
    db: Db,
    tr: &crate::i18n::Translations,
) -> ResponseResult<()> {
    let parts: Vec<&str> = callback_data.split(':').collect();
    if parts.len() < 3 {
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
        "toggle" if parts.len() >= 4 => {
            let setting = parts[2];
            let value = parts[3];
            
            // Handle toggle logic here (would update database)
            handle_setting_toggle(bot, chat_id, user_id, setting, value, db, tr).await?;
            return Ok(());
        },
        _ => (
            tr.s_not_found.to_string(),
            InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(tr.s_back, format!("settings:{}", user_id))]]),
        ),
    };

    bot.send_message(chat_id, message_text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_admin_settings_callback(
    bot: Bot,
    chat_id: ChatId,
    user_id: i64,
    callback_data: &str,
    _db: Db,
    config: &crate::config::Config,
    tr: &crate::i18n::Translations,
) -> ResponseResult<()> {
    // Verify admin permissions
    if user_id != config.admin_id {
        bot.send_message(chat_id, tr.s_admin_no_permission)
            .await?;
        return Ok(());
    }

    let parts: Vec<&str> = callback_data.split(':').collect();
    if parts.len() < 3 {
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
            let message = format!(
                "<b>{}</b>\n\n{}",
                tr.s_global_stats_title,
                tr.s_global_stats_desc
            );
            let keyboard = InlineKeyboardMarkup::new(vec![
                vec![InlineKeyboardButton::callback(
                    tr.s_refresh,
                    format!("admin_setting:refresh_stats:{}", user_id),
                )],
                vec![InlineKeyboardButton::callback(
                    tr.s_back,
                    format!("settings:{}", user_id),
                )],
            ]);
            (message, keyboard)
        }
        _ => {
            let message = tr.s_admin_option_not_found.to_string();
            let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
                tr.s_back,
                format!("settings:{}", user_id),
            )]]);
            (message, keyboard)
        }
    };

    bot.send_message(chat_id, message_text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_setting_toggle(
    bot: Bot,
    chat_id: ChatId,
    _user_id: i64,
    setting: &str,
    value: &str,
    _db: Db,
    tr: &crate::i18n::Translations,
) -> ResponseResult<()> {
    // This would update the database in a real implementation
    let result_message = match (setting, value) {
        ("notif", "1") => tr.s_notif_enabled,
        ("notif", "0") => tr.s_notif_disabled,
        ("ai", _) => tr.s_ai_toggled,
        _ => tr.s_setting_updated,
    };

    bot.send_message(chat_id, result_message)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}

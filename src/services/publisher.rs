//! Publication Discord des calendriers et notifications de changement.

use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::Utc;
use serenity::all::*;
use tokio::time::sleep;

use crate::app::App;
use crate::services::ics::ChangeReport;

pub fn calendar_components() -> Vec<CreateActionRow> {
    let options = crate::util::dates::DAYS
        .iter()
        .map(|day| CreateSelectMenuOption::new(*day, *day))
        .collect();
    vec![
        CreateActionRow::SelectMenu(
            CreateSelectMenu::new("select-day", CreateSelectMenuKind::String { options })
                .placeholder("Jour ciblé"),
        ),
        CreateActionRow::Buttons(vec![
            CreateButton::new("button-today")
                .label("Aujourd'hui")
                .style(ButtonStyle::Success),
            CreateButton::new("button-ics")
                .label("Télécharger ICS")
                .emoji('⬇')
                .style(ButtonStyle::Secondary),
        ]),
        CreateActionRow::Buttons(vec![CreateButton::new("button-specific-week")
            .label("Semaine Spécifique")
            .style(ButtonStyle::Primary)]),
    ]
}

pub async fn send_calendar(
    http: &Http,
    app: &App,
    channel_id: ChannelId,
    changed: bool,
    image_url: Option<&str>,
) -> Result<()> {
    let bot_id = UserId::new(app.bot_user_id.load(Ordering::Relaxed));
    let mut selected = None;
    for message in channel_id
        .messages(http, GetMessages::new().limit(5))
        .await?
    {
        if message.author.id == bot_id {
            selected = Some(message.id);
        }
    }
    let mut description = format!(
        "Mise à jour : <t:{}:F>",
        app.last_update_unix.load(Ordering::Relaxed)
    );
    if changed {
        description.push_str(
            " /!\\ Modification à l'emplois du temps apportée depuis la dernière mise à jour.",
        );
    }
    let mut embed = CreateEmbed::new()
        .title("Calendrier")
        .description(description)
        .color(0x2ecc71);
    if let Some(url) = image_url {
        embed = embed.image(url);
    }
    if let Some(message_id) = selected {
        channel_id
            .edit_message(
                http,
                message_id,
                EditMessage::new()
                    .embed(embed)
                    .components(calendar_components()),
            )
            .await?;
    } else {
        channel_id
            .send_message(
                http,
                CreateMessage::new()
                    .embed(embed)
                    .components(calendar_components()),
            )
            .await?;
    }
    Ok(())
}

pub async fn upload_image(http: &Http, app: &App, path: &Path) -> Result<String> {
    let channel = ChannelId::new(app.config.img_target_channel);
    let bot_id = UserId::new(app.bot_user_id.load(Ordering::Relaxed));
    if let Ok(messages) = channel.messages(http, GetMessages::new().limit(1)).await {
        if let Some(message) = messages
            .first()
            .filter(|message| message.author.id == bot_id)
        {
            retry(|| message.delete(http)).await?;
            sleep(Duration::from_millis(1500)).await;
        }
    }
    let attachment = CreateAttachment::path(path).await?;
    let message =
        retry(|| channel.send_files(http, vec![attachment.clone()], CreateMessage::new())).await?;
    message
        .attachments
        .first()
        .map(|file| file.url.clone())
        .ok_or_else(|| anyhow!("Discord n'a pas renvoyé l'URL de l'image"))
}

async fn retry<T, F, Fut>(mut operation: F) -> serenity::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = serenity::Result<T>>,
{
    let mut last = None;
    for attempt in 0..3 {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                last = Some(error);
                if attempt < 2 {
                    sleep(Duration::from_secs(5 * (1 << attempt))).await;
                }
            }
        }
    }
    Err(last.expect("au moins une tentative"))
}

pub async fn send_change_message(
    http: &Http,
    app: &App,
    guild_id: GuildId,
    report: &ChangeReport,
) -> Result<()> {
    let Some(notification) =
        crate::db::get_guild_notification(&app.pool, guild_id.get() as i64).await?
    else {
        return Ok(());
    };
    let role = RoleId::new(notification.role_id as u64);
    let roles = guild_id.roles(http).await?;
    if !roles.contains_key(&role) {
        return Ok(());
    }
    let mut embed = CreateEmbed::new()
        .title(&report.title)
        .description(&report.description)
        .color(report.color)
        .timestamp(Utc::now())
        .footer(CreateEmbedFooter::new(&report.footer));
    for field in &report.fields {
        embed = embed.field(&field.name, &field.value, false);
    }
    sleep(Duration::from_secs(1)).await;
    ChannelId::new(notification.channel_id as u64)
        .send_message(
            http,
            CreateMessage::new()
                .content(role.mention().to_string())
                .embed(embed)
                .allowed_mentions(CreateAllowedMentions::new().roles(vec![role])),
        )
        .await?;
    Ok(())
}

pub async fn publish_all(
    http: Arc<Http>,
    app: Arc<App>,
    changed: bool,
    report: Option<ChangeReport>,
    image_url: String,
) {
    let channels = match crate::db::list_target_channels(&app.pool).await {
        Ok(channels) => channels,
        Err(error) => {
            tracing::error!(%error, "lecture des salons impossible");
            return;
        }
    };
    for raw in channels {
        let channel = ChannelId::new(raw as u64);
        if let Err(error) = send_calendar(&http, &app, channel, changed, Some(&image_url)).await {
            tracing::error!(channel_id = raw, %error, "publication du calendrier impossible");
            continue;
        }
        if let Some(report) = &report {
            match channel
                .to_channel(&http)
                .await
                .ok()
                .and_then(|channel| channel.guild().map(|channel| channel.guild_id))
            {
                Some(guild_id) => {
                    if let Err(error) = send_change_message(&http, &app, guild_id, report).await {
                        tracing::error!(%guild_id, %error, "notification de changement impossible");
                    }
                }
                None => tracing::warn!(channel_id = raw, "salon sans guilde accessible"),
            }
        }
    }
}

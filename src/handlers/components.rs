//! Composants persistants et modal de sélection d'une semaine.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use chrono::Duration as ChronoDuration;
use rand::Rng;
use serenity::all::*;

use crate::app::App;
use crate::services::{ics, images};
use crate::util::dates;

pub async fn handle_component(ctx: &Context, app: Arc<App>, interaction: &ComponentInteraction) {
    if let Err(error) = dispatch_component(ctx, &app, interaction).await {
        tracing::error!(%error, custom_id = %interaction.data.custom_id, "composant Discord échoué");
    }
}

async fn dispatch_component(
    ctx: &Context,
    app: &App,
    interaction: &ComponentInteraction,
) -> Result<()> {
    match interaction.data.custom_id.as_str() {
        "select-day" => {
            let ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind
            else {
                return Err(anyhow!("sélection invalide"));
            };
            let day = values.first().ok_or_else(|| anyhow!("jour absent"))?;
            let index = dates::DAYS
                .iter()
                .position(|candidate| candidate == day)
                .ok_or_else(|| anyhow!("jour invalide"))?;
            let path = app
                .config
                .data_dir
                .join("days_imgs")
                .join(format!("{day}_schedule.png"));
            tokio::fs::create_dir_all(path.parent().unwrap()).await?;
            let stale = std::fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .map_or(true, |modified| {
                    SystemTime::now()
                        .duration_since(modified)
                        .unwrap_or_default()
                        > Duration::from_secs(1800)
                });
            if stale {
                let _lock = tokio::time::timeout(Duration::from_secs(10), app.image_writes.lock())
                    .await
                    .map_err(|_| anyhow!("génération d'image occupée"))?;
                let content =
                    tokio::fs::read_to_string(app.config.data_dir.join("current.ics")).await?;
                let events = ics::parse_ics(&content)?;
                let target = dates::monday_of_week(dates::today_paris())
                    + ChronoDuration::days(index as i64);
                images::render_day(&images::events_for_date(&events, target), target, &path)?;
            }
            send_file(ctx, interaction, &path).await?;
        }
        "button-today" => {
            send_file(
                ctx,
                interaction,
                &app.config.data_dir.join("day_schedule.png"),
            )
            .await?
        }
        "button-ics" => {
            send_file(ctx, interaction, &app.config.data_dir.join("current.ics")).await?
        }
        "button-specific-week" => {
            let modal =
                CreateModal::new("modal-specific-week", "Semaine Spécifique").components(vec![
                    CreateActionRow::InputText(
                        CreateInputText::new(
                            InputTextStyle::Short,
                            "Semaine/date ciblée (n'importe quel format)",
                            "specific-week-date",
                        )
                        .required(true),
                    ),
                ]);
            interaction
                .create_response(&ctx.http, CreateInteractionResponse::Modal(modal))
                .await?;
        }
        _ => {}
    }
    Ok(())
}

pub async fn handle_modal(ctx: &Context, app: Arc<App>, interaction: &ModalInteraction) {
    if interaction.data.custom_id != "modal-specific-week" {
        return;
    }
    if let Err(error) = dispatch_modal(ctx, &app, interaction).await {
        tracing::error!(%error, "modal Discord échoué");
        let _ = interaction
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("Erreur lors du traitement de la requête.")
                        .ephemeral(true),
                ),
            )
            .await;
    }
}

async fn dispatch_modal(ctx: &Context, app: &App, interaction: &ModalInteraction) -> Result<()> {
    let value = interaction
        .data
        .components
        .iter()
        .flat_map(|row| &row.components)
        .find_map(|component| match component {
            ActionRowComponent::InputText(input) => input.value.as_deref(),
            _ => None,
        })
        .ok_or_else(|| anyhow!("date absente"))?;
    let Some(date) = dates::parse_date_input(value) else {
        interaction
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("La date saisie est inconnue.")
                        .ephemeral(true),
                ),
            )
            .await?;
        return Ok(());
    };
    let content = tokio::fs::read_to_string(app.config.data_dir.join("current.ics")).await?;
    let events = ics::parse_ics(&content)?;
    let filename = format!("week-{}.png", rand::rng().random::<u64>());
    let path = app.config.data_dir.join(filename);
    images::render_week(&events, dates::monday_of_week(date), &path)?;
    send_modal_file(ctx, interaction, &path).await?;
    if let Err(error) = tokio::fs::remove_file(&path).await {
        tracing::warn!(%error, "suppression de l'image temporaire impossible");
    }
    Ok(())
}

async fn send_file(
    ctx: &Context,
    interaction: &ComponentInteraction,
    path: &std::path::Path,
) -> Result<()> {
    if !path.exists() {
        interaction
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("Le calendrier n'est pas encore disponible.")
                        .ephemeral(true),
                ),
            )
            .await?;
    } else {
        let file = CreateAttachment::path(path).await?;
        interaction
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_file(file)
                        .ephemeral(true),
                ),
            )
            .await?;
    }
    Ok(())
}

async fn send_modal_file(
    ctx: &Context,
    interaction: &ModalInteraction,
    path: &std::path::Path,
) -> Result<()> {
    let file = CreateAttachment::path(path).await?;
    interaction
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_file(file)
                    .ephemeral(true),
            ),
        )
        .await?;
    Ok(())
}

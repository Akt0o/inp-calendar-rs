//! Déclaration et traitement des huit commandes slash publiques.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use serenity::all::*;

use crate::app::App;
use crate::services::publisher;

pub fn definitions() -> Vec<CreateCommand> {
    let admin = Permissions::MANAGE_ROLES;
    vec![
        CreateCommand::new("register")
            .description("Enregistre/dé-enregistre ce salon comme cible de publication")
            .default_member_permissions(admin),
        CreateCommand::new("register_change_role")
            .description("Enregistre un rôle et ce salon comme cible des changements")
            .default_member_permissions(admin)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "role_id",
                    "Identifiant du rôle",
                )
                .required(true),
            ),
        CreateCommand::new("unregister_change_role")
            .description("Désenregistre le rôle et le salon des changements")
            .default_member_permissions(admin),
        CreateCommand::new("change_notif")
            .description("Vous donne/retire le rôle des notifications"),
        CreateCommand::new("force_message")
            .description("Force l'envoi du calendrier")
            .default_member_permissions(admin),
        CreateCommand::new("force_change_message")
            .description("Force la vérification d'un changement")
            .default_member_permissions(admin),
        CreateCommand::new("get_ics").description("Récupère le fichier ICS"),
        CreateCommand::new("get_day").description("Récupère l'agenda du jour"),
    ]
}

pub async fn handle(ctx: &Context, app: Arc<App>, command: &CommandInteraction) {
    if let Err(error) = dispatch(ctx, &app, command).await {
        tracing::error!(command = %command.data.name, %error, "commande Discord échouée");
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("Une erreur interne est survenue.")
                .ephemeral(true),
        );
        let _ = command.create_response(&ctx.http, response).await;
    }
}

async fn dispatch(ctx: &Context, app: &App, command: &CommandInteraction) -> Result<()> {
    match command.data.name.as_str() {
        "register" => {
            if !is_admin(command) {
                reply(
                    command,
                    ctx,
                    "Doit être admin pour effectuer cette commande",
                )
                .await?;
                return Ok(());
            }
            let added =
                crate::db::toggle_target_channel(&app.pool, command.channel_id.get() as i64)
                    .await?;
            reply(
                command,
                ctx,
                if added {
                    "Salon ajouté à la liste !"
                } else {
                    "Salon retiré de la liste !"
                },
            )
            .await?;
        }
        "register_change_role" => {
            if !is_admin(command) {
                reply(
                    command,
                    ctx,
                    "Doit être admin pour effectuer cette commande",
                )
                .await?;
                return Ok(());
            }
            let guild = command
                .guild_id
                .ok_or_else(|| anyhow!("commande utilisable uniquement dans une guilde"))?;
            let role_id = string_option(command, "role_id")?
                .parse::<u64>()
                .map(RoleId::new)?;
            if !guild.roles(&ctx.http).await?.contains_key(&role_id) {
                reply(command, ctx, "Rôle inconnu.").await?;
            } else {
                crate::db::set_guild_notification(
                    &app.pool,
                    guild.get() as i64,
                    command.channel_id.get() as i64,
                    role_id.get() as i64,
                )
                .await?;
                reply(command, ctx, "Rôle et salon de notif changés !").await?;
            }
        }
        "unregister_change_role" => {
            if !is_admin(command) {
                reply(
                    command,
                    ctx,
                    "Doit être admin pour effectuer cette commande",
                )
                .await?;
                return Ok(());
            }
            let guild = command
                .guild_id
                .ok_or_else(|| anyhow!("commande utilisable uniquement dans une guilde"))?;
            crate::db::clear_guild_notification(&app.pool, guild.get() as i64).await?;
            reply(command, ctx, "Rôle et salon de notif retirés !").await?;
        }
        "change_notif" => toggle_notification_role(ctx, app, command).await?,
        "force_message" => {
            if !is_admin(command) {
                reply(
                    command,
                    ctx,
                    "Doit être admin pour effectuer cette commande",
                )
                .await?;
                return Ok(());
            }
            let image = app.current_image_url.lock().await.clone();
            publisher::send_calendar(&ctx.http, app, command.channel_id, false, image.as_deref())
                .await?;
            reply(command, ctx, "Calendrier envoyé !").await?;
        }
        "force_change_message" => {
            if !is_admin(command) {
                reply(
                    command,
                    ctx,
                    "Doit être admin pour effectuer cette commande",
                )
                .await?;
                return Ok(());
            }
            reply(command, ctx, "Message de changement envoyé !").await?;
        }
        "get_ics" => {
            send_file(
                command,
                ctx,
                &app.config.data_dir.join("current.ics"),
                "current.ics",
            )
            .await?
        }
        "get_day" => {
            send_file(
                command,
                ctx,
                &app.config.data_dir.join("day_schedule.png"),
                "day_schedule.png",
            )
            .await?
        }
        _ => {}
    }
    Ok(())
}

fn is_admin(command: &CommandInteraction) -> bool {
    command
        .member
        .as_ref()
        .and_then(|member| member.permissions)
        .is_some_and(|permissions| permissions.contains(Permissions::MANAGE_ROLES))
}

fn string_option<'a>(command: &'a CommandInteraction, name: &str) -> Result<&'a str> {
    command
        .data
        .options
        .iter()
        .find(|option| option.name == name)
        .and_then(|option| match &option.value {
            CommandDataOptionValue::String(value) => Some(value.as_str()),
            _ => None,
        })
        .ok_or_else(|| anyhow!("option {name} absente"))
}

async fn toggle_notification_role(
    ctx: &Context,
    app: &App,
    command: &CommandInteraction,
) -> Result<()> {
    let guild = command
        .guild_id
        .ok_or_else(|| anyhow!("commande utilisable uniquement dans une guilde"))?;
    let Some(notification) =
        crate::db::get_guild_notification(&app.pool, guild.get() as i64).await?
    else {
        reply(command, ctx, "Rôle inconnu.").await?;
        return Ok(());
    };
    let role = RoleId::new(notification.role_id as u64);
    let member = guild.member(&ctx.http, command.user.id).await?;
    let result = if member.roles.contains(&role) {
        member
            .remove_role(&ctx.http, role)
            .await
            .map(|_| "Rôle supprimé.")
    } else {
        member
            .add_role(&ctx.http, role)
            .await
            .map(|_| "Rôle ajouté.")
    };
    match result {
        Ok(message) => reply(command, ctx, message).await?,
        Err(serenity::Error::Http(error))
            if error.status_code() == Some(reqwest::StatusCode::FORBIDDEN) =>
        {
            reply(command, ctx, "N'a pas les droits pour ajouter le rôle.").await?
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

async fn reply(command: &CommandInteraction, ctx: &Context, content: &str) -> serenity::Result<()> {
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(content)
                    .ephemeral(true),
            ),
        )
        .await
}

async fn send_file(
    command: &CommandInteraction,
    ctx: &Context,
    path: &std::path::Path,
    filename: &str,
) -> Result<()> {
    if !path.exists() {
        reply(command, ctx, "Le calendrier n'est pas encore disponible.").await?;
        return Ok(());
    }
    let mut attachment = CreateAttachment::path(path).await?;
    attachment.filename = filename.to_string();
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_file(attachment)
                    .ephemeral(true),
            ),
        )
        .await?;
    Ok(())
}

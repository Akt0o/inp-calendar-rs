# Bot calendrier

Bot Discord en Rust qui récupère un calendrier ICS, génère les vues du jour et de la semaine, puis maintient un message calendrier dans chaque salon enregistré. Les salons, rôles de notification et le dernier calendrier sont stockés dans SQLite sous `data/`.

## Configuration

Copier `.env.example` vers `.env`, puis renseigner `TOKEN`, `CALENDAR_USERNAME`, `CALENDAR_PASSWORD` et `CALENDAR_URL`. Les autres variables ont des valeurs par défaut.

Le bot doit être invité avec les scopes `bot` et `applications.commands`, ainsi que les permissions de lecture/écriture des salons, gestion des messages et gestion des rôles.

## Exécution

```bash
cargo run --release
```

Ou avec Docker :

```bash
docker compose up -d --build
```

Le volume `./data:/app/data` conserve `calendar.db`, `current.ics` et les images générées.

## Commandes

- `/register` ajoute ou retire le salon courant des publications.
- `/register_change_role role_id` configure les notifications de changement de la guilde.
- `/unregister_change_role` retire cette configuration.
- `/change_notif` ajoute ou retire le rôle configuré à l'utilisateur.
- `/force_message` republie le calendrier dans le salon courant.
- `/force_change_message` reproduit la vérification historique sans générer de fausse différence.
- `/get_ics` et `/get_day` téléchargent les fichiers courants.

Les commandes d'administration requièrent la permission `Gérer les rôles`. Les boutons et le sélecteur du message calendrier restent utilisables après un redémarrage.

## Développement

Le projet nécessite Rust 1.94 ou une version ultérieure.

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Les tags `vMAJOR.MINOR.PATCH` déclenchent la publication de l'image GHCR et d'une release GitHub.

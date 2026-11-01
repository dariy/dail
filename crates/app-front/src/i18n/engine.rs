//! The engine's texts in a language. `app-core` stays in English: its errors and undo labels
//! are part of the JSON that agents read. The front-ends translate them here, for display.
//! An English text unknown here (a server's message, a new label) is shown as it is.

use app_core::{CommandName, CoreError};

use super::{Lang, commands, pseudo};

/// English reasons of `invalid_argument` and similar errors, with their French.
const REASONS: &[(&str, &str)] = &[
    (
        "give `ids` (threads), `messages` or both",
        "indiquez `ids` (discussions), `messages` ou les deux",
    ),
    (
        "the mailbox belongs to another account than the messages",
        "le dossier appartient à un autre compte que les messages",
    ),
    (
        "the op is running on the server; discard it after it failed",
        "l’opération est en cours sur le serveur ; abandonnez-la après son échec",
    ),
    (
        "expected an email address like news@example.org",
        "adresse attendue, par exemple news@example.org",
    ),
    (
        "no conversation is open (open_thread)",
        "aucune discussion n’est ouverte (open_thread)",
    ),
    ("give either id or account", "indiquez id ou account"),
    (
        "is empty; give at least one term, e.g. `is:unread`",
        "est vide ; donnez au moins un terme, par exemple `is:unread`",
    ),
    (
        "the inbox can't be renamed",
        "la boîte de réception ne peut pas être renommée",
    ),
    (
        "the inbox can't be deleted",
        "la boîte de réception ne peut pas être supprimée",
    ),
    ("that is its name already", "c’est déjà son nom"),
    (
        "a folder can't move into itself",
        "un dossier ne peut pas être déplacé dans lui-même",
    ),
    (
        "delete its subfolders first",
        "supprimez d’abord ses sous-dossiers",
    ),
    ("must not be empty", "ne doit pas être vide"),
    (
        "nothing to update: give display_name, imap or smtp",
        "rien à modifier : indiquez display_name, imap ou smtp",
    ),
    (
        "expected an address like name@example.com",
        "adresse attendue, par exemple nom@example.com",
    ),
    (
        "expected an address like me@example.org",
        "adresse attendue, par exemple moi@example.org",
    ),
    (
        "must be non-empty and contain no control characters",
        "ne doit être ni vide ni contenir de caractères de contrôle",
    ),
    (
        "host must be a DNS name or IP address",
        "l’hôte doit être un nom DNS ou une adresse IP",
    ),
    (
        "port must be between 1 and 65535",
        "le port doit être compris entre 1 et 65535",
    ),
    (
        "this build cannot update itself (the runtime reported no version)",
        "cette version ne peut pas se mettre à jour elle-même",
    ),
    (
        "no update is available; run check_for_updates first",
        "aucune mise à jour disponible ; lancez d’abord check_for_updates",
    ),
    (
        "no update is downloaded; run download_update first",
        "aucune mise à jour téléchargée ; lancez d’abord download_update",
    ),
    (
        "the runtime reported no install kind",
        "type d’installation inconnu",
    ),
    (
        "expected a version like 0.2.0, or null",
        "version attendue, par exemple 0.2.0, ou null",
    ),
    ("expected 1 to 720 hours", "de 1 à 720 heures attendues"),
    (
        "nothing to update: give to, cc, bcc, subject or body",
        "rien à modifier : indiquez to, cc, bcc, subject ou body",
    ),
    (
        "must not contain line breaks or control characters",
        "ne doit contenir ni saut de ligne ni caractère de contrôle",
    ),
    (
        "must not contain NUL characters",
        "ne doit pas contenir de caractère NUL",
    ),
    (
        "must be an absolute file path",
        "doit être un chemin de fichier absolu",
    ),
    (
        "must name a file, not a directory",
        "doit désigner un fichier, pas un répertoire",
    ),
];

/// Undo labels without a variable part.
const LABELS: &[(&str, &str)] = &[
    (
        "Offer every version again",
        "Proposer de nouveau toutes les versions",
    ),
    (
        "Change update checks",
        "Modifier la recherche de mises à jour",
    ),
    ("Create draft", "Créer le brouillon"),
    ("Edit draft", "Modifier le brouillon"),
    ("Discard draft", "Supprimer le brouillon"),
    ("Restore draft", "Restaurer le brouillon"),
    ("Attach file", "Joindre le fichier"),
    ("Remove attachment", "Retirer la pièce jointe"),
    ("Send", "Envoi"),
    ("Cancel send", "Annulation de l’envoi"),
    (
        "Change the mailbox's address",
        "Changer l’adresse du dossier",
    ),
    ("Change the sender", "Changer l’expéditeur"),
    ("Key preset", "Jeu de raccourcis"),
    ("Theme", "Thème"),
];

/// Undo labels `<prefix><value>`. Longer prefixes come first.
const PREFIXES: &[(&str, &str)] = &[
    ("Mark all read ", "Tout marquer comme lu "),
    ("Mark unread ", "Marquer comme non lu "),
    ("Mark read ", "Marquer comme lu "),
    ("Move to Trash ", "Mettre à la corbeille "),
    ("Archive ", "Archiver "),
    ("Move ", "Déplacer "),
    ("Unstar ", "Ne plus suivre "),
    ("Star ", "Suivre "),
    ("Add account ", "Ajouter le compte "),
    ("Edit account ", "Modifier le compte "),
    ("Show images from ", "Afficher les images de "),
    ("Hide images from ", "Masquer les images de "),
    ("Skip version ", "Ignorer la version "),
    (
        "Default offline window for ",
        "Fenêtre hors ligne par défaut pour ",
    ),
    ("Keep all of ", "Tout garder hors ligne : "),
    ("Keys of ", "Touches de "),
    ("Rename ", "Renommer "),
    ("Remove ", "Retirer "),
    ("Unpin ", "Détacher "),
    ("Pin ", "Épingler "),
    ("Hide ", "Masquer "),
    ("Show ", "Afficher "),
    ("Set ", "Régler "),
];

fn french_reason(reason: &str) -> String {
    REASONS
        .iter()
        .find(|(english, _)| *english == reason)
        .map_or_else(|| reason.to_owned(), |(_, french)| (*french).to_owned())
}

fn french_entity(entity: &str) -> &str {
    match entity {
        "account" => "compte",
        "attachment" => "pièce jointe",
        "draft" => "brouillon",
        "mailbox" => "dossier",
        "message" => "message",
        "thread" => "discussion",
        other => other,
    }
}

/// `err` as the user reads it in `lang`.
pub fn error_in(lang: Lang, err: &CoreError) -> String {
    match lang {
        Lang::English => err.to_string(),
        Lang::Pseudo => pseudo(&err.to_string()),
        Lang::French => french_error(err),
    }
}

/// The match is exhaustive, so a new error does not compile without its French.
fn french_error(err: &CoreError) -> String {
    match err {
        CoreError::InvalidCommand { reason } => {
            format!("commande non valide : {}", french_reason(reason))
        }
        CoreError::InvalidArgument { field, reason } => {
            format!("argument `{field}` non valide : {}", french_reason(reason))
        }
        CoreError::InvalidReplay { line, reason } => {
            format!("enregistrement non valide à la ligne {line} : {reason}")
        }
        CoreError::NotFound { entity, id } => {
            format!("{} {id} introuvable", french_entity(entity))
        }
        CoreError::AlreadyExists { entity, key } => {
            format!("{} {key} existe déjà", french_entity(entity))
        }
        CoreError::ConfirmationRequired { command } => {
            format!("{command} est irréversible ; répétez avec `confirm: true`")
        }
        CoreError::NoMailboxForRole { account, role } => {
            format!("le compte {account} n’a pas de dossier {role:?}")
        }
        CoreError::OriginNotAllowed { command, origin } => format!(
            "{command} ne peut pas être lancé depuis {origin} ; l’utilisateur doit le faire \
             dans l’interface graphique ou la ligne de commande"
        ),
        CoreError::NotSendable { draft, reason } => format!(
            "le brouillon {draft} ne peut pas être envoyé : {}",
            french_reason(reason)
        ),
        CoreError::NothingToUndo => "rien à annuler".to_owned(),
        CoreError::NothingToRedo => "rien à rétablir".to_owned(),
    }
}

/// The undo label `label` in `lang`.
pub fn label_in(lang: Lang, label: &str) -> String {
    match lang {
        Lang::English => label.to_owned(),
        Lang::Pseudo => pseudo(label),
        Lang::French => french_label(label),
    }
}

fn french_label(label: &str) -> String {
    if let Some((_, french)) = LABELS.iter().find(|(english, _)| *english == label) {
        return (*french).to_owned();
    }
    let keep_newest = label
        .strip_prefix("Keep the newest ")
        .and_then(|rest| rest.strip_suffix(" offline"))
        .and_then(|rest| rest.split_once(" of "));
    if let Some((n, path)) = keep_newest {
        return format!("Garder hors ligne les {n} plus récents de {path}");
    }
    PREFIXES
        .iter()
        .find_map(|(english, french)| {
            let rest = label.strip_prefix(english)?;
            let rest = rest.strip_suffix(" offline").unwrap_or(rest);
            Some(format!("{french}{rest}"))
        })
        .unwrap_or_else(|| label.to_owned())
}

/// A command's one-sentence help in `lang`: settings tips and the palette's help line.
pub fn help_in(lang: Lang, name: CommandName) -> String {
    match lang {
        Lang::English => first_sentence(name),
        Lang::French => commands::french_help(name).to_owned(),
        Lang::Pseudo => pseudo(&first_sentence(name)),
    }
}

/// The first sentence of the command's doc comment.
fn first_sentence(name: CommandName) -> String {
    let text = name
        .description()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let end = text.find(". ").map_or(text.len(), |at| at + 1);
    text.get(..end).unwrap_or_default().to_owned()
}

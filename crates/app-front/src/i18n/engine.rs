//! The engine's texts in a language. `app-core` stays in English: its errors and undo labels
//! are part of the JSON that agents read. The front-ends translate them here, for display.
//! An English text unknown here (a server's message, a new label) is shown as it is.

use app_core::{CommandName, CoreError};

use super::{Lang, commands, pseudo};

/// English reasons of `error_002_invalid_argument` and similar errors, with their French.
const REASONS: &[(&str, &str)] = &[
    (
        "it is being sent and can't be stopped any more",
        "il est en cours d’envoi et ne peut plus être arrêté",
    ),
    ("no send is scheduled", "aucun envoi n’est programmé"),
    ("it is being sent", "il est en cours d’envoi"),
    (
        "a send is scheduled; run cancel_send first",
        "un envoi est programmé ; lancez d’abord cancel_send",
    ),
    (
        "it has no recipients; set to, cc or bcc with update_draft",
        "il n’a pas de destinataire ; indiquez to, cc ou bcc avec update_draft",
    ),
    (
        "no thread is open (open_thread)",
        "aucune discussion n’est ouverte (open_thread)",
    ),
    (
        "no draft is open (open_composer)",
        "aucun brouillon n’est ouvert (open_composer)",
    ),
    ("the pane is not shown", "le volet n’est pas affiché"),
    (
        "must be a message ID without spaces, line breaks or control characters",
        "doit être un identifiant de message sans espace, saut de ligne ni caractère de contrôle",
    ),
    (
        "empty phrase; remove the quotes",
        "expression vide ; retirez les guillemets",
    ),
    (
        "unterminated quote; add a closing `\"`",
        "guillemet non fermé ; ajoutez un `\"` final",
    ),
    (
        "expected a space after the closing `\"`",
        "espace attendu après le `\"` final",
    ),
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

/// Reasons with variable parts: each `{}` of the English is a value, kept in order in the French.
const PATTERNS: &[(&str, &str)] = &[
    ("at column {}: {}", "à la colonne {} : {}"),
    ("{} exists already", "{} existe déjà"),
    (
        "message {} is not in the open conversation",
        "le message {} n’est pas dans la discussion ouverte",
    ),
    (
        "account {} has no folder {}",
        "le compte {} n’a pas de dossier {}",
    ),
    (
        "must be between 1 and {}",
        "doit être compris entre 1 et {}",
    ),
    (
        "must be between 0 and {} (the number of attachments)",
        "doit être compris entre 0 et {} (le nombre de pièces jointes)",
    ),
    (
        "the draft has {} attachment(s)",
        "le brouillon a {} pièce(s) jointe(s)",
    ),
    (
        "expected a folder path like {}",
        "chemin de dossier attendu, par exemple {}",
    ),
    (
        "required for the {} preset",
        "obligatoire pour le préréglage {}",
    ),
    (
        "this install cannot download the update; open {}",
        "cette installation ne peut pas télécharger la mise à jour ; ouvrez {}",
    ),
    (
        "{} is still being checked; try again",
        "{} est encore en cours de vérification ; réessayez",
    ),
    ("`{}:` needs a value", "`{}:` demande une valeur"),
    (
        "unknown `is:{}`; use is:unread, is:read or is:starred",
        "`is:{}` inconnu ; utilisez is:unread, is:read ou is:starred",
    ),
    (
        "unknown `has:{}`; use has:attachment",
        "`has:{}` inconnu ; utilisez has:attachment",
    ),
    (
        "invalid date `{}`; write YYYY-MM-DD",
        "date `{}` non valide ; écrivez AAAA-MM-JJ",
    ),
    (
        "`{}` is not a calendar date",
        "`{}` n’est pas une date du calendrier",
    ),
];

/// The values of `text` if it has the shape of `template`.
fn values<'a>(template: &str, text: &'a str) -> Option<Vec<&'a str>> {
    let mut parts = template.split("{}");
    let mut rest = text.strip_prefix(parts.next()?)?;
    let mut found = Vec::new();
    let parts: Vec<&str> = parts.collect();
    for (at, part) in parts.iter().enumerate() {
        let end = if at + 1 == parts.len() {
            rest.strip_suffix(part)?.len()
        } else {
            rest.find(part)?
        };
        found.push(rest.get(..end)?);
        rest = rest.get(end + part.len()..)?;
    }
    rest.is_empty().then_some(found)
}

/// Undo labels without a variable part.
const LABELS: &[(&str, &str)] = &[
    ("Reply", "Répondre"),
    ("Reply all", "Répondre à tous"),
    ("Forward", "Transférer"),
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
    if let Some((_, french)) = REASONS.iter().find(|(english, _)| *english == reason) {
        return (*french).to_owned();
    }
    PATTERNS
        .iter()
        .find_map(|(english, french)| {
            let values = values(english, reason)?;
            let mut parts = french.split("{}");
            let mut out = parts.next()?.to_owned();
            for (value, part) in values.iter().zip(parts) {
                // A column's reason is itself a reason.
                out.push_str(&french_reason(value));
                out.push_str(part);
            }
            Some(out)
        })
        .unwrap_or_else(|| reason.to_owned())
}

fn french_role(role: &str) -> &str {
    match role {
        "Inbox" => "de réception",
        "Sent" => "des messages envoyés",
        "Drafts" => "de brouillons",
        "Trash" => "corbeille",
        "Archive" => "d’archives",
        "Junk" => "de courrier indésirable",
        "All" => "de tout le courrier",
        "Flagged" => "de messages suivis",
        other => other,
    }
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

/// `err` as the user reads it in `lang`. The code stays as it is in every language.
pub fn error_in(lang: Lang, err: &CoreError) -> String {
    match lang {
        Lang::English => err.to_string(),
        Lang::Pseudo => {
            let text = err.to_string();
            let prefix = format!("{}: ", err.code());
            let message = text.strip_prefix(&prefix).unwrap_or(&text);
            format!("{prefix}{}", pseudo(message))
        }
        Lang::French => format!("{} : {}", err.code(), french_error(err)),
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
            format!(
                "le compte {account} n’a pas de dossier {}",
                french_role(&format!("{role:?}"))
            )
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

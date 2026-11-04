//! The titles of the command registry per language. The `match` is exhaustive, so a new command
//! does not compile without its translation.

use app_core::CommandName;

/// The French title of `name`.
pub(super) const fn french(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Afficher les paramètres",
        CommandName::SetSetting => "Changer un paramètre",
        CommandName::SetKeybinding => "Changer les touches d’une commande",
        CommandName::SetKeyPreset => "Choisir les raccourcis (Gmail, Vim)",
        CommandName::SetTheme => "Choisir le thème",
        CommandName::OpenSettings => "Paramètres",
        CommandName::CloseSettings => "Fermer les paramètres",
        CommandName::Undo => "Annuler",
        CommandName::Redo => "Rétablir",
        CommandName::OpenPalette => "Ouvrir la palette de commandes",
        CommandName::ClosePalette => "Fermer la palette de commandes",
        CommandName::AddAccount => "Ajouter un compte",
        CommandName::TestAccount => "Tester la connexion du compte",
        CommandName::DiscoverServers => "Trouver les serveurs d’une adresse",
        CommandName::UpdateAccount => "Modifier le compte",
        CommandName::SetPassword => "Changer le mot de passe du compte",
        CommandName::SetIdentity => "Ajouter ou modifier une adresse d’envoi",
        CommandName::RemoveIdentity => "Retirer une adresse d’envoi",
        CommandName::SetMailboxIdentity => "Envoyer depuis une adresse pour ce dossier",
        CommandName::CreateFolder => "Nouveau dossier",
        CommandName::RenameFolder => "Renommer le dossier",
        CommandName::DeleteFolder => "Supprimer le dossier",
        CommandName::EmptyTrash => "Vider la corbeille",
        CommandName::SetDraftFrom => "Envoyer ce brouillon depuis…",
        CommandName::RemoveAccount => "Supprimer le compte",
        CommandName::SyncNow => "Synchroniser",
        CommandName::ListMailboxes => "Lister les dossiers",
        CommandName::SetSyncWindow => "Régler le courrier gardé hors ligne",
        CommandName::ListThreads => "Lister les discussions",
        CommandName::OpenMailbox => "Aller au dossier",
        CommandName::OpenUnifiedInbox => "Aller aux dossiers unifiés",
        CommandName::SelectThreads => "Sélectionner des discussions",
        CommandName::CheckThreads => "Sélectionner toutes les discussions",
        CommandName::UncheckThreads => "Effacer les discussions cochées",
        CommandName::OpenThread => "Ouvrir la discussion",
        CommandName::CloseThread => "Fermer la discussion",
        CommandName::ExpandMessages => "Développer tous les messages",
        CommandName::CollapseMessages => "Réduire tous les messages",
        CommandName::GetThread => "Afficher la discussion",
        CommandName::GetMessage => "Afficher le message",
        CommandName::ShowImagesFrom => "Afficher les images de l’expéditeur",
        CommandName::HideImagesFrom => "Masquer les images de l’expéditeur",
        CommandName::PinMailbox => "Ajouter aux dossiers préférés",
        CommandName::UnpinMailbox => "Retirer le dossier",
        CommandName::HideMailbox => "Masquer le dossier",
        CommandName::ShowMailbox => "Afficher le dossier masqué",
        CommandName::SaveAttachments => "Enregistrer les pièces jointes",
        CommandName::MarkRead => "Marquer comme lu",
        CommandName::MarkMailboxRead => "Tout marquer comme lu",
        CommandName::MarkUnread => "Marquer comme non lu",
        CommandName::Star => "Ajouter un suivi",
        CommandName::Unstar => "Retirer le suivi",
        CommandName::PinThreads => "Épingler en haut",
        CommandName::UnpinThreads => "Détacher",
        CommandName::ArchiveThreads => "Archiver",
        CommandName::TrashThreads => "Mettre à la corbeille",
        CommandName::MarkJunk => "Signaler comme indésirable",
        CommandName::MarkNotJunk => "Pas indésirable",
        CommandName::MoveThreads => "Déplacer vers…",
        CommandName::ListPendingOps => "Afficher les modifications en attente",
        CommandName::RetryPendingOps => "Réessayer les modifications en attente",
        CommandName::DiscardPendingOp => "Abandonner la modification en attente",
        CommandName::CreateDraft => "Rédiger",
        CommandName::Reply => "Répondre",
        CommandName::ReplyAll => "Répondre à tous",
        CommandName::Forward => "Transférer",
        CommandName::UpdateDraft => "Modifier le brouillon",
        CommandName::DiscardDraft => "Abandonner le brouillon",
        CommandName::RestoreDraft => "Restaurer le brouillon",
        CommandName::AttachFile => "Joindre un fichier",
        CommandName::RemoveAttachment => "Supprimer la pièce jointe",
        CommandName::SendDraft => "Envoyer",
        CommandName::CancelSend => "Annuler l’envoi",
        CommandName::Search => "Rechercher dans le courrier",
        CommandName::ListContacts => "Lister les contacts",
        CommandName::SetSearch => "Rechercher",
        CommandName::ClearSearch => "Effacer la recherche",
        CommandName::OpenComposer => "Ouvrir le brouillon",
        CommandName::CloseComposer => "Fermer la rédaction",
        CommandName::FocusPane => "Activer un panneau",
        CommandName::SetListFilter => "Filtrer la liste",
        CommandName::FocusMailbox => "Focus sur ce dossier",
        CommandName::ClearFocus => "Quitter le focus",
        CommandName::CollapseFolders => "Réduire les dossiers",
        CommandName::ExpandFolders => "Développer les dossiers",
        CommandName::ToggleSidebar => "Afficher ou masquer la barre latérale",
        CommandName::GetView => "Afficher l’état de la vue",
        CommandName::CompactStore => "Compacter le stockage local",
        CommandName::CheckForUpdates => "Rechercher des mises à jour",
        CommandName::DownloadUpdate => "Télécharger la mise à jour",
        CommandName::InstallUpdate => "Installer la mise à jour",
        CommandName::SkipUpdateVersion => "Ignorer cette version",
        CommandName::SetUpdatePolicy => "Vérification des mises à jour",
        CommandName::ListApprovals => "Afficher les demandes d’envoi",
        CommandName::ApproveRequest => "Approuver la demande d’envoi",
        CommandName::RejectRequest => "Rejeter la demande d’envoi",
    }
}

/// The French one-sentence help of `name`.
pub(super) const fn french_help(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Afficher les paramètres actuels.",
        CommandName::SetKeybinding => {
            "Associer des touches à une commande, à la place de celles du jeu de raccourcis."
        }
        CommandName::SetKeyPreset => {
            "Choisir le jeu de raccourcis de départ : `gmail` (par défaut) ou `vim`."
        }
        CommandName::SetTheme => {
            "Choisir l’apparence : le mode (système, clair, sombre) et le thème de chaque mode."
        }
        CommandName::SetSetting => "Changer un paramètre.",
        CommandName::Undo => "Annuler la dernière commande annulable.",
        CommandName::Redo => "Rétablir la dernière commande annulée.",
        CommandName::OpenPalette => "Ouvrir la palette de commandes.",
        CommandName::ClosePalette => "Fermer la palette de commandes.",
        CommandName::AddAccount => "Ajouter un compte de messagerie IMAP/SMTP.",
        CommandName::DiscoverServers => {
            "Chercher les serveurs IMAP et SMTP du domaine d’une adresse."
        }
        CommandName::TestAccount => {
            "Vérifier un compte : connexion aux serveurs IMAP et SMTP avec le mot de passe enregistré."
        }
        CommandName::UpdateAccount => "Changer le nom affiché ou les serveurs d’un compte.",
        CommandName::SetPassword => "Remplacer le mot de passe d’un compte.",
        CommandName::SetIdentity => {
            "Ajouter une adresse d’envoi (un alias), ou changer son nom ou sa signature."
        }
        CommandName::RemoveIdentity => {
            "Retirer un alias, ou le nom et la signature de l’adresse du compte."
        }
        CommandName::SetMailboxIdentity => {
            "Choisir l’adresse d’envoi des nouveaux brouillons écrits depuis un dossier."
        }
        CommandName::CreateFolder => "Créer un dossier sur le serveur.",
        CommandName::RenameFolder => {
            "Renommer ou déplacer un dossier sur le serveur, avec ses sous-dossiers."
        }
        CommandName::DeleteFolder => "Supprimer un dossier du serveur, avec tous ses messages.",
        CommandName::EmptyTrash => {
            "Supprimer définitivement tous les messages de la corbeille d’un compte, aussi sur le serveur."
        }
        CommandName::SetDraftFrom => "Choisir l’adresse d’envoi d’un brouillon.",
        CommandName::RemoveAccount => "Retirer un compte et effacer son mot de passe du trousseau.",
        CommandName::SyncNow => {
            "Synchroniser maintenant : lister les dossiers et récupérer le nouveau courrier."
        }
        CommandName::ListMailboxes => {
            "Lister les dossiers en cache, avec leur rôle et leurs compteurs."
        }
        CommandName::ListThreads => {
            "Lister les discussions d’un dossier, les plus récentes d’abord."
        }
        CommandName::SetSyncWindow => {
            "Choisir combien de messages récents d’un dossier garder en cache local."
        }
        CommandName::OpenUnifiedInbox => {
            "Afficher les boîtes de réception de tous les comptes (boîte unifiée)."
        }
        CommandName::OpenMailbox => "Afficher un dossier dans la liste des discussions.",
        CommandName::SelectThreads => "Remplacer la sélection de discussions.",
        CommandName::CheckThreads => "Cocher des discussions pour agir sur plusieurs à la fois.",
        CommandName::UncheckThreads => "Décocher des discussions.",
        CommandName::OpenThread => "Ouvrir une discussion dans le lecteur.",
        CommandName::CloseThread => "Fermer la discussion affichée dans le lecteur.",
        CommandName::ExpandMessages => "Déplier des messages de la discussion ouverte.",
        CommandName::CollapseMessages => {
            "Replier des messages de la discussion ouverte sur une ligne."
        }
        CommandName::GetThread => {
            "Afficher une discussion et tous ses messages, les plus anciens d’abord."
        }
        CommandName::GetMessage => "Afficher un message.",
        CommandName::ShowImagesFrom => {
            "Afficher les images intégrées aux messages de cet expéditeur."
        }
        CommandName::HideImagesFrom => {
            "Ne plus afficher les images intégrées aux messages de cet expéditeur."
        }
        CommandName::PinMailbox => {
            "Épingler un dossier dans les *Favoris* en haut de la barre latérale."
        }
        CommandName::UnpinMailbox => "Retirer un dossier des *Favoris* de la barre latérale.",
        CommandName::HideMailbox => "Masquer un dossier dans l’arborescence de la barre latérale.",
        CommandName::ShowMailbox => {
            "Afficher de nouveau un dossier dans l’arborescence de la barre latérale."
        }
        CommandName::SaveAttachments => {
            "Enregistrer les pièces jointes d’un message téléchargé dans un dossier de cet ordinateur."
        }
        CommandName::MarkRead => "Marquer des discussions ou des messages comme lus.",
        CommandName::MarkMailboxRead => {
            "Marquer comme lus tous les messages d’un dossier, y compris ceux hors du cache."
        }
        CommandName::MarkUnread => "Marquer des discussions ou des messages comme non lus.",
        CommandName::Star => "Suivre des discussions ou des messages.",
        CommandName::Unstar => "Ne plus suivre des discussions ou des messages.",
        CommandName::PinThreads => "Épingler des discussions en haut des listes qui les affichent.",
        CommandName::UnpinThreads => "Détacher des discussions ou des messages.",
        CommandName::ArchiveThreads => "Archiver des discussions ou des messages.",
        CommandName::TrashThreads => "Mettre des discussions ou des messages à la corbeille.",
        CommandName::MarkJunk => "Déplacer des discussions ou des messages dans les indésirables.",
        CommandName::MarkNotJunk => "Remettre des indésirables dans la boîte de réception.",
        CommandName::MoveThreads => {
            "Déplacer des discussions ou des messages vers un autre dossier du même compte."
        }
        CommandName::ListPendingOps => {
            "Lister les modifications locales qui n’ont pas encore atteint le serveur."
        }
        CommandName::RetryPendingOps => "Relancer maintenant les opérations en attente.",
        CommandName::DiscardPendingOp => {
            "Abandonner une opération en attente : sa modification est annulée localement."
        }
        CommandName::CreateDraft => "Commencer un nouveau message depuis un compte.",
        CommandName::Reply => "Commencer une réponse à l’expéditeur d’un message.",
        CommandName::ReplyAll => "Répondre à l’expéditeur et à tous les autres destinataires.",
        CommandName::Forward => "Commencer le transfert d’un message.",
        CommandName::UpdateDraft => "Modifier les champs d’un brouillon.",
        CommandName::DiscardDraft => "Supprimer un brouillon.",
        CommandName::AttachFile => "Joindre un fichier local à un brouillon.",
        CommandName::RemoveAttachment => {
            "Retirer une pièce jointe d’un brouillon (le fichier n’est pas touché)."
        }
        CommandName::SendDraft => "Envoyer un brouillon.",
        CommandName::CancelSend => {
            "Arrêter un envoi programmé avant son départ ; le brouillon redevient modifiable."
        }
        CommandName::RestoreDraft => "Rétablir un brouillon supprimé tel qu’il était.",
        CommandName::ListContacts => "Lister les personnes présentes dans le courrier en cache.",
        CommandName::Search => "Chercher dans le courrier en cache.",
        CommandName::SetSearch => {
            "Afficher des résultats de recherche à la place du dossier ouvert."
        }
        CommandName::ClearSearch => {
            "Quitter la recherche : la liste affiche de nouveau le dossier ouvert."
        }
        CommandName::OpenComposer => {
            "Afficher un brouillon dans la fenêtre de rédaction et y placer le focus."
        }
        CommandName::CloseComposer => "Fermer la fenêtre de rédaction.",
        CommandName::OpenSettings => "Afficher la page des paramètres, à une section donnée.",
        CommandName::CloseSettings => "Fermer la page des paramètres.",
        CommandName::FocusPane => "Placer le focus clavier sur un volet.",
        CommandName::SetListFilter => "Filtrer la liste : non lus, suivis, pièces jointes.",
        CommandName::FocusMailbox => {
            "Se concentrer sur un dossier ou un compte : la barre latérale n’affiche que lui."
        }
        CommandName::ClearFocus => {
            "Quitter le mode concentré : la barre latérale affiche tout de nouveau."
        }
        CommandName::CollapseFolders => {
            "Replier des dossiers dans l’arborescence de la barre latérale."
        }
        CommandName::ExpandFolders => {
            "Déplier des dossiers dans l’arborescence de la barre latérale."
        }
        CommandName::ToggleSidebar => "Replier ou déplier la barre latérale.",
        CommandName::CompactStore => {
            "Compacter le stockage local et supprimer les corps de messages orphelins."
        }
        CommandName::GetView => "Afficher ce que l’utilisateur regarde dans l’interface graphique.",
        CommandName::CheckForUpdates => "Chercher maintenant une nouvelle version de dail.",
        CommandName::DownloadUpdate => {
            "Télécharger la mise à jour disponible et vérifier sa taille et sa signature."
        }
        CommandName::InstallUpdate => "Installer la mise à jour téléchargée.",
        CommandName::SkipUpdateVersion => "Ignorer une version : elle ne sera plus proposée.",
        CommandName::SetUpdatePolicy => {
            "Changer la recherche périodique de mises à jour et son intervalle."
        }
        CommandName::ListApprovals => {
            "Lister les demandes d’envoi d’agents qui attendent votre décision."
        }
        CommandName::ApproveRequest => "Approuver la demande d’envoi d’un agent.",
        CommandName::RejectRequest => "Refuser la demande d’envoi d’un agent.",
    }
}

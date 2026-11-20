//! The titles and one-sentence help of the command registry per language. The `match` is exhaustive, so a new command
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
        CommandName::TrustCertificate => "Faire confiance au certificat d’un serveur",
        CommandName::ForgetCertificate => "Oublier un certificat de confiance",
        CommandName::SetMailboxIdentity => "Envoyer depuis une adresse pour ce dossier",
        CommandName::CreateFolder => "Nouveau dossier",
        CommandName::RenameFolder => "Renommer le dossier",
        CommandName::DeleteFolder => "Supprimer le dossier",
        CommandName::EmptyTrash => "Vider la corbeille",
        CommandName::EmptyJunk => "Vider les indésirables",
        CommandName::SetDraftFrom => "Envoyer ce brouillon depuis…",
        CommandName::RemoveAccount => "Supprimer le compte",
        CommandName::SyncNow => "Synchroniser",
        CommandName::PullSettings => "Récupérer les paramètres du serveur",
        CommandName::PushSettings => "Envoyer les paramètres au serveur",
        CommandName::SetSettingsAccount => "Choisir le compte des paramètres",
        CommandName::ListRules => "Lister les règles de tri",
        CommandName::SaveRule => "Créer une règle de tri",
        CommandName::DeleteRule => "Supprimer une règle de tri",
        CommandName::EnableRule => "Activer ou désactiver une règle de tri",
        CommandName::ApplyRules => "Appliquer les règles de tri à un dossier",
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
        CommandName::EditDraft => "Modifier un brouillon de la conversation",
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
        CommandName::TrustCertificate => {
            "Faire confiance, pour un compte, au certificat auto-signé ou à l’autorité privée qu’un de ses serveurs a présenté."
        }
        CommandName::ForgetCertificate => {
            "Ne plus faire confiance à un certificat accepté pour un compte."
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
        CommandName::EmptyJunk => {
            "Supprimer définitivement tous les messages indésirables d’un compte, aussi sur le serveur."
        }
        CommandName::SetDraftFrom => "Choisir l’adresse d’envoi d’un brouillon.",
        CommandName::RemoveAccount => "Retirer un compte et effacer son mot de passe du trousseau.",
        CommandName::PullSettings => {
            "Récupérer du serveur de courrier les paramètres partagés entre appareils et les fusionner."
        }
        CommandName::PushSettings => {
            "Enregistrer les paramètres partagés sur le serveur de courrier pour les autres appareils."
        }
        CommandName::SetSettingsAccount => {
            "Choisir le compte dont le serveur garde les paramètres partagés entre appareils."
        }
        CommandName::ListRules => "Lister les règles qui trient le nouveau courrier.",
        CommandName::SaveRule => {
            "Créer ou remplacer une règle qui trie le nouveau courrier ; elle ne peut ni transférer ni répondre."
        }
        CommandName::DeleteRule => "Supprimer une règle de tri, sur tous les appareils.",
        CommandName::EnableRule => "Activer ou désactiver une règle de tri sans la supprimer.",
        CommandName::ApplyRules => {
            "Appliquer maintenant les règles de tri au courrier en cache d’un dossier."
        }
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
        CommandName::EditDraft => {
            "Modifier un brouillon affiché dans une conversation, même écrit sur un autre appareil."
        }
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

/// The Spanish title of `name`.
pub(super) const fn spanish(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Mostrar los ajustes",
        CommandName::SetSetting => "Cambiar un ajuste",
        CommandName::SetKeybinding => "Cambiar las teclas de un comando",
        CommandName::SetKeyPreset => "Elegir los atajos (Gmail, Vim)",
        CommandName::SetTheme => "Elegir el tema",
        CommandName::OpenSettings => "Ajustes",
        CommandName::CloseSettings => "Cerrar los ajustes",
        CommandName::Undo => "Deshacer",
        CommandName::Redo => "Rehacer",
        CommandName::OpenPalette => "Abrir la paleta de comandos",
        CommandName::ClosePalette => "Cerrar la paleta de comandos",
        CommandName::AddAccount => "Añadir una cuenta",
        CommandName::TestAccount => "Probar la conexión de la cuenta",
        CommandName::DiscoverServers => "Buscar los servidores de una dirección",
        CommandName::UpdateAccount => "Editar la cuenta",
        CommandName::SetPassword => "Cambiar la contraseña de la cuenta",
        CommandName::SetIdentity => "Añadir o editar una dirección de envío",
        CommandName::RemoveIdentity => "Quitar una dirección de envío",
        CommandName::TrustCertificate => "Confiar en el certificado de un servidor",
        CommandName::ForgetCertificate => "Olvidar un certificado de confianza",
        CommandName::SetMailboxIdentity => "Enviar desde una dirección para esta carpeta",
        CommandName::CreateFolder => "Carpeta nueva",
        CommandName::RenameFolder => "Cambiar el nombre de la carpeta",
        CommandName::DeleteFolder => "Eliminar la carpeta",
        CommandName::EmptyTrash => "Vaciar la papelera",
        CommandName::EmptyJunk => "Vaciar el spam",
        CommandName::SetDraftFrom => "Enviar este borrador desde…",
        CommandName::RemoveAccount => "Quitar la cuenta",
        CommandName::SyncNow => "Sincronizar ahora",
        CommandName::PullSettings => "Leer los ajustes del servidor",
        CommandName::PushSettings => "Guardar los ajustes en el servidor",
        CommandName::SetSettingsAccount => "Elegir la cuenta de los ajustes",
        CommandName::ListRules => "Listar las reglas de clasificación",
        CommandName::SaveRule => "Crear una regla de clasificación",
        CommandName::DeleteRule => "Eliminar una regla de clasificación",
        CommandName::EnableRule => "Activar o desactivar una regla de clasificación",
        CommandName::ApplyRules => "Aplicar las reglas de clasificación a una carpeta",
        CommandName::ListMailboxes => "Listar las carpetas",
        CommandName::SetSyncWindow => "Elegir el correo que se guarda sin conexión",
        CommandName::ListThreads => "Listar las conversaciones",
        CommandName::OpenMailbox => "Ir a la carpeta",
        CommandName::OpenUnifiedInbox => "Ir a todas las bandejas de entrada",
        CommandName::SelectThreads => "Seleccionar conversaciones",
        CommandName::CheckThreads => "Seleccionar todas las conversaciones",
        CommandName::UncheckThreads => "Borrar las conversaciones marcadas",
        CommandName::OpenThread => "Abrir la conversación",
        CommandName::CloseThread => "Cerrar la conversación",
        CommandName::ExpandMessages => "Expandir todos los mensajes",
        CommandName::CollapseMessages => "Contraer todos los mensajes",
        CommandName::GetThread => "Mostrar la conversación",
        CommandName::GetMessage => "Mostrar el mensaje",
        CommandName::ShowImagesFrom => "Mostrar las imágenes del remitente",
        CommandName::HideImagesFrom => "Ocultar las imágenes del remitente",
        CommandName::PinMailbox => "Fijar en Favoritos",
        CommandName::UnpinMailbox => "Quitar la carpeta de Favoritos",
        CommandName::HideMailbox => "Ocultar la carpeta",
        CommandName::ShowMailbox => "Mostrar la carpeta oculta",
        CommandName::SaveAttachments => "Guardar los adjuntos",
        CommandName::MarkRead => "Marcar como leído",
        CommandName::MarkMailboxRead => "Marcar todo como leído",
        CommandName::MarkUnread => "Marcar como no leído",
        CommandName::Star => "Destacar",
        CommandName::Unstar => "Quitar destacado",
        CommandName::PinThreads => "Fijar arriba",
        CommandName::UnpinThreads => "Desfijar",
        CommandName::ArchiveThreads => "Archivar",
        CommandName::TrashThreads => "Mover a la papelera",
        CommandName::MarkJunk => "Marcar como spam",
        CommandName::MarkNotJunk => "No es spam",
        CommandName::MoveThreads => "Mover a…",
        CommandName::ListPendingOps => "Mostrar los cambios pendientes",
        CommandName::RetryPendingOps => "Reintentar los cambios pendientes",
        CommandName::DiscardPendingOp => "Descartar el cambio pendiente",
        CommandName::CreateDraft => "Redactar",
        CommandName::Reply => "Responder",
        CommandName::ReplyAll => "Responder a todos",
        CommandName::Forward => "Reenviar",
        CommandName::UpdateDraft => "Editar el borrador",
        CommandName::DiscardDraft => "Descartar el borrador",
        CommandName::EditDraft => "Editar un borrador de la conversación",
        CommandName::RestoreDraft => "Restaurar el borrador",
        CommandName::AttachFile => "Adjuntar un archivo",
        CommandName::RemoveAttachment => "Quitar el adjunto",
        CommandName::SendDraft => "Enviar",
        CommandName::CancelSend => "Cancelar el envío",
        CommandName::Search => "Buscar en el correo",
        CommandName::ListContacts => "Listar los contactos",
        CommandName::SetSearch => "Buscar",
        CommandName::ClearSearch => "Borrar la búsqueda",
        CommandName::OpenComposer => "Abrir el borrador",
        CommandName::CloseComposer => "Cerrar la redacción",
        CommandName::FocusPane => "Activar un panel",
        CommandName::SetListFilter => "Filtrar la lista",
        CommandName::FocusMailbox => "Centrarse en esta carpeta",
        CommandName::ClearFocus => "Salir del enfoque",
        CommandName::CollapseFolders => "Contraer las carpetas",
        CommandName::ExpandFolders => "Expandir las carpetas",
        CommandName::ToggleSidebar => "Mostrar u ocultar la barra lateral",
        CommandName::GetView => "Mostrar el estado de la vista",
        CommandName::CompactStore => "Compactar el almacenamiento local",
        CommandName::CheckForUpdates => "Buscar actualizaciones",
        CommandName::DownloadUpdate => "Descargar la actualización",
        CommandName::InstallUpdate => "Instalar la actualización",
        CommandName::SkipUpdateVersion => "Omitir esta versión",
        CommandName::SetUpdatePolicy => "Búsqueda de actualizaciones",
        CommandName::ListApprovals => "Mostrar las solicitudes de envío",
        CommandName::ApproveRequest => "Aprobar la solicitud de envío",
        CommandName::RejectRequest => "Rechazar la solicitud de envío",
    }
}

/// The Romanian title of `name`.
pub(super) const fn romanian(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Afișați setările",
        CommandName::SetSetting => "Schimbați o setare",
        CommandName::SetKeybinding => "Schimbați tastele unei comenzi",
        CommandName::SetKeyPreset => "Alegeți comenzile rapide (Gmail, Vim)",
        CommandName::SetTheme => "Alegeți tema",
        CommandName::OpenSettings => "Setări",
        CommandName::CloseSettings => "Închideți setările",
        CommandName::Undo => "Anulați",
        CommandName::Redo => "Refaceți",
        CommandName::OpenPalette => "Deschideți paleta de comenzi",
        CommandName::ClosePalette => "Închideți paleta de comenzi",
        CommandName::AddAccount => "Adăugați un cont",
        CommandName::TestAccount => "Testați conexiunea contului",
        CommandName::DiscoverServers => "Găsiți serverele unei adrese",
        CommandName::UpdateAccount => "Editați contul",
        CommandName::SetPassword => "Schimbați parola contului",
        CommandName::SetIdentity => "Adăugați sau editați o adresă de trimitere",
        CommandName::RemoveIdentity => "Eliminați o adresă de trimitere",
        CommandName::TrustCertificate => "Acceptați certificatul unui server",
        CommandName::ForgetCertificate => "Uitați un certificat acceptat",
        CommandName::SetMailboxIdentity => "Trimiteți de la o adresă pentru acest dosar",
        CommandName::CreateFolder => "Dosar nou",
        CommandName::RenameFolder => "Redenumiți dosarul",
        CommandName::DeleteFolder => "Ștergeți dosarul",
        CommandName::EmptyTrash => "Goliți coșul de gunoi",
        CommandName::EmptyJunk => "Goliți dosarul spam",
        CommandName::SetDraftFrom => "Trimiteți această ciornă de la…",
        CommandName::RemoveAccount => "Eliminați contul",
        CommandName::SyncNow => "Sincronizați acum",
        CommandName::PullSettings => "Citiți setările de pe server",
        CommandName::PushSettings => "Salvați setările pe server",
        CommandName::SetSettingsAccount => "Alegeți contul setărilor",
        CommandName::ListRules => "Listați regulile de sortare",
        CommandName::SaveRule => "Creați o regulă de sortare",
        CommandName::DeleteRule => "Ștergeți o regulă de sortare",
        CommandName::EnableRule => "Activați sau dezactivați o regulă de sortare",
        CommandName::ApplyRules => "Aplicați regulile de sortare pe un dosar",
        CommandName::ListMailboxes => "Listați dosarele",
        CommandName::SetSyncWindow => "Alegeți e-mailurile păstrate offline",
        CommandName::ListThreads => "Listați conversațiile",
        CommandName::OpenMailbox => "Mergeți la dosar",
        CommandName::OpenUnifiedInbox => "Mergeți la toate căsuțele de intrare",
        CommandName::SelectThreads => "Selectați conversații",
        CommandName::CheckThreads => "Selectați toate conversațiile",
        CommandName::UncheckThreads => "Debifați conversațiile bifate",
        CommandName::OpenThread => "Deschideți conversația",
        CommandName::CloseThread => "Închideți conversația",
        CommandName::ExpandMessages => "Extindeți toate mesajele",
        CommandName::CollapseMessages => "Restrângeți toate mesajele",
        CommandName::GetThread => "Afișați conversația",
        CommandName::GetMessage => "Afișați mesajul",
        CommandName::ShowImagesFrom => "Afișați imaginile expeditorului",
        CommandName::HideImagesFrom => "Ascundeți imaginile expeditorului",
        CommandName::PinMailbox => "Fixați în Favorite",
        CommandName::UnpinMailbox => "Eliminați dosarul din Favorite",
        CommandName::HideMailbox => "Ascundeți dosarul",
        CommandName::ShowMailbox => "Afișați dosarul ascuns",
        CommandName::SaveAttachments => "Salvați atașamentele",
        CommandName::MarkRead => "Marcați ca citit",
        CommandName::MarkMailboxRead => "Marcați tot ca citit",
        CommandName::MarkUnread => "Marcați ca necitit",
        CommandName::Star => "Marcați cu stea",
        CommandName::Unstar => "Eliminați steaua",
        CommandName::PinThreads => "Fixați sus",
        CommandName::UnpinThreads => "Anulați fixarea",
        CommandName::ArchiveThreads => "Arhivați",
        CommandName::TrashThreads => "Mutați în coșul de gunoi",
        CommandName::MarkJunk => "Marcați ca spam",
        CommandName::MarkNotJunk => "Nu este spam",
        CommandName::MoveThreads => "Mutați în…",
        CommandName::ListPendingOps => "Afișați modificările în așteptare",
        CommandName::RetryPendingOps => "Reîncercați modificările în așteptare",
        CommandName::DiscardPendingOp => "Renunțați la modificarea în așteptare",
        CommandName::CreateDraft => "Scrieți",
        CommandName::Reply => "Răspundeți",
        CommandName::ReplyAll => "Răspundeți tuturor",
        CommandName::Forward => "Redirecționați",
        CommandName::UpdateDraft => "Editați ciorna",
        CommandName::DiscardDraft => "Renunțați la ciornă",
        CommandName::EditDraft => "Editați o ciornă din conversație",
        CommandName::RestoreDraft => "Restabiliți ciorna",
        CommandName::AttachFile => "Atașați un fișier",
        CommandName::RemoveAttachment => "Eliminați atașamentul",
        CommandName::SendDraft => "Trimiteți",
        CommandName::CancelSend => "Anulați trimiterea",
        CommandName::Search => "Căutați în e-mailuri",
        CommandName::ListContacts => "Listați contactele",
        CommandName::SetSearch => "Căutați",
        CommandName::ClearSearch => "Ștergeți căutarea",
        CommandName::OpenComposer => "Deschideți ciorna",
        CommandName::CloseComposer => "Închideți editorul de mesaj",
        CommandName::FocusPane => "Activați un panou",
        CommandName::SetListFilter => "Filtrați lista",
        CommandName::FocusMailbox => "Concentrare pe acest dosar",
        CommandName::ClearFocus => "Ieșiți din concentrare",
        CommandName::CollapseFolders => "Restrângeți dosarele",
        CommandName::ExpandFolders => "Extindeți dosarele",
        CommandName::ToggleSidebar => "Afișați sau ascundeți bara laterală",
        CommandName::GetView => "Afișați starea vizualizării",
        CommandName::CompactStore => "Compactați stocarea locală",
        CommandName::CheckForUpdates => "Căutați actualizări",
        CommandName::DownloadUpdate => "Descărcați actualizarea",
        CommandName::InstallUpdate => "Instalați actualizarea",
        CommandName::SkipUpdateVersion => "Omiteți această versiune",
        CommandName::SetUpdatePolicy => "Căutarea actualizărilor",
        CommandName::ListApprovals => "Afișați cererile de trimitere",
        CommandName::ApproveRequest => "Aprobați cererea de trimitere",
        CommandName::RejectRequest => "Respingeți cererea de trimitere",
    }
}

/// The Spanish one-sentence help of `name`.
pub(super) const fn spanish_help(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Mostrar los ajustes actuales.",
        CommandName::SetSetting => "Cambiar un ajuste.",
        CommandName::SetKeybinding => {
            "Asignar teclas a un comando, en lugar de las del preajuste de atajos."
        }
        CommandName::SetKeyPreset => {
            "Elegir el preajuste de atajos de partida: `gmail` (predeterminado) o `vim`."
        }
        CommandName::SetTheme => {
            "Elegir la apariencia: el modo (sistema, claro, oscuro) y el tema de cada modo."
        }
        CommandName::OpenSettings => "Mostrar la página de ajustes, en una sección concreta.",
        CommandName::CloseSettings => "Cerrar la página de ajustes.",
        CommandName::Undo => "Deshacer el último comando que se puede deshacer.",
        CommandName::Redo => "Rehacer el último comando deshecho.",
        CommandName::OpenPalette => "Abrir la paleta de comandos.",
        CommandName::ClosePalette => "Cerrar la paleta de comandos.",
        CommandName::AddAccount => "Añadir una cuenta de correo IMAP/SMTP.",
        CommandName::TestAccount => {
            "Comprobar una cuenta: conectarse a los servidores IMAP y SMTP con la contraseña guardada."
        }
        CommandName::DiscoverServers => {
            "Buscar los servidores IMAP y SMTP del dominio de una dirección."
        }
        CommandName::UpdateAccount => "Cambiar el nombre visible o los servidores de una cuenta.",
        CommandName::SetPassword => "Sustituir la contraseña de una cuenta.",
        CommandName::SetIdentity => {
            "Añadir una dirección de envío (un alias) o cambiar su nombre o su firma."
        }
        CommandName::RemoveIdentity => {
            "Quitar un alias, o el nombre y la firma de la dirección de la cuenta."
        }
        CommandName::TrustCertificate => {
            "Confiar, para una cuenta, en el certificado autofirmado o la autoridad privada que presentó uno de sus servidores."
        }
        CommandName::ForgetCertificate => {
            "Dejar de confiar en un certificado aceptado para una cuenta."
        }
        CommandName::SetMailboxIdentity => {
            "Elegir la dirección de envío de los borradores nuevos escritos desde una carpeta."
        }
        CommandName::CreateFolder => "Crear una carpeta en el servidor.",
        CommandName::RenameFolder => {
            "Cambiar el nombre de una carpeta del servidor o moverla, con sus subcarpetas."
        }
        CommandName::DeleteFolder => "Eliminar una carpeta del servidor, con todos sus mensajes.",
        CommandName::EmptyTrash => {
            "Eliminar para siempre todos los mensajes de la papelera de una cuenta, también en el servidor."
        }
        CommandName::EmptyJunk => {
            "Eliminar para siempre todos los mensajes de spam de una cuenta, también en el servidor."
        }
        CommandName::SetDraftFrom => "Elegir la dirección de envío de un borrador.",
        CommandName::RemoveAccount => "Quitar una cuenta y borrar su contraseña del llavero.",
        CommandName::SyncNow => {
            "Sincronizar ahora: listar las carpetas y descargar el correo nuevo."
        }
        CommandName::PullSettings => {
            "Leer del servidor de correo los ajustes compartidos entre dispositivos y combinarlos."
        }
        CommandName::PushSettings => {
            "Guardar los ajustes compartidos en el servidor de correo para los demás dispositivos."
        }
        CommandName::SetSettingsAccount => {
            "Elegir la cuenta cuyo servidor guarda los ajustes compartidos entre dispositivos."
        }
        CommandName::ListRules => "Listar las reglas que clasifican el correo nuevo.",
        CommandName::SaveRule => {
            "Crear o sustituir una regla que clasifica el correo nuevo; no puede reenviar ni responder."
        }
        CommandName::DeleteRule => {
            "Eliminar una regla de clasificación, en todos los dispositivos."
        }
        CommandName::EnableRule => {
            "Activar o desactivar una regla de clasificación sin eliminarla."
        }
        CommandName::ApplyRules => {
            "Aplicar ahora las reglas de clasificación al correo en caché de una carpeta."
        }
        CommandName::ListMailboxes => {
            "Listar las carpetas en caché, con su función y sus contadores."
        }
        CommandName::SetSyncWindow => {
            "Elegir cuántos de los mensajes más recientes de una carpeta se guardan en la caché local."
        }
        CommandName::ListThreads => {
            "Listar las conversaciones de una carpeta, las más recientes primero."
        }
        CommandName::OpenMailbox => "Mostrar una carpeta en la lista de conversaciones.",
        CommandName::OpenUnifiedInbox => {
            "Mostrar las bandejas de entrada de todas las cuentas (bandeja unificada)."
        }
        CommandName::SelectThreads => "Sustituir la selección de conversaciones.",
        CommandName::CheckThreads => "Marcar conversaciones para actuar sobre varias a la vez.",
        CommandName::UncheckThreads => "Desmarcar conversaciones.",
        CommandName::OpenThread => "Abrir una conversación en el lector.",
        CommandName::CloseThread => "Cerrar la conversación que muestra el lector.",
        CommandName::ExpandMessages => "Expandir mensajes de la conversación abierta.",
        CommandName::CollapseMessages => {
            "Contraer mensajes de la conversación abierta en una línea."
        }
        CommandName::GetThread => {
            "Mostrar una conversación y todos sus mensajes, los más antiguos primero."
        }
        CommandName::GetMessage => "Mostrar un mensaje.",
        CommandName::ShowImagesFrom => {
            "Mostrar las imágenes incluidas en los mensajes de este remitente."
        }
        CommandName::HideImagesFrom => {
            "Dejar de mostrar las imágenes incluidas en los mensajes de este remitente."
        }
        CommandName::PinMailbox => "Fijar una carpeta en *Favoritos*, arriba en la barra lateral.",
        CommandName::UnpinMailbox => "Quitar una carpeta de los *Favoritos* de la barra lateral.",
        CommandName::HideMailbox => "Ocultar una carpeta en el árbol de la barra lateral.",
        CommandName::ShowMailbox => "Volver a mostrar una carpeta en el árbol de la barra lateral.",
        CommandName::SaveAttachments => {
            "Guardar los adjuntos de un mensaje descargado en una carpeta de este ordenador."
        }
        CommandName::MarkRead => "Marcar conversaciones o mensajes como leídos.",
        CommandName::MarkMailboxRead => {
            "Marcar como leídos todos los mensajes de una carpeta, también los que no están en caché."
        }
        CommandName::MarkUnread => "Marcar conversaciones o mensajes como no leídos.",
        CommandName::Star => "Destacar conversaciones o mensajes.",
        CommandName::Unstar => "Quitar el destacado de conversaciones o mensajes.",
        CommandName::PinThreads => "Fijar conversaciones arriba en las listas que las muestran.",
        CommandName::UnpinThreads => "Desfijar conversaciones o mensajes.",
        CommandName::ArchiveThreads => "Archivar conversaciones o mensajes.",
        CommandName::TrashThreads => "Mover conversaciones o mensajes a la papelera.",
        CommandName::MarkJunk => "Mover conversaciones o mensajes a la carpeta de spam.",
        CommandName::MarkNotJunk => "Devolver el spam a la bandeja de entrada.",
        CommandName::MoveThreads => {
            "Mover conversaciones o mensajes a otra carpeta de la misma cuenta."
        }
        CommandName::ListPendingOps => {
            "Listar los cambios locales que aún no han llegado al servidor."
        }
        CommandName::RetryPendingOps => "Reintentar ahora las operaciones pendientes.",
        CommandName::DiscardPendingOp => {
            "Descartar una operación pendiente: su cambio se deshace localmente."
        }
        CommandName::CreateDraft => "Empezar un mensaje nuevo desde una cuenta.",
        CommandName::Reply => "Empezar una respuesta al remitente de un mensaje.",
        CommandName::ReplyAll => "Responder al remitente y a todos los demás destinatarios.",
        CommandName::Forward => "Empezar a reenviar un mensaje.",
        CommandName::UpdateDraft => "Cambiar los campos de un borrador.",
        CommandName::DiscardDraft => "Eliminar un borrador.",
        CommandName::EditDraft => {
            "Editar un borrador que aparece en una conversación, aunque se escribiera en otro dispositivo."
        }
        CommandName::RestoreDraft => "Restaurar un borrador eliminado tal como estaba.",
        CommandName::AttachFile => "Adjuntar un archivo local a un borrador.",
        CommandName::RemoveAttachment => {
            "Quitar un adjunto de un borrador (el archivo no se toca)."
        }
        CommandName::SendDraft => "Enviar un borrador.",
        CommandName::CancelSend => {
            "Detener un envío programado antes de que salga; el borrador vuelve a ser editable."
        }
        CommandName::Search => "Buscar en el correo en caché.",
        CommandName::ListContacts => "Listar las personas que aparecen en el correo en caché.",
        CommandName::SetSearch => "Mostrar resultados de búsqueda en lugar de la carpeta abierta.",
        CommandName::ClearSearch => {
            "Salir de la búsqueda: la lista vuelve a mostrar la carpeta abierta."
        }
        CommandName::OpenComposer => {
            "Mostrar un borrador en la ventana de redacción y llevar el foco a ella."
        }
        CommandName::CloseComposer => "Cerrar la ventana de redacción.",
        CommandName::FocusPane => "Llevar el foco del teclado a un panel.",
        CommandName::SetListFilter => "Filtrar la lista: no leídos, destacados, con adjuntos.",
        CommandName::FocusMailbox => {
            "Centrarse en una carpeta o una cuenta: la barra lateral solo muestra eso."
        }
        CommandName::ClearFocus => {
            "Salir del modo enfoque: la barra lateral vuelve a mostrarlo todo."
        }
        CommandName::CollapseFolders => "Contraer carpetas en el árbol de la barra lateral.",
        CommandName::ExpandFolders => "Expandir carpetas en el árbol de la barra lateral.",
        CommandName::ToggleSidebar => "Contraer o expandir la barra lateral.",
        CommandName::GetView => "Mostrar lo que el usuario está mirando en la interfaz gráfica.",
        CommandName::CompactStore => {
            "Compactar el almacenamiento local y eliminar los cuerpos de mensaje huérfanos."
        }
        CommandName::CheckForUpdates => "Buscar ahora una versión nueva de dail.",
        CommandName::DownloadUpdate => {
            "Descargar la actualización disponible y comprobar su tamaño y su firma."
        }
        CommandName::InstallUpdate => "Instalar la actualización descargada.",
        CommandName::SkipUpdateVersion => "Omitir una versión: no se volverá a ofrecer.",
        CommandName::SetUpdatePolicy => {
            "Cambiar la búsqueda periódica de actualizaciones y su intervalo."
        }
        CommandName::ListApprovals => {
            "Listar las solicitudes de envío de agentes que esperan tu decisión."
        }
        CommandName::ApproveRequest => "Aprobar la solicitud de envío de un agente.",
        CommandName::RejectRequest => "Rechazar la solicitud de envío de un agente.",
    }
}

/// The Romanian one-sentence help of `name`.
pub(super) const fn romanian_help(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Afișați setările curente.",
        CommandName::SetSetting => "Schimbați o setare.",
        CommandName::SetKeybinding => {
            "Atribuiți taste unei comenzi, în locul celor din presetarea de comenzi rapide."
        }
        CommandName::SetKeyPreset => {
            "Alegeți presetarea de comenzi rapide de pornire: `gmail` (implicit) sau `vim`."
        }
        CommandName::SetTheme => {
            "Alegeți aspectul: modul (sistem, luminos, întunecat) și tema fiecărui mod."
        }
        CommandName::OpenSettings => "Afișați pagina de setări, la o anumită secțiune.",
        CommandName::CloseSettings => "Închideți pagina de setări.",
        CommandName::Undo => "Anulați ultima comandă care poate fi anulată.",
        CommandName::Redo => "Refaceți ultima comandă anulată.",
        CommandName::OpenPalette => "Deschideți paleta de comenzi.",
        CommandName::ClosePalette => "Închideți paleta de comenzi.",
        CommandName::AddAccount => "Adăugați un cont de e-mail IMAP/SMTP.",
        CommandName::TestAccount => {
            "Verificați un cont: conectare la serverele IMAP și SMTP cu parola salvată."
        }
        CommandName::DiscoverServers => {
            "Căutați serverele IMAP și SMTP ale domeniului unei adrese."
        }
        CommandName::UpdateAccount => "Schimbați numele afișat sau serverele unui cont.",
        CommandName::SetPassword => "Înlocuiți parola unui cont.",
        CommandName::SetIdentity => {
            "Adăugați o adresă de trimitere (un alias) sau schimbați-i numele ori semnătura."
        }
        CommandName::RemoveIdentity => {
            "Eliminați un alias sau numele și semnătura adresei contului."
        }
        CommandName::TrustCertificate => {
            "Acceptați, pentru un cont, certificatul autosemnat sau autoritatea privată prezentată de unul dintre serverele sale."
        }
        CommandName::ForgetCertificate => "Nu mai acceptați un certificat acceptat pentru un cont.",
        CommandName::SetMailboxIdentity => {
            "Alegeți adresa de trimitere a ciornelor noi scrise dintr-un dosar."
        }
        CommandName::CreateFolder => "Creați un dosar pe server.",
        CommandName::RenameFolder => {
            "Redenumiți sau mutați un dosar de pe server, împreună cu subdosarele sale."
        }
        CommandName::DeleteFolder => "Ștergeți un dosar de pe server, cu toate mesajele sale.",
        CommandName::EmptyTrash => {
            "Ștergeți definitiv toate mesajele din coșul de gunoi al unui cont, inclusiv de pe server."
        }
        CommandName::EmptyJunk => {
            "Ștergeți definitiv toate mesajele spam ale unui cont, inclusiv de pe server."
        }
        CommandName::SetDraftFrom => "Alegeți adresa de trimitere a unei ciorne.",
        CommandName::RemoveAccount => {
            "Eliminați un cont și ștergeți-i parola din portofelul de chei."
        }
        CommandName::SyncNow => {
            "Sincronizați acum: listați dosarele și descărcați e-mailurile noi."
        }
        CommandName::PullSettings => {
            "Citiți de pe serverul de e-mail setările comune dispozitivelor și combinați-le."
        }
        CommandName::PushSettings => {
            "Salvați setările comune pe serverul de e-mail pentru celelalte dispozitive."
        }
        CommandName::SetSettingsAccount => {
            "Alegeți contul al cărui server păstrează setările comune dispozitivelor."
        }
        CommandName::ListRules => "Listați regulile care sortează e-mailurile noi.",
        CommandName::SaveRule => {
            "Creați sau înlocuiți o regulă care sortează e-mailurile noi; nu poate redirecționa și nu poate răspunde."
        }
        CommandName::DeleteRule => "Ștergeți o regulă de sortare, pe toate dispozitivele.",
        CommandName::EnableRule => "Activați sau dezactivați o regulă de sortare fără a o șterge.",
        CommandName::ApplyRules => {
            "Aplicați acum regulile de sortare pe e-mailurile din cache ale unui dosar."
        }
        CommandName::ListMailboxes => "Listați dosarele din cache, cu rolul și contoarele lor.",
        CommandName::SetSyncWindow => {
            "Alegeți câte dintre cele mai noi mesaje ale unui dosar sunt păstrate în cache-ul local."
        }
        CommandName::ListThreads => "Listați conversațiile unui dosar, cele mai noi primele.",
        CommandName::OpenMailbox => "Afișați un dosar în lista de conversații.",
        CommandName::OpenUnifiedInbox => {
            "Afișați căsuțele de intrare ale tuturor conturilor (căsuța unificată)."
        }
        CommandName::SelectThreads => "Înlocuiți selecția de conversații.",
        CommandName::CheckThreads => {
            "Bifați conversații pentru a acționa asupra mai multora deodată."
        }
        CommandName::UncheckThreads => "Debifați conversații.",
        CommandName::OpenThread => "Deschideți o conversație în cititor.",
        CommandName::CloseThread => "Închideți conversația afișată în cititor.",
        CommandName::ExpandMessages => "Extindeți mesaje din conversația deschisă.",
        CommandName::CollapseMessages => {
            "Restrângeți mesaje din conversația deschisă pe un singur rând."
        }
        CommandName::GetThread => {
            "Afișați o conversație și toate mesajele ei, cele mai vechi primele."
        }
        CommandName::GetMessage => "Afișați un mesaj.",
        CommandName::ShowImagesFrom => "Afișați imaginile incluse în mesajele acestui expeditor.",
        CommandName::HideImagesFrom => {
            "Nu mai afișați imaginile incluse în mesajele acestui expeditor."
        }
        CommandName::PinMailbox => {
            "Fixați un dosar în *Favorite*, în partea de sus a barei laterale."
        }
        CommandName::UnpinMailbox => "Eliminați un dosar din *Favorite* din bara laterală.",
        CommandName::HideMailbox => "Ascundeți un dosar în arborele din bara laterală.",
        CommandName::ShowMailbox => "Afișați din nou un dosar în arborele din bara laterală.",
        CommandName::SaveAttachments => {
            "Salvați atașamentele unui mesaj descărcat într-un dosar de pe acest computer."
        }
        CommandName::MarkRead => "Marcați conversații sau mesaje ca citite.",
        CommandName::MarkMailboxRead => {
            "Marcați ca citite toate mesajele unui dosar, inclusiv cele din afara cache-ului."
        }
        CommandName::MarkUnread => "Marcați conversații sau mesaje ca necitite.",
        CommandName::Star => "Marcați cu stea conversații sau mesaje.",
        CommandName::Unstar => "Eliminați steaua de pe conversații sau mesaje.",
        CommandName::PinThreads => {
            "Fixați conversații în partea de sus a listelor care le afișează."
        }
        CommandName::UnpinThreads => "Anulați fixarea conversațiilor sau mesajelor.",
        CommandName::ArchiveThreads => "Arhivați conversații sau mesaje.",
        CommandName::TrashThreads => "Mutați conversații sau mesaje în coșul de gunoi.",
        CommandName::MarkJunk => "Mutați conversații sau mesaje în dosarul spam.",
        CommandName::MarkNotJunk => "Mutați înapoi în căsuța de intrare mesajele spam.",
        CommandName::MoveThreads => "Mutați conversații sau mesaje în alt dosar al aceluiași cont.",
        CommandName::ListPendingOps => {
            "Listați modificările locale care nu au ajuns încă pe server."
        }
        CommandName::RetryPendingOps => "Reîncercați acum operațiunile în așteptare.",
        CommandName::DiscardPendingOp => {
            "Renunțați la o operațiune în așteptare: modificarea ei este anulată local."
        }
        CommandName::CreateDraft => "Începeți un mesaj nou dintr-un cont.",
        CommandName::Reply => "Începeți un răspuns către expeditorul unui mesaj.",
        CommandName::ReplyAll => "Răspundeți expeditorului și tuturor celorlalți destinatari.",
        CommandName::Forward => "Începeți redirecționarea unui mesaj.",
        CommandName::UpdateDraft => "Schimbați câmpurile unei ciorne.",
        CommandName::DiscardDraft => "Ștergeți o ciornă.",
        CommandName::EditDraft => {
            "Editați o ciornă afișată într-o conversație, chiar dacă a fost scrisă pe alt dispozitiv."
        }
        CommandName::RestoreDraft => "Restabiliți o ciornă ștearsă așa cum era.",
        CommandName::AttachFile => "Atașați un fișier local la o ciornă.",
        CommandName::RemoveAttachment => {
            "Eliminați un atașament dintr-o ciornă (fișierul nu este atins)."
        }
        CommandName::SendDraft => "Trimiteți o ciornă.",
        CommandName::CancelSend => {
            "Opriți o trimitere programată înainte de a pleca; ciorna devine din nou editabilă."
        }
        CommandName::Search => "Căutați în e-mailurile din cache.",
        CommandName::ListContacts => "Listați persoanele care apar în e-mailurile din cache.",
        CommandName::SetSearch => "Afișați rezultatele căutării în locul dosarului deschis.",
        CommandName::ClearSearch => "Ieșiți din căutare: lista afișează din nou dosarul deschis.",
        CommandName::OpenComposer => {
            "Afișați o ciornă în fereastra de scriere și mutați focalizarea acolo."
        }
        CommandName::CloseComposer => "Închideți fereastra de scriere.",
        CommandName::FocusPane => "Mutați focalizarea tastaturii pe un panou.",
        CommandName::SetListFilter => "Filtrați lista: necitite, cu stea, cu atașamente.",
        CommandName::FocusMailbox => {
            "Concentrați-vă pe un dosar sau un cont: bara laterală afișează doar atât."
        }
        CommandName::ClearFocus => {
            "Ieșiți din modul concentrare: bara laterală afișează din nou totul."
        }
        CommandName::CollapseFolders => "Restrângeți dosare în arborele din bara laterală.",
        CommandName::ExpandFolders => "Extindeți dosare în arborele din bara laterală.",
        CommandName::ToggleSidebar => "Restrângeți sau extindeți bara laterală.",
        CommandName::GetView => "Afișați ce privește utilizatorul în interfața grafică.",
        CommandName::CompactStore => {
            "Compactați stocarea locală și ștergeți corpurile de mesaj orfane."
        }
        CommandName::CheckForUpdates => "Căutați acum o versiune nouă de dail.",
        CommandName::DownloadUpdate => {
            "Descărcați actualizarea disponibilă și verificați-i dimensiunea și semnătura."
        }
        CommandName::InstallUpdate => "Instalați actualizarea descărcată.",
        CommandName::SkipUpdateVersion => "Omiteți o versiune: nu va mai fi propusă.",
        CommandName::SetUpdatePolicy => {
            "Schimbați căutarea periodică a actualizărilor și intervalul ei."
        }
        CommandName::ListApprovals => {
            "Listați cererile de trimitere ale agenților care așteaptă decizia dvs."
        }
        CommandName::ApproveRequest => "Aprobați cererea de trimitere a unui agent.",
        CommandName::RejectRequest => "Respingeți cererea de trimitere a unui agent.",
    }
}

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
        CommandName::ListDevices => "Afficher les appareils associés",
        CommandName::StartPairing => "Associer un appareil",
        CommandName::RevokeDevice => "Révoquer un appareil",
        CommandName::ConnectServer => "Se connecter à un serveur dail",
        CommandName::TrustServer => "Faire confiance au serveur dail et s’y associer",
        CommandName::DisconnectServer => "Se déconnecter du serveur dail",
        CommandName::RetryOffline => "Réessayer les modifications hors ligne refusées",
        CommandName::DismissOfflineRejection => "Abandonner une modification hors ligne refusée",
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
        CommandName::ListDevices => {
            "Lister les appareils associés à ce serveur dail, avec leur rôle et leur dernière connexion."
        }
        CommandName::StartPairing => {
            "Créer un code à usage unique, valable 10 minutes, pour associer un appareil à ce serveur dail."
        }
        CommandName::RevokeDevice => {
            "Révoquer un appareil associé : il perd son accès au serveur dail."
        }
        CommandName::ConnectServer => {
            "Relier ce profil à un serveur dail et afficher l’empreinte de son certificat."
        }
        CommandName::TrustServer => {
            "Faire confiance au certificat du serveur dail et s’y associer avec le code d’association."
        }
        CommandName::DisconnectServer => {
            "Délier ce profil de son serveur dail : le courrier copié et les modifications hors ligne en attente sont perdus."
        }
        CommandName::RetryOffline => {
            "Remettre en file les modifications hors ligne que le serveur dail a refusées."
        }
        CommandName::DismissOfflineRejection => {
            "Abandonner une modification hors ligne refusée par le serveur dail, sans la renvoyer."
        }
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
        CommandName::ListDevices => "Mostrar los dispositivos vinculados",
        CommandName::StartPairing => "Vincular un dispositivo",
        CommandName::RevokeDevice => "Revocar un dispositivo",
        CommandName::ConnectServer => "Conectarse a un servidor dail",
        CommandName::TrustServer => "Confiar en el servidor dail y vincularse",
        CommandName::DisconnectServer => "Desconectarse del servidor dail",
        CommandName::RetryOffline => "Reintentar los cambios sin conexión rechazados",
        CommandName::DismissOfflineRejection => "Descartar un cambio sin conexión rechazado",
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
        CommandName::ListDevices => "Afișați dispozitivele asociate",
        CommandName::StartPairing => "Asociați un dispozitiv",
        CommandName::RevokeDevice => "Revocați un dispozitiv",
        CommandName::ConnectServer => "Conectați-vă la un server dail",
        CommandName::TrustServer => "Acceptați serverul dail și asociați dispozitivul",
        CommandName::DisconnectServer => "Deconectați-vă de la serverul dail",
        CommandName::RetryOffline => "Reîncercați modificările offline refuzate",
        CommandName::DismissOfflineRejection => "Renunțați la o modificare offline refuzată",
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
        CommandName::ListDevices => {
            "Listar los dispositivos vinculados a este servidor dail, con su rol y su última conexión."
        }
        CommandName::StartPairing => {
            "Crear un código de un solo uso, válido 10 minutos, para vincular un dispositivo a este servidor dail."
        }
        CommandName::RevokeDevice => {
            "Revocar un dispositivo vinculado: pierde el acceso al servidor dail."
        }
        CommandName::ConnectServer => {
            "Enlazar este perfil con un servidor dail y mostrar la huella de su certificado."
        }
        CommandName::TrustServer => {
            "Confiar en el certificado del servidor dail y vincularte con el código de vinculación."
        }
        CommandName::DisconnectServer => {
            "Desenlazar este perfil de su servidor dail: se pierden el correo copiado y los cambios sin conexión pendientes."
        }
        CommandName::RetryOffline => {
            "Volver a poner en cola los cambios sin conexión que el servidor dail rechazó."
        }
        CommandName::DismissOfflineRejection => {
            "Descartar un cambio sin conexión rechazado por el servidor dail, sin reenviarlo."
        }
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
        CommandName::ListDevices => {
            "Listați dispozitivele asociate acestui server dail, cu rolul și ultima lor conectare."
        }
        CommandName::StartPairing => {
            "Creați un cod de unică folosință, valabil 10 minute, pentru a asocia un dispozitiv cu acest server dail."
        }
        CommandName::RevokeDevice => {
            "Revocați un dispozitiv asociat: își pierde accesul la serverul dail."
        }
        CommandName::ConnectServer => {
            "Legați acest profil de un server dail și afișați amprenta certificatului său."
        }
        CommandName::TrustServer => {
            "Acceptați certificatul serverului dail și asociați dispozitivul cu codul de asociere."
        }
        CommandName::DisconnectServer => {
            "Dezlegați acest profil de serverul dail: e-mailurile copiate și modificările offline în așteptare se pierd."
        }
        CommandName::RetryOffline => {
            "Puneți din nou în coadă modificările offline refuzate de serverul dail."
        }
        CommandName::DismissOfflineRejection => {
            "Renunțați la o modificare offline refuzată de serverul dail, fără a o retrimite."
        }
    }
}

/// The Brazilian Portuguese title of `name`.
pub(super) const fn portuguese_brazil(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Mostrar as configurações",
        CommandName::SetSetting => "Alterar uma configuração",
        CommandName::SetKeybinding => "Alterar as teclas de um comando",
        CommandName::SetKeyPreset => "Escolher os atalhos (Gmail, Vim)",
        CommandName::SetTheme => "Escolher o tema",
        CommandName::OpenSettings => "Configurações",
        CommandName::CloseSettings => "Fechar as configurações",
        CommandName::Undo => "Desfazer",
        CommandName::Redo => "Refazer",
        CommandName::OpenPalette => "Abrir a paleta de comandos",
        CommandName::ClosePalette => "Fechar a paleta de comandos",
        CommandName::AddAccount => "Adicionar uma conta",
        CommandName::TestAccount => "Testar a conexão da conta",
        CommandName::DiscoverServers => "Encontrar os servidores de um endereço",
        CommandName::UpdateAccount => "Editar a conta",
        CommandName::SetPassword => "Alterar a senha da conta",
        CommandName::SetIdentity => "Adicionar ou editar um endereço de envio",
        CommandName::RemoveIdentity => "Remover um endereço de envio",
        CommandName::TrustCertificate => "Confiar no certificado de um servidor",
        CommandName::ForgetCertificate => "Esquecer um certificado confiável",
        CommandName::SetMailboxIdentity => "Enviar de um endereço para esta pasta",
        CommandName::CreateFolder => "Nova pasta",
        CommandName::RenameFolder => "Renomear a pasta",
        CommandName::DeleteFolder => "Excluir a pasta",
        CommandName::EmptyTrash => "Esvaziar a lixeira",
        CommandName::EmptyJunk => "Esvaziar o spam",
        CommandName::SetDraftFrom => "Enviar este rascunho de…",
        CommandName::RemoveAccount => "Remover a conta",
        CommandName::SyncNow => "Sincronizar agora",
        CommandName::PullSettings => "Ler as configurações do servidor",
        CommandName::PushSettings => "Salvar as configurações no servidor",
        CommandName::SetSettingsAccount => "Escolher a conta das configurações",
        CommandName::ListRules => "Listar as regras de organização",
        CommandName::SaveRule => "Criar uma regra de organização",
        CommandName::DeleteRule => "Excluir uma regra de organização",
        CommandName::EnableRule => "Ativar ou desativar uma regra de organização",
        CommandName::ApplyRules => "Aplicar as regras de organização a uma pasta",
        CommandName::ListMailboxes => "Listar as pastas",
        CommandName::SetSyncWindow => "Escolher os e-mails mantidos offline",
        CommandName::ListThreads => "Listar as conversas",
        CommandName::OpenMailbox => "Ir para a pasta",
        CommandName::OpenUnifiedInbox => "Ir para todas as caixas de entrada",
        CommandName::SelectThreads => "Selecionar conversas",
        CommandName::CheckThreads => "Selecionar todas as conversas",
        CommandName::UncheckThreads => "Limpar as conversas marcadas",
        CommandName::OpenThread => "Abrir a conversa",
        CommandName::CloseThread => "Fechar a conversa",
        CommandName::ExpandMessages => "Expandir todas as mensagens",
        CommandName::CollapseMessages => "Recolher todas as mensagens",
        CommandName::GetThread => "Mostrar a conversa",
        CommandName::GetMessage => "Mostrar a mensagem",
        CommandName::ShowImagesFrom => "Mostrar as imagens do remetente",
        CommandName::HideImagesFrom => "Ocultar as imagens do remetente",
        CommandName::PinMailbox => "Fixar nos Favoritos",
        CommandName::UnpinMailbox => "Remover a pasta dos Favoritos",
        CommandName::HideMailbox => "Ocultar a pasta",
        CommandName::ShowMailbox => "Mostrar a pasta oculta",
        CommandName::SaveAttachments => "Salvar os anexos",
        CommandName::MarkRead => "Marcar como lida",
        CommandName::MarkMailboxRead => "Marcar tudo como lido",
        CommandName::MarkUnread => "Marcar como não lida",
        CommandName::Star => "Adicionar estrela",
        CommandName::Unstar => "Remover estrela",
        CommandName::PinThreads => "Fixar no topo",
        CommandName::UnpinThreads => "Desafixar",
        CommandName::ArchiveThreads => "Arquivar",
        CommandName::TrashThreads => "Mover para a lixeira",
        CommandName::MarkJunk => "Marcar como spam",
        CommandName::MarkNotJunk => "Não é spam",
        CommandName::MoveThreads => "Mover para…",
        CommandName::ListPendingOps => "Mostrar as alterações pendentes",
        CommandName::RetryPendingOps => "Tentar de novo as alterações pendentes",
        CommandName::DiscardPendingOp => "Descartar a alteração pendente",
        CommandName::CreateDraft => "Escrever",
        CommandName::Reply => "Responder",
        CommandName::ReplyAll => "Responder a todos",
        CommandName::Forward => "Encaminhar",
        CommandName::UpdateDraft => "Editar o rascunho",
        CommandName::DiscardDraft => "Descartar o rascunho",
        CommandName::EditDraft => "Editar um rascunho da conversa",
        CommandName::RestoreDraft => "Restaurar o rascunho",
        CommandName::AttachFile => "Anexar um arquivo",
        CommandName::RemoveAttachment => "Remover o anexo",
        CommandName::SendDraft => "Enviar",
        CommandName::CancelSend => "Cancelar o envio",
        CommandName::Search => "Pesquisar no e-mail",
        CommandName::ListContacts => "Listar os contatos",
        CommandName::SetSearch => "Pesquisar",
        CommandName::ClearSearch => "Limpar a pesquisa",
        CommandName::OpenComposer => "Abrir o rascunho",
        CommandName::CloseComposer => "Fechar a escrita",
        CommandName::FocusPane => "Focar um painel",
        CommandName::SetListFilter => "Filtrar a lista",
        CommandName::FocusMailbox => "Focar nesta pasta",
        CommandName::ClearFocus => "Sair do foco",
        CommandName::CollapseFolders => "Recolher as pastas",
        CommandName::ExpandFolders => "Expandir as pastas",
        CommandName::ToggleSidebar => "Mostrar ou ocultar a barra lateral",
        CommandName::GetView => "Mostrar o estado da visualização",
        CommandName::CompactStore => "Compactar o armazenamento local",
        CommandName::CheckForUpdates => "Procurar atualizações",
        CommandName::DownloadUpdate => "Baixar a atualização",
        CommandName::InstallUpdate => "Instalar a atualização",
        CommandName::SkipUpdateVersion => "Pular esta versão",
        CommandName::SetUpdatePolicy => "Verificação de atualizações",
        CommandName::ListApprovals => "Mostrar as solicitações de envio",
        CommandName::ApproveRequest => "Aprovar a solicitação de envio",
        CommandName::RejectRequest => "Rejeitar a solicitação de envio",
        CommandName::ListDevices => "Mostrar os dispositivos pareados",
        CommandName::StartPairing => "Parear um dispositivo",
        CommandName::RevokeDevice => "Revogar um dispositivo",
        CommandName::ConnectServer => "Conectar a um servidor dail",
        CommandName::TrustServer => "Confiar no servidor dail e parear",
        CommandName::DisconnectServer => "Desconectar do servidor dail",
        CommandName::RetryOffline => "Tentar de novo as alterações offline recusadas",
        CommandName::DismissOfflineRejection => "Descartar uma alteração offline recusada",
    }
}

/// The European Portuguese title of `name`.
pub(super) const fn portuguese_portugal(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Mostrar as definições",
        CommandName::SetSetting => "Alterar uma definição",
        CommandName::SetKeybinding => "Alterar as teclas de um comando",
        CommandName::SetKeyPreset => "Escolher os atalhos (Gmail, Vim)",
        CommandName::SetTheme => "Escolher o tema",
        CommandName::OpenSettings => "Definições",
        CommandName::CloseSettings => "Fechar as definições",
        CommandName::Undo => "Anular",
        CommandName::Redo => "Refazer",
        CommandName::OpenPalette => "Abrir a paleta de comandos",
        CommandName::ClosePalette => "Fechar a paleta de comandos",
        CommandName::AddAccount => "Adicionar uma conta",
        CommandName::TestAccount => "Testar a ligação da conta",
        CommandName::DiscoverServers => "Encontrar os servidores de um endereço",
        CommandName::UpdateAccount => "Editar a conta",
        CommandName::SetPassword => "Alterar a palavra-passe da conta",
        CommandName::SetIdentity => "Adicionar ou editar um endereço de envio",
        CommandName::RemoveIdentity => "Remover um endereço de envio",
        CommandName::TrustCertificate => "Confiar no certificado de um servidor",
        CommandName::ForgetCertificate => "Esquecer um certificado aceite",
        CommandName::SetMailboxIdentity => "Enviar de um endereço para esta pasta",
        CommandName::CreateFolder => "Nova pasta",
        CommandName::RenameFolder => "Mudar o nome da pasta",
        CommandName::DeleteFolder => "Eliminar a pasta",
        CommandName::EmptyTrash => "Esvaziar o lixo",
        CommandName::EmptyJunk => "Esvaziar o spam",
        CommandName::SetDraftFrom => "Enviar este rascunho de…",
        CommandName::RemoveAccount => "Remover a conta",
        CommandName::SyncNow => "Sincronizar agora",
        CommandName::PullSettings => "Ler as definições do servidor",
        CommandName::PushSettings => "Guardar as definições no servidor",
        CommandName::SetSettingsAccount => "Escolher a conta das definições",
        CommandName::ListRules => "Listar as regras de organização",
        CommandName::SaveRule => "Criar uma regra de organização",
        CommandName::DeleteRule => "Eliminar uma regra de organização",
        CommandName::EnableRule => "Ativar ou desativar uma regra de organização",
        CommandName::ApplyRules => "Aplicar as regras de organização a uma pasta",
        CommandName::ListMailboxes => "Listar as pastas",
        CommandName::SetSyncWindow => "Escolher o correio mantido offline",
        CommandName::ListThreads => "Listar as conversas",
        CommandName::OpenMailbox => "Ir para a pasta",
        CommandName::OpenUnifiedInbox => "Ir para todas as caixas de entrada",
        CommandName::SelectThreads => "Selecionar conversas",
        CommandName::CheckThreads => "Selecionar todas as conversas",
        CommandName::UncheckThreads => "Limpar as conversas marcadas",
        CommandName::OpenThread => "Abrir a conversa",
        CommandName::CloseThread => "Fechar a conversa",
        CommandName::ExpandMessages => "Expandir todas as mensagens",
        CommandName::CollapseMessages => "Recolher todas as mensagens",
        CommandName::GetThread => "Mostrar a conversa",
        CommandName::GetMessage => "Mostrar a mensagem",
        CommandName::ShowImagesFrom => "Mostrar as imagens do remetente",
        CommandName::HideImagesFrom => "Ocultar as imagens do remetente",
        CommandName::PinMailbox => "Fixar nos Favoritos",
        CommandName::UnpinMailbox => "Remover a pasta dos Favoritos",
        CommandName::HideMailbox => "Ocultar a pasta",
        CommandName::ShowMailbox => "Mostrar a pasta oculta",
        CommandName::SaveAttachments => "Guardar os anexos",
        CommandName::MarkRead => "Marcar como lida",
        CommandName::MarkMailboxRead => "Marcar tudo como lido",
        CommandName::MarkUnread => "Marcar como não lida",
        CommandName::Star => "Adicionar estrela",
        CommandName::Unstar => "Remover estrela",
        CommandName::PinThreads => "Fixar no topo",
        CommandName::UnpinThreads => "Desafixar",
        CommandName::ArchiveThreads => "Arquivar",
        CommandName::TrashThreads => "Mover para o lixo",
        CommandName::MarkJunk => "Marcar como spam",
        CommandName::MarkNotJunk => "Não é spam",
        CommandName::MoveThreads => "Mover para…",
        CommandName::ListPendingOps => "Mostrar as alterações pendentes",
        CommandName::RetryPendingOps => "Tentar de novo as alterações pendentes",
        CommandName::DiscardPendingOp => "Rejeitar a alteração pendente",
        CommandName::CreateDraft => "Escrever",
        CommandName::Reply => "Responder",
        CommandName::ReplyAll => "Responder a todos",
        CommandName::Forward => "Reencaminhar",
        CommandName::UpdateDraft => "Editar o rascunho",
        CommandName::DiscardDraft => "Rejeitar o rascunho",
        CommandName::EditDraft => "Editar um rascunho da conversa",
        CommandName::RestoreDraft => "Restaurar o rascunho",
        CommandName::AttachFile => "Anexar um ficheiro",
        CommandName::RemoveAttachment => "Remover o anexo",
        CommandName::SendDraft => "Enviar",
        CommandName::CancelSend => "Cancelar o envio",
        CommandName::Search => "Pesquisar no correio",
        CommandName::ListContacts => "Listar os contactos",
        CommandName::SetSearch => "Pesquisar",
        CommandName::ClearSearch => "Limpar a pesquisa",
        CommandName::OpenComposer => "Abrir o rascunho",
        CommandName::CloseComposer => "Fechar a escrita",
        CommandName::FocusPane => "Focar um painel",
        CommandName::SetListFilter => "Filtrar a lista",
        CommandName::FocusMailbox => "Focar nesta pasta",
        CommandName::ClearFocus => "Sair do foco",
        CommandName::CollapseFolders => "Recolher as pastas",
        CommandName::ExpandFolders => "Expandir as pastas",
        CommandName::ToggleSidebar => "Mostrar ou ocultar a barra lateral",
        CommandName::GetView => "Mostrar o estado da vista",
        CommandName::CompactStore => "Compactar o armazenamento local",
        CommandName::CheckForUpdates => "Procurar atualizações",
        CommandName::DownloadUpdate => "Transferir a atualização",
        CommandName::InstallUpdate => "Instalar a atualização",
        CommandName::SkipUpdateVersion => "Ignorar esta versão",
        CommandName::SetUpdatePolicy => "Verificação de atualizações",
        CommandName::ListApprovals => "Mostrar os pedidos de envio",
        CommandName::ApproveRequest => "Aprovar o pedido de envio",
        CommandName::RejectRequest => "Rejeitar o pedido de envio",
        CommandName::ListDevices => "Mostrar os dispositivos emparelhados",
        CommandName::StartPairing => "Emparelhar um dispositivo",
        CommandName::RevokeDevice => "Revogar um dispositivo",
        CommandName::ConnectServer => "Ligar a um servidor dail",
        CommandName::TrustServer => "Confiar no servidor dail e emparelhar",
        CommandName::DisconnectServer => "Desligar do servidor dail",
        CommandName::RetryOffline => "Tentar de novo as alterações offline recusadas",
        CommandName::DismissOfflineRejection => "Rejeitar uma alteração offline recusada",
    }
}

/// The Latin title of `name`.
pub(super) const fn latin(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Optiones monstra",
        CommandName::SetSetting => "Optionem muta",
        CommandName::SetKeybinding => "Claves mandati muta",
        CommandName::SetKeyPreset => "Compendia elige (Gmail, Vim)",
        CommandName::SetTheme => "Thema elige",
        CommandName::OpenSettings => "Optiones",
        CommandName::CloseSettings => "Optiones claude",
        CommandName::Undo => "Revoca",
        CommandName::Redo => "Restitue",
        CommandName::OpenPalette => "Tabulam mandatorum aperi",
        CommandName::ClosePalette => "Tabulam mandatorum claude",
        CommandName::AddAccount => "Rationem adde",
        CommandName::TestAccount => "Conexionem rationis proba",
        CommandName::DiscoverServers => "Servitores inscriptionis inveni",
        CommandName::UpdateAccount => "Rationem recense",
        CommandName::SetPassword => "Tesseram rationis muta",
        CommandName::SetIdentity => "Inscriptionem missionis adde aut recense",
        CommandName::RemoveIdentity => "Inscriptionem missionis remove",
        CommandName::TrustCertificate => "Testimonio servitoris confide",
        CommandName::ForgetCertificate => "Testimonium acceptum obliviscere",
        CommandName::SetMailboxIdentity => "Ab inscriptione pro hac capsa mitte",
        CommandName::CreateFolder => "Nova capsa",
        CommandName::RenameFolder => "Capsam renomina",
        CommandName::DeleteFolder => "Capsam dele",
        CommandName::EmptyTrash => "Quisquilias exhauri",
        CommandName::EmptyJunk => "Purgamentum exhauri",
        CommandName::SetDraftFrom => "Hanc adumbrationem mitte ab…",
        CommandName::RemoveAccount => "Rationem remove",
        CommandName::SyncNow => "Nunc synchroniza",
        CommandName::PullSettings => "Optiones e servitore lege",
        CommandName::PushSettings => "Optiones in servitore serva",
        CommandName::SetSettingsAccount => "Rationem optionum elige",
        CommandName::ListRules => "Regulas digerendi enumera",
        CommandName::SaveRule => "Regulam digerendi crea",
        CommandName::DeleteRule => "Regulam digerendi dele",
        CommandName::EnableRule => "Regulam digerendi accende aut exstingue",
        CommandName::ApplyRules => "Regulas digerendi capsae adhibe",
        CommandName::ListMailboxes => "Capsas enumera",
        CommandName::SetSyncWindow => "Epistulas sine conexione servandas elige",
        CommandName::ListThreads => "Colloquia enumera",
        CommandName::OpenMailbox => "Ad capsam i",
        CommandName::OpenUnifiedInbox => "Ad omnes capsas acceptorum i",
        CommandName::SelectThreads => "Colloquia selige",
        CommandName::CheckThreads => "Omnia colloquia selige",
        CommandName::UncheckThreads => "Colloquia signata purga",
        CommandName::OpenThread => "Colloquium aperi",
        CommandName::CloseThread => "Colloquium claude",
        CommandName::ExpandMessages => "Omnes epistulas expande",
        CommandName::CollapseMessages => "Omnes epistulas contrahe",
        CommandName::GetThread => "Colloquium monstra",
        CommandName::GetMessage => "Epistulam monstra",
        CommandName::ShowImagesFrom => "Imagines missoris monstra",
        CommandName::HideImagesFrom => "Imagines missoris cela",
        CommandName::PinMailbox => "In dilectis fige",
        CommandName::UnpinMailbox => "Capsam e dilectis remove",
        CommandName::HideMailbox => "Capsam cela",
        CommandName::ShowMailbox => "Capsam occultam monstra",
        CommandName::SaveAttachments => "Adiuncta serva",
        CommandName::MarkRead => "Ut lectum nota",
        CommandName::MarkMailboxRead => "Omnia ut lecta nota",
        CommandName::MarkUnread => "Ut non lectum nota",
        CommandName::Star => "Stella nota",
        CommandName::Unstar => "Stellam remove",
        CommandName::PinThreads => "In summo fige",
        CommandName::UnpinThreads => "Refige",
        CommandName::ArchiveThreads => "In archivum conde",
        CommandName::TrashThreads => "In quisquilias move",
        CommandName::MarkJunk => "Ut purgamentum nota",
        CommandName::MarkNotJunk => "Non purgamentum",
        CommandName::MoveThreads => "Move in…",
        CommandName::ListPendingOps => "Mutationes exspectantes monstra",
        CommandName::RetryPendingOps => "Mutationes exspectantes iterum conare",
        CommandName::DiscardPendingOp => "Mutationem exspectantem abice",
        CommandName::CreateDraft => "Scribe",
        CommandName::Reply => "Responde",
        CommandName::ReplyAll => "Omnibus responde",
        CommandName::Forward => "Transmitte",
        CommandName::UpdateDraft => "Adumbrationem recense",
        CommandName::DiscardDraft => "Adumbrationem abice",
        CommandName::EditDraft => "Adumbrationem colloquii recense",
        CommandName::RestoreDraft => "Adumbrationem restitue",
        CommandName::AttachFile => "Documentum adiunge",
        CommandName::RemoveAttachment => "Adiunctum remove",
        CommandName::SendDraft => "Mitte",
        CommandName::CancelSend => "Missionem rescinde",
        CommandName::Search => "In cursu quaere",
        CommandName::ListContacts => "Necessarios enumera",
        CommandName::SetSearch => "Quaere",
        CommandName::ClearSearch => "Quaestionem purga",
        CommandName::OpenComposer => "Adumbrationem aperi",
        CommandName::CloseComposer => "Scriptionem claude",
        CommandName::FocusPane => "Tabulatum intende",
        CommandName::SetListFilter => "Indicem cola",
        CommandName::FocusMailbox => "In hac capsa intende",
        CommandName::ClearFocus => "Ab intentione exi",
        CommandName::CollapseFolders => "Capsas contrahe",
        CommandName::ExpandFolders => "Capsas expande",
        CommandName::ToggleSidebar => "Tabulam lateralem monstra aut cela",
        CommandName::GetView => "Statum conspectus monstra",
        CommandName::CompactStore => "Repositorium locale comprime",
        CommandName::CheckForUpdates => "Renovationes quaere",
        CommandName::DownloadUpdate => "Renovationem deprime",
        CommandName::InstallUpdate => "Renovationem institue",
        CommandName::SkipUpdateVersion => "Hanc versionem praetermitte",
        CommandName::SetUpdatePolicy => "Inspectio renovationum",
        CommandName::ListApprovals => "Petitiones missionis monstra",
        CommandName::ApproveRequest => "Petitionem missionis proba",
        CommandName::RejectRequest => "Petitionem missionis reice",
        CommandName::ListDevices => "Instrumenta copulata monstra",
        CommandName::StartPairing => "Instrumentum copula",
        CommandName::RevokeDevice => "Instrumentum revoca",
        CommandName::ConnectServer => "Cum servitore dail conecte",
        CommandName::TrustServer => "Servitori dail confide et copula",
        CommandName::DisconnectServer => "A servitore dail disiunge",
        CommandName::RetryOffline => "Mutationes sine conexione recusatas iterum conare",
        CommandName::DismissOfflineRejection => "Mutationem sine conexione recusatam abice",
    }
}

/// The Brazilian Portuguese one-sentence help of `name`.
pub(super) const fn portuguese_brazil_help(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Mostrar as configurações atuais.",
        CommandName::SetKeybinding => {
            "Associar teclas a um comando, no lugar das da predefinição de atalhos."
        }
        CommandName::SetKeyPreset => {
            "Escolher a predefinição de atalhos de partida: `gmail` (padrão) ou `vim`."
        }
        CommandName::SetTheme => {
            "Escolher a aparência: o modo (sistema, claro, escuro) e o tema de cada modo."
        }
        CommandName::SetSetting => "Alterar uma configuração.",
        CommandName::Undo => "Desfazer o último comando que pode ser desfeito.",
        CommandName::Redo => "Refazer o último comando desfeito.",
        CommandName::OpenPalette => "Abrir a paleta de comandos.",
        CommandName::ClosePalette => "Fechar a paleta de comandos.",
        CommandName::AddAccount => "Adicionar uma conta de e-mail IMAP/SMTP.",
        CommandName::DiscoverServers => {
            "Procurar os servidores IMAP e SMTP do domínio de um endereço."
        }
        CommandName::TestAccount => {
            "Verificar uma conta: conectar aos servidores IMAP e SMTP com a senha salva."
        }
        CommandName::UpdateAccount => "Alterar o nome de exibição ou os servidores de uma conta.",
        CommandName::SetPassword => "Substituir a senha de uma conta.",
        CommandName::SetIdentity => {
            "Adicionar um endereço de envio (um alias) ou alterar seu nome ou sua assinatura."
        }
        CommandName::RemoveIdentity => {
            "Remover um alias, ou o nome e a assinatura do endereço da conta."
        }
        CommandName::TrustCertificate => {
            "Confiar, para uma conta, no certificado autoassinado ou na autoridade privada que um dos seus servidores apresentou."
        }
        CommandName::ForgetCertificate => {
            "Deixar de confiar em um certificado aceito para uma conta."
        }
        CommandName::SetMailboxIdentity => {
            "Escolher o endereço de envio dos novos rascunhos escritos a partir de uma pasta."
        }
        CommandName::CreateFolder => "Criar uma pasta no servidor.",
        CommandName::RenameFolder => "Renomear ou mover uma pasta no servidor, com suas subpastas.",
        CommandName::DeleteFolder => "Excluir uma pasta do servidor, com todas as suas mensagens.",
        CommandName::EmptyTrash => {
            "Excluir para sempre todas as mensagens da lixeira de uma conta, também no servidor."
        }
        CommandName::EmptyJunk => {
            "Excluir para sempre todas as mensagens de spam de uma conta, também no servidor."
        }
        CommandName::SetDraftFrom => "Escolher o endereço de envio de um rascunho.",
        CommandName::RemoveAccount => "Remover uma conta e apagar sua senha do chaveiro.",
        CommandName::PullSettings => {
            "Ler do servidor de e-mail as configurações compartilhadas entre dispositivos e mesclá-las."
        }
        CommandName::PushSettings => {
            "Salvar as configurações compartilhadas no servidor de e-mail para os outros dispositivos."
        }
        CommandName::SetSettingsAccount => {
            "Escolher a conta cujo servidor guarda as configurações compartilhadas entre dispositivos."
        }
        CommandName::ListRules => "Listar as regras que organizam os novos e-mails.",
        CommandName::SaveRule => {
            "Criar ou substituir uma regra que organiza os novos e-mails; ela não pode encaminhar nem responder."
        }
        CommandName::DeleteRule => "Excluir uma regra de organização, em todos os dispositivos.",
        CommandName::EnableRule => "Ativar ou desativar uma regra de organização sem excluí-la.",
        CommandName::ApplyRules => {
            "Aplicar agora as regras de organização aos e-mails em cache de uma pasta."
        }
        CommandName::SyncNow => "Sincronizar agora: listar as pastas e baixar os novos e-mails.",
        CommandName::ListMailboxes => {
            "Listar as pastas em cache, com sua função e seus contadores."
        }
        CommandName::ListThreads => "Listar as conversas de uma pasta, as mais recentes primeiro.",
        CommandName::SetSyncWindow => {
            "Escolher quantas das mensagens mais recentes de uma pasta ficam no cache local."
        }
        CommandName::OpenUnifiedInbox => {
            "Mostrar as caixas de entrada de todas as contas (caixa unificada)."
        }
        CommandName::OpenMailbox => "Mostrar uma pasta na lista de conversas.",
        CommandName::SelectThreads => "Substituir a seleção de conversas.",
        CommandName::CheckThreads => "Marcar conversas para agir sobre várias de uma vez.",
        CommandName::UncheckThreads => "Desmarcar conversas.",
        CommandName::OpenThread => "Abrir uma conversa no leitor.",
        CommandName::CloseThread => "Fechar a conversa mostrada no leitor.",
        CommandName::ExpandMessages => "Expandir mensagens da conversa aberta.",
        CommandName::CollapseMessages => "Recolher mensagens da conversa aberta em uma linha.",
        CommandName::GetThread => {
            "Mostrar uma conversa e todas as suas mensagens, as mais antigas primeiro."
        }
        CommandName::GetMessage => "Mostrar uma mensagem.",
        CommandName::ShowImagesFrom => {
            "Mostrar as imagens incluídas nas mensagens deste remetente."
        }
        CommandName::HideImagesFrom => {
            "Deixar de mostrar as imagens incluídas nas mensagens deste remetente."
        }
        CommandName::PinMailbox => "Fixar uma pasta nos *Favoritos*, no topo da barra lateral.",
        CommandName::UnpinMailbox => "Remover uma pasta dos *Favoritos* da barra lateral.",
        CommandName::HideMailbox => "Ocultar uma pasta na árvore da barra lateral.",
        CommandName::ShowMailbox => "Mostrar de novo uma pasta na árvore da barra lateral.",
        CommandName::SaveAttachments => {
            "Salvar os anexos de uma mensagem baixada em uma pasta deste computador."
        }
        CommandName::MarkRead => "Marcar conversas ou mensagens como lidas.",
        CommandName::MarkMailboxRead => {
            "Marcar como lidas todas as mensagens de uma pasta, inclusive as fora do cache."
        }
        CommandName::MarkUnread => "Marcar conversas ou mensagens como não lidas.",
        CommandName::Star => "Adicionar estrela a conversas ou mensagens.",
        CommandName::Unstar => "Remover a estrela de conversas ou mensagens.",
        CommandName::PinThreads => "Fixar conversas no topo das listas que as mostram.",
        CommandName::UnpinThreads => "Desafixar conversas ou mensagens.",
        CommandName::ArchiveThreads => "Arquivar conversas ou mensagens.",
        CommandName::TrashThreads => "Mover conversas ou mensagens para a lixeira.",
        CommandName::MarkJunk => "Mover conversas ou mensagens para a pasta de spam.",
        CommandName::MarkNotJunk => "Devolver o spam à caixa de entrada.",
        CommandName::MoveThreads => "Mover conversas ou mensagens para outra pasta da mesma conta.",
        CommandName::ListPendingOps => {
            "Listar as alterações locais que ainda não chegaram ao servidor."
        }
        CommandName::RetryPendingOps => "Tentar de novo agora as operações pendentes.",
        CommandName::DiscardPendingOp => {
            "Descartar uma operação pendente: sua alteração é desfeita localmente."
        }
        CommandName::CreateDraft => "Começar uma nova mensagem a partir de uma conta.",
        CommandName::Reply => "Começar uma resposta ao remetente de uma mensagem.",
        CommandName::ReplyAll => "Responder ao remetente e a todos os outros destinatários.",
        CommandName::Forward => "Começar a encaminhar uma mensagem.",
        CommandName::UpdateDraft => "Alterar os campos de um rascunho.",
        CommandName::DiscardDraft => "Excluir um rascunho.",
        CommandName::EditDraft => {
            "Editar um rascunho mostrado em uma conversa, mesmo escrito em outro dispositivo."
        }
        CommandName::AttachFile => "Anexar um arquivo local a um rascunho.",
        CommandName::RemoveAttachment => {
            "Remover um anexo de um rascunho (o arquivo não é alterado)."
        }
        CommandName::SendDraft => "Enviar um rascunho.",
        CommandName::CancelSend => {
            "Parar um envio programado antes de sair; o rascunho volta a ser editável."
        }
        CommandName::RestoreDraft => "Restaurar um rascunho excluído como estava.",
        CommandName::ListContacts => "Listar as pessoas que aparecem nos e-mails em cache.",
        CommandName::Search => "Pesquisar nos e-mails em cache.",
        CommandName::SetSearch => "Mostrar resultados de pesquisa no lugar da pasta aberta.",
        CommandName::ClearSearch => "Sair da pesquisa: a lista volta a mostrar a pasta aberta.",
        CommandName::OpenComposer => {
            "Mostrar um rascunho na janela de escrita e levar o foco até ela."
        }
        CommandName::CloseComposer => "Fechar a janela de escrita.",
        CommandName::OpenSettings => "Mostrar a página de configurações, em uma seção específica.",
        CommandName::CloseSettings => "Fechar a página de configurações.",
        CommandName::FocusPane => "Levar o foco do teclado para um painel.",
        CommandName::SetListFilter => "Filtrar a lista: não lidas, com estrela, com anexos.",
        CommandName::FocusMailbox => "Focar em uma pasta ou conta: a barra lateral mostra só isso.",
        CommandName::ClearFocus => "Sair do modo foco: a barra lateral volta a mostrar tudo.",
        CommandName::CollapseFolders => "Recolher pastas na árvore da barra lateral.",
        CommandName::ExpandFolders => "Expandir pastas na árvore da barra lateral.",
        CommandName::ToggleSidebar => "Recolher ou expandir a barra lateral.",
        CommandName::CompactStore => {
            "Compactar o armazenamento local e remover corpos de mensagem órfãos."
        }
        CommandName::GetView => "Mostrar o que o usuário está vendo na interface gráfica.",
        CommandName::CheckForUpdates => "Procurar agora uma nova versão do dail.",
        CommandName::DownloadUpdate => {
            "Baixar a atualização disponível e verificar seu tamanho e sua assinatura."
        }
        CommandName::InstallUpdate => "Instalar a atualização baixada.",
        CommandName::SkipUpdateVersion => "Pular uma versão: ela não será mais oferecida.",
        CommandName::SetUpdatePolicy => {
            "Alterar a verificação periódica de atualizações e seu intervalo."
        }
        CommandName::ListApprovals => {
            "Listar as solicitações de envio de agentes que aguardam sua decisão."
        }
        CommandName::ApproveRequest => "Aprovar a solicitação de envio de um agente.",
        CommandName::RejectRequest => "Rejeitar a solicitação de envio de um agente.",
        CommandName::ListDevices => {
            "Listar os dispositivos pareados com este servidor dail, com a função e a última conexão de cada um."
        }
        CommandName::StartPairing => {
            "Criar um código de uso único, válido por 10 minutos, para parear um dispositivo com este servidor dail."
        }
        CommandName::RevokeDevice => {
            "Revogar um dispositivo pareado: ele perde o acesso ao servidor dail."
        }
        CommandName::ConnectServer => {
            "Vincular este perfil a um servidor dail e mostrar a impressão digital do certificado dele."
        }
        CommandName::TrustServer => {
            "Confiar no certificado do servidor dail e parear com o código de pareamento."
        }
        CommandName::DisconnectServer => {
            "Desvincular este perfil do servidor dail: os e-mails copiados e as alterações offline pendentes são perdidos."
        }
        CommandName::RetryOffline => {
            "Colocar de novo na fila as alterações offline que o servidor dail recusou."
        }
        CommandName::DismissOfflineRejection => {
            "Descartar uma alteração offline recusada pelo servidor dail, sem reenviá-la."
        }
    }
}

/// The European Portuguese one-sentence help of `name`.
pub(super) const fn portuguese_portugal_help(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Mostrar as definições atuais.",
        CommandName::SetKeybinding => {
            "Associar teclas a um comando, no lugar das da predefinição de atalhos."
        }
        CommandName::SetKeyPreset => {
            "Escolher a predefinição de atalhos de partida: `gmail` (predefinido) ou `vim`."
        }
        CommandName::SetTheme => {
            "Escolher a aparência: o modo (sistema, claro, escuro) e o tema de cada modo."
        }
        CommandName::SetSetting => "Alterar uma definição.",
        CommandName::Undo => "Anular o último comando que pode ser anulado.",
        CommandName::Redo => "Refazer o último comando anulado.",
        CommandName::OpenPalette => "Abrir a paleta de comandos.",
        CommandName::ClosePalette => "Fechar a paleta de comandos.",
        CommandName::AddAccount => "Adicionar uma conta de correio IMAP/SMTP.",
        CommandName::DiscoverServers => {
            "Procurar os servidores IMAP e SMTP do domínio de um endereço."
        }
        CommandName::TestAccount => {
            "Verificar uma conta: ligar aos servidores IMAP e SMTP com a palavra-passe guardada."
        }
        CommandName::UpdateAccount => "Alterar o nome a mostrar ou os servidores de uma conta.",
        CommandName::SetPassword => "Substituir a palavra-passe de uma conta.",
        CommandName::SetIdentity => {
            "Adicionar um endereço de envio (um alias) ou alterar o seu nome ou a sua assinatura."
        }
        CommandName::RemoveIdentity => {
            "Remover um alias, ou o nome e a assinatura do endereço da conta."
        }
        CommandName::TrustCertificate => {
            "Confiar, para uma conta, no certificado autoassinado ou na autoridade privada que um dos seus servidores apresentou."
        }
        CommandName::ForgetCertificate => {
            "Deixar de confiar num certificado aceite para uma conta."
        }
        CommandName::SetMailboxIdentity => {
            "Escolher o endereço de envio dos novos rascunhos escritos a partir de uma pasta."
        }
        CommandName::CreateFolder => "Criar uma pasta no servidor.",
        CommandName::RenameFolder => {
            "Mudar o nome ou mover uma pasta no servidor, com as suas subpastas."
        }
        CommandName::DeleteFolder => "Eliminar uma pasta do servidor, com todas as suas mensagens.",
        CommandName::EmptyTrash => {
            "Eliminar definitivamente todas as mensagens do lixo de uma conta, também no servidor."
        }
        CommandName::EmptyJunk => {
            "Eliminar definitivamente todas as mensagens de spam de uma conta, também no servidor."
        }
        CommandName::SetDraftFrom => "Escolher o endereço de envio de um rascunho.",
        CommandName::RemoveAccount => {
            "Remover uma conta e apagar a sua palavra-passe do porta-chaves."
        }
        CommandName::PullSettings => {
            "Ler do servidor de correio as definições partilhadas entre dispositivos e juntá-las."
        }
        CommandName::PushSettings => {
            "Guardar as definições partilhadas no servidor de correio para os outros dispositivos."
        }
        CommandName::SetSettingsAccount => {
            "Escolher a conta cujo servidor guarda as definições partilhadas entre dispositivos."
        }
        CommandName::ListRules => "Listar as regras que organizam o correio novo.",
        CommandName::SaveRule => {
            "Criar ou substituir uma regra que organiza o correio novo; não pode reencaminhar nem responder."
        }
        CommandName::DeleteRule => "Eliminar uma regra de organização, em todos os dispositivos.",
        CommandName::EnableRule => "Ativar ou desativar uma regra de organização sem a eliminar.",
        CommandName::ApplyRules => {
            "Aplicar agora as regras de organização ao correio em cache de uma pasta."
        }
        CommandName::SyncNow => "Sincronizar agora: listar as pastas e transferir o correio novo.",
        CommandName::ListMailboxes => {
            "Listar as pastas em cache, com a sua função e os seus contadores."
        }
        CommandName::ListThreads => "Listar as conversas de uma pasta, as mais recentes primeiro.",
        CommandName::SetSyncWindow => {
            "Escolher quantas das mensagens mais recentes de uma pasta ficam na cache local."
        }
        CommandName::OpenUnifiedInbox => {
            "Mostrar as caixas de entrada de todas as contas (caixa unificada)."
        }
        CommandName::OpenMailbox => "Mostrar uma pasta na lista de conversas.",
        CommandName::SelectThreads => "Substituir a seleção de conversas.",
        CommandName::CheckThreads => "Marcar conversas para agir sobre várias de uma vez.",
        CommandName::UncheckThreads => "Desmarcar conversas.",
        CommandName::OpenThread => "Abrir uma conversa no leitor.",
        CommandName::CloseThread => "Fechar a conversa mostrada no leitor.",
        CommandName::ExpandMessages => "Expandir mensagens da conversa aberta.",
        CommandName::CollapseMessages => "Recolher mensagens da conversa aberta numa linha.",
        CommandName::GetThread => {
            "Mostrar uma conversa e todas as suas mensagens, as mais antigas primeiro."
        }
        CommandName::GetMessage => "Mostrar uma mensagem.",
        CommandName::ShowImagesFrom => {
            "Mostrar as imagens incluídas nas mensagens deste remetente."
        }
        CommandName::HideImagesFrom => {
            "Deixar de mostrar as imagens incluídas nas mensagens deste remetente."
        }
        CommandName::PinMailbox => "Fixar uma pasta nos *Favoritos*, no topo da barra lateral.",
        CommandName::UnpinMailbox => "Remover uma pasta dos *Favoritos* da barra lateral.",
        CommandName::HideMailbox => "Ocultar uma pasta na árvore da barra lateral.",
        CommandName::ShowMailbox => "Mostrar de novo uma pasta na árvore da barra lateral.",
        CommandName::SaveAttachments => {
            "Guardar os anexos de uma mensagem transferida numa pasta deste computador."
        }
        CommandName::MarkRead => "Marcar conversas ou mensagens como lidas.",
        CommandName::MarkMailboxRead => {
            "Marcar como lidas todas as mensagens de uma pasta, incluindo as que estão fora da cache."
        }
        CommandName::MarkUnread => "Marcar conversas ou mensagens como não lidas.",
        CommandName::Star => "Adicionar estrela a conversas ou mensagens.",
        CommandName::Unstar => "Remover a estrela de conversas ou mensagens.",
        CommandName::PinThreads => "Fixar conversas no topo das listas que as mostram.",
        CommandName::UnpinThreads => "Desafixar conversas ou mensagens.",
        CommandName::ArchiveThreads => "Arquivar conversas ou mensagens.",
        CommandName::TrashThreads => "Mover conversas ou mensagens para o lixo.",
        CommandName::MarkJunk => "Mover conversas ou mensagens para a pasta de spam.",
        CommandName::MarkNotJunk => "Devolver o spam à caixa de entrada.",
        CommandName::MoveThreads => "Mover conversas ou mensagens para outra pasta da mesma conta.",
        CommandName::ListPendingOps => {
            "Listar as alterações locais que ainda não chegaram ao servidor."
        }
        CommandName::RetryPendingOps => "Tentar de novo agora as operações pendentes.",
        CommandName::DiscardPendingOp => {
            "Rejeitar uma operação pendente: a sua alteração é anulada localmente."
        }
        CommandName::CreateDraft => "Começar uma nova mensagem a partir de uma conta.",
        CommandName::Reply => "Começar uma resposta ao remetente de uma mensagem.",
        CommandName::ReplyAll => "Responder ao remetente e a todos os outros destinatários.",
        CommandName::Forward => "Começar a reencaminhar uma mensagem.",
        CommandName::UpdateDraft => "Alterar os campos de um rascunho.",
        CommandName::DiscardDraft => "Eliminar um rascunho.",
        CommandName::EditDraft => {
            "Editar um rascunho mostrado numa conversa, mesmo que escrito noutro dispositivo."
        }
        CommandName::AttachFile => "Anexar um ficheiro local a um rascunho.",
        CommandName::RemoveAttachment => {
            "Remover um anexo de um rascunho (o ficheiro não é alterado)."
        }
        CommandName::SendDraft => "Enviar um rascunho.",
        CommandName::CancelSend => {
            "Parar um envio agendado antes de sair; o rascunho volta a ser editável."
        }
        CommandName::RestoreDraft => "Restaurar um rascunho eliminado tal como estava.",
        CommandName::ListContacts => "Listar as pessoas que aparecem no correio em cache.",
        CommandName::Search => "Pesquisar no correio em cache.",
        CommandName::SetSearch => "Mostrar resultados de pesquisa no lugar da pasta aberta.",
        CommandName::ClearSearch => "Sair da pesquisa: a lista volta a mostrar a pasta aberta.",
        CommandName::OpenComposer => {
            "Mostrar um rascunho na janela de escrita e levar o foco até ela."
        }
        CommandName::CloseComposer => "Fechar a janela de escrita.",
        CommandName::OpenSettings => "Mostrar a página de definições, numa secção específica.",
        CommandName::CloseSettings => "Fechar a página de definições.",
        CommandName::FocusPane => "Levar o foco do teclado para um painel.",
        CommandName::SetListFilter => "Filtrar a lista: não lidas, com estrela, com anexos.",
        CommandName::FocusMailbox => "Focar numa pasta ou conta: a barra lateral mostra só isso.",
        CommandName::ClearFocus => "Sair do modo foco: a barra lateral volta a mostrar tudo.",
        CommandName::CollapseFolders => "Recolher pastas na árvore da barra lateral.",
        CommandName::ExpandFolders => "Expandir pastas na árvore da barra lateral.",
        CommandName::ToggleSidebar => "Recolher ou expandir a barra lateral.",
        CommandName::CompactStore => {
            "Compactar o armazenamento local e remover corpos de mensagem órfãos."
        }
        CommandName::GetView => "Mostrar o que o utilizador está a ver na interface gráfica.",
        CommandName::CheckForUpdates => "Procurar agora uma nova versão do dail.",
        CommandName::DownloadUpdate => {
            "Transferir a atualização disponível e verificar o seu tamanho e a sua assinatura."
        }
        CommandName::InstallUpdate => "Instalar a atualização transferida.",
        CommandName::SkipUpdateVersion => "Ignorar uma versão: não voltará a ser oferecida.",
        CommandName::SetUpdatePolicy => {
            "Alterar a verificação periódica de atualizações e o seu intervalo."
        }
        CommandName::ListApprovals => {
            "Listar os pedidos de envio de agentes que aguardam a sua decisão."
        }
        CommandName::ApproveRequest => "Aprovar o pedido de envio de um agente.",
        CommandName::RejectRequest => "Rejeitar o pedido de envio de um agente.",
        CommandName::ListDevices => {
            "Listar os dispositivos emparelhados com este servidor dail, com a função e a última ligação de cada um."
        }
        CommandName::StartPairing => {
            "Criar um código de utilização única, válido durante 10 minutos, para emparelhar um dispositivo com este servidor dail."
        }
        CommandName::RevokeDevice => {
            "Revogar um dispositivo emparelhado: perde o acesso ao servidor dail."
        }
        CommandName::ConnectServer => {
            "Associar este perfil a um servidor dail e mostrar a impressão digital do respetivo certificado."
        }
        CommandName::TrustServer => {
            "Confiar no certificado do servidor dail e emparelhar com o código de emparelhamento."
        }
        CommandName::DisconnectServer => {
            "Desassociar este perfil do servidor dail: o correio copiado e as alterações offline pendentes perdem-se."
        }
        CommandName::RetryOffline => {
            "Voltar a pôr na fila as alterações offline que o servidor dail recusou."
        }
        CommandName::DismissOfflineRejection => {
            "Rejeitar uma alteração offline recusada pelo servidor dail, sem a reenviar."
        }
    }
}

/// The Latin one-sentence help of `name`.
pub(super) const fn latin_help(name: CommandName) -> &'static str {
    match name {
        CommandName::GetSettings => "Optiones praesentes monstra.",
        CommandName::SetKeybinding => "Claves mandato tribue, loco earum praeformati compendiorum.",
        CommandName::SetKeyPreset => {
            "Praeformatum compendiorum initiale elige: `gmail` (praesumptum) aut `vim`."
        }
        CommandName::SetTheme => {
            "Speciem elige: modum (systema, lucidum, obscurum) et thema cuiusque modi."
        }
        CommandName::SetSetting => "Optionem muta.",
        CommandName::Undo => "Ultimum mandatum revocabile revoca.",
        CommandName::Redo => "Ultimum mandatum revocatum restitue.",
        CommandName::OpenPalette => "Tabulam mandatorum aperi.",
        CommandName::ClosePalette => "Tabulam mandatorum claude.",
        CommandName::AddAccount => "Rationem cursus electronici IMAP/SMTP adde.",
        CommandName::DiscoverServers => "Servitores IMAP et SMTP dominii inscriptionis quaere.",
        CommandName::TestAccount => {
            "Rationem inspice: ad servitores IMAP et SMTP tessera servata coniunge."
        }
        CommandName::UpdateAccount => "Nomen monstrandum aut servitores rationis muta.",
        CommandName::SetPassword => "Tesseram rationis substitue.",
        CommandName::SetIdentity => {
            "Inscriptionem missionis (aliam) adde, aut nomen eius vel subscriptionem muta."
        }
        CommandName::RemoveIdentity => {
            "Aliam remove, aut nomen et subscriptionem inscriptionis rationis."
        }
        CommandName::TrustCertificate => {
            "Pro ratione confide testimonio a se ipso signato aut auctoritati privatae quam unus eius servitorum praebuit."
        }
        CommandName::ForgetCertificate => "Testimonio pro ratione accepto confidere desine.",
        CommandName::SetMailboxIdentity => {
            "Inscriptionem missionis novarum adumbrationum in capsa scriptarum elige."
        }
        CommandName::CreateFolder => "Capsam in servitore crea.",
        CommandName::RenameFolder => "Capsam in servitore renomina aut move, cum subcapsis eius.",
        CommandName::DeleteFolder => "Capsam e servitore dele, cum omnibus eius epistulis.",
        CommandName::EmptyTrash => {
            "Omnes epistulas in quisquiliis rationis in perpetuum dele, etiam in servitore."
        }
        CommandName::EmptyJunk => {
            "Omnes epistulas purgamenti rationis in perpetuum dele, etiam in servitore."
        }
        CommandName::SetDraftFrom => "Inscriptionem missionis adumbrationis elige.",
        CommandName::RemoveAccount => "Rationem remove et tesseram eius e clavario dele.",
        CommandName::PullSettings => "Optiones machinis communes e servitore cursus lege et misce.",
        CommandName::PushSettings => {
            "Optiones communes in servitore cursus pro ceteris machinis serva."
        }
        CommandName::SetSettingsAccount => {
            "Rationem elige cuius servitor optiones machinis communes servat."
        }
        CommandName::ListRules => "Regulas quae epistulas novas digerunt enumera.",
        CommandName::SaveRule => {
            "Regulam quae epistulas novas digerit crea aut substitue; nec transmittere nec respondere potest."
        }
        CommandName::DeleteRule => "Regulam digerendi dele, in omnibus machinis.",
        CommandName::EnableRule => "Regulam digerendi accende aut exstingue sine deletione.",
        CommandName::ApplyRules => "Regulas digerendi nunc epistulis in memoria capsae adhibe.",
        CommandName::SyncNow => "Nunc synchroniza: capsas enumera et epistulas novas deprime.",
        CommandName::ListMailboxes => "Capsas in memoria enumera, cum munere et numeris earum.",
        CommandName::ListThreads => "Colloquia capsae enumera, recentissima prima.",
        CommandName::SetSyncWindow => {
            "Elige quot recentissimae epistulae capsae in memoria locali serventur."
        }
        CommandName::OpenUnifiedInbox => "Capsas acceptorum omnium rationum monstra (capsa unita).",
        CommandName::OpenMailbox => "Capsam in indice colloquiorum monstra.",
        CommandName::SelectThreads => "Selectionem colloquiorum substitue.",
        CommandName::CheckThreads => "Colloquia signa ut in pluribus simul agas.",
        CommandName::UncheckThreads => "Colloquia dissigna.",
        CommandName::OpenThread => "Colloquium in lectore aperi.",
        CommandName::CloseThread => "Colloquium in lectore monstratum claude.",
        CommandName::ExpandMessages => "Epistulas colloquii aperti expande.",
        CommandName::CollapseMessages => "Epistulas colloquii aperti in unum versum contrahe.",
        CommandName::GetThread => {
            "Colloquium et omnes eius epistulas monstra, antiquissimas primas."
        }
        CommandName::GetMessage => "Epistulam monstra.",
        CommandName::ShowImagesFrom => "Imagines in epistulis huius missoris inclusas monstra.",
        CommandName::HideImagesFrom => {
            "Imagines in epistulis huius missoris inclusas monstrare desine."
        }
        CommandName::PinMailbox => "Capsam in *Dilectis* in summa tabula laterali fige.",
        CommandName::UnpinMailbox => "Capsam e *Dilectis* tabulae lateralis remove.",
        CommandName::HideMailbox => "Capsam in arbore tabulae lateralis cela.",
        CommandName::ShowMailbox => "Capsam in arbore tabulae lateralis iterum monstra.",
        CommandName::SaveAttachments => {
            "Adiuncta epistulae depressae in capsa huius computatri serva."
        }
        CommandName::MarkRead => "Colloquia aut epistulas ut lecta nota.",
        CommandName::MarkMailboxRead => {
            "Omnes epistulas capsae ut lectas nota, etiam extra memoriam."
        }
        CommandName::MarkUnread => "Colloquia aut epistulas ut non lecta nota.",
        CommandName::Star => "Colloquia aut epistulas stella nota.",
        CommandName::Unstar => "Stellam e colloquiis aut epistulis remove.",
        CommandName::PinThreads => "Colloquia in summis indicibus qui ea monstrant fige.",
        CommandName::UnpinThreads => "Colloquia aut epistulas refige.",
        CommandName::ArchiveThreads => "Colloquia aut epistulas in archivum conde.",
        CommandName::TrashThreads => "Colloquia aut epistulas in quisquilias move.",
        CommandName::MarkJunk => "Colloquia aut epistulas in capsam purgamenti move.",
        CommandName::MarkNotJunk => "Purgamentum in capsam acceptorum redde.",
        CommandName::MoveThreads => {
            "Colloquia aut epistulas in aliam capsam eiusdem rationis move."
        }
        CommandName::ListPendingOps => {
            "Mutationes locales quae servitorem nondum attigerunt enumera."
        }
        CommandName::RetryPendingOps => "Operationes exspectantes nunc iterum conare.",
        CommandName::DiscardPendingOp => {
            "Operationem exspectantem abice: mutatio eius localiter revocatur."
        }
        CommandName::CreateDraft => "Novam epistulam ex ratione incipe.",
        CommandName::Reply => "Responsum missori epistulae incipe.",
        CommandName::ReplyAll => "Missori et omnibus ceteris recipientibus responde.",
        CommandName::Forward => "Epistulam transmittere incipe.",
        CommandName::UpdateDraft => "Campos adumbrationis muta.",
        CommandName::DiscardDraft => "Adumbrationem dele.",
        CommandName::EditDraft => {
            "Adumbrationem in colloquio monstratam recense, etiam in alia machina scriptam."
        }
        CommandName::AttachFile => "Documentum locale adumbrationi adiunge.",
        CommandName::RemoveAttachment => {
            "Adiunctum ex adumbratione remove (documentum non tangitur)."
        }
        CommandName::SendDraft => "Adumbrationem mitte.",
        CommandName::CancelSend => {
            "Missionem constitutam ante discessum siste; adumbratio iterum recenseri potest."
        }
        CommandName::RestoreDraft => "Adumbrationem deletam ut erat restitue.",
        CommandName::ListContacts => "Personas in epistulis in memoria apparentes enumera.",
        CommandName::Search => "In epistulis in memoria quaere.",
        CommandName::SetSearch => "Effectus quaestionis loco capsae apertae monstra.",
        CommandName::ClearSearch => "A quaestione exi: index iterum capsam apertam monstrat.",
        CommandName::OpenComposer => {
            "Adumbrationem in fenestra scribendi monstra et intentionem eo duc."
        }
        CommandName::CloseComposer => "Fenestram scribendi claude.",
        CommandName::OpenSettings => "Paginam optionum monstra, ad certam sectionem.",
        CommandName::CloseSettings => "Paginam optionum claude.",
        CommandName::FocusPane => "Intentionem claviaturae ad tabulatum duc.",
        CommandName::SetListFilter => "Indicem cola: non lecta, stellata, cum adiunctis.",
        CommandName::FocusMailbox => {
            "In capsa aut ratione intende: tabula lateralis id solum monstrat."
        }
        CommandName::ClearFocus => {
            "A modo intentionis exi: tabula lateralis iterum omnia monstrat."
        }
        CommandName::CollapseFolders => "Capsas in arbore tabulae lateralis contrahe.",
        CommandName::ExpandFolders => "Capsas in arbore tabulae lateralis expande.",
        CommandName::ToggleSidebar => "Tabulam lateralem contrahe aut expande.",
        CommandName::CompactStore => {
            "Repositorium locale comprime et corpora epistularum orba dele."
        }
        CommandName::GetView => "Monstra quod usor in interfacie graphica aspicit.",
        CommandName::CheckForUpdates => "Novam versionem dail nunc quaere.",
        CommandName::DownloadUpdate => {
            "Renovationem praesto deprime et magnitudinem eius ac signum inspice."
        }
        CommandName::InstallUpdate => "Renovationem depressam institue.",
        CommandName::SkipUpdateVersion => "Versionem praetermitte: non iam offeretur.",
        CommandName::SetUpdatePolicy => {
            "Inspectionem periodicam renovationum et intervallum eius muta."
        }
        CommandName::ListApprovals => {
            "Petitiones missionis agentium quae iudicium tuum exspectant enumera."
        }
        CommandName::ApproveRequest => "Petitionem missionis agentis proba.",
        CommandName::RejectRequest => "Petitionem missionis agentis reice.",
        CommandName::ListDevices => {
            "Instrumenta huic servitori dail copulata enumera, cum munere et ultima conexione."
        }
        CommandName::StartPairing => {
            "Notam semel utendam, decem minutis validam, crea ut instrumentum huic servitori dail copules."
        }
        CommandName::RevokeDevice => {
            "Instrumentum copulatum revoca: aditum ad servitorem dail amittit."
        }
        CommandName::ConnectServer => {
            "Hanc profilem cum servitore dail conecte et vestigium testimonii eius monstra."
        }
        CommandName::TrustServer => {
            "Testimonio servitoris dail confide et nota copulationis copula."
        }
        CommandName::DisconnectServer => {
            "Hanc profilem a servitore dail disiunge: epistulae exscriptae et mutationes sine conexione exspectantes amittuntur."
        }
        CommandName::RetryOffline => {
            "Mutationes sine conexione a servitore dail recusatas iterum in ordinem pone."
        }
        CommandName::DismissOfflineRejection => {
            "Mutationem sine conexione a servitore dail recusatam abice, nec iterum mitte."
        }
    }
}

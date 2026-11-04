//! The strings of every language. A row is a key with its English and French templates, so a
//! key cannot miss a language; the doc of a key is its English text.

/// Declares [`Key`], [`Key::ALL`] and a function per language from one table.
macro_rules! strings {
    ($($key:ident: $english:literal, $french:literal;)*) => {
        /// A string of the table.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Key {
            $(#[doc = $english] $key,)*
        }

        impl Key {
            /// Every key, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$key,)*];
        }

        /// The English table.
        pub const fn english(key: Key) -> &'static str {
            match key {
                $(Key::$key => $english,)*
            }
        }

        /// The French table.
        pub const fn french(key: Key) -> &'static str {
            match key {
                $(Key::$key => $french,)*
            }
        }
    };
}

strings! {
    StoreFailed:
        "local store failed",
        "échec du stockage local";
    Quarantined:
        "{count} saved items could not be read",
        "{count} éléments enregistrés n’ont pas pu être lus";
    QuarantinedHint:
        "They are kept aside in the profile, not deleted: a later version of dail may read them. Report it with the version you updated from.",
        "Ils sont mis de côté dans le profil, pas supprimés : une version ultérieure de dail pourra peut-être les lire. Signalez-le en indiquant la version précédente.";
    KeyringFailed:
        "keyring failed",
        "échec du trousseau";
    KeyringHint:
        "The system keyring (Secret Service on Linux, Credential Manager on Windows) is locked or missing: unlock it, or start a keyring service.",
        "Le trousseau du système (Secret Service sous Linux, Gestionnaire d’identifiants sous Windows) est verrouillé ou absent : déverrouillez-le ou démarrez un service de trousseau.";
    LoginFailed:
        "{email}: login failed",
        "{email} : échec de la connexion";
    SyncFailed:
        "{email}: sync failed",
        "{email} : échec de la synchronisation";
    FixPassword:
        "re-enter password",
        "saisir à nouveau le mot de passe";
    FixRetry:
        "retry",
        "réessayer";
    FixTest:
        "test again",
        "tester à nouveau";
    HintPasswordGmail:
        "Gmail refused the password. It needs an App Password (Google account → Security → 2-Step Verification → App passwords), not your Google password.",
        "Gmail a refusé le mot de passe. Il faut un mot de passe d’application (compte Google → Sécurité → Validation en deux étapes → Mots de passe des applications), pas votre mot de passe Google.";
    HintPassword:
        "The server refused the password (or none is stored). Enter it again; if you changed it on the web, use the new one.",
        "Le serveur a refusé le mot de passe (ou aucun n’est enregistré). Saisissez-le à nouveau ; si vous l’avez changé sur le web, utilisez le nouveau.";
    HintQuota:
        "The mailbox is full. Delete or archive mail and empty the Trash, then retry.",
        "La boîte aux lettres est pleine. Supprimez ou archivez du courrier et videz la corbeille, puis réessayez.";
    HintTlsCiphers:
        "The server offers only TLS versions or ciphers dail does not support (dail needs TLS 1.2 or 1.3 with ChaCha20-Poly1305 or AES-GCM). Ask the provider; nothing in dail's settings changes this.",
        "Le serveur ne propose que des versions ou des chiffrements TLS que dail ne prend pas en charge (dail exige TLS 1.2 ou 1.3 avec ChaCha20-Poly1305 ou AES-GCM). Contactez le fournisseur ; aucun réglage de dail n’y change rien.";
    HintCertificate:
        "The server's certificate is not trusted: expired, self-signed or for another name. Check the server name; dail does not connect without a valid certificate.",
        "Le certificat du serveur n’est pas fiable : expiré, auto-signé ou émis pour un autre nom. Vérifiez le nom du serveur ; dail ne se connecte pas sans certificat valide.";
    HintNetwork:
        "The server could not be reached. Check the connection and the server name; syncing tries again by itself.",
        "Le serveur est injoignable. Vérifiez la connexion et le nom du serveur ; la synchronisation réessaie d’elle-même.";
    Syncing:
        "⟳ syncing…",
        "⟳ synchronisation…";
    Synced:
        "synced {age}",
        "synchronisé {age}";
    NoAccounts:
        "no accounts",
        "aucun compte";
    NotSynced:
        "not synced",
        "non synchronisé";
    NextPoll:
        "next {when}",
        "prochaine {when}";
    Pending:
        "{pending} pending",
        "{pending} en attente";
    PendingFailed:
        "{pending} pending · {failed} failed",
        "{pending} en attente · {failed} en échec";
    CollapseSidebar:
        "Collapse sidebar (Ctrl+B)",
        "Réduire la barre latérale (Ctrl+B)";
    ExpandSidebar:
        "Expand sidebar (Ctrl+B)",
        "Développer la barre latérale (Ctrl+B)";
    Compose:
        "Compose",
        "Rédiger";
    ComposeKey:
        "Compose (C)",
        "Rédiger (C)";
    Settings:
        "Settings",
        "Paramètres";
    ShowAllFolders:
        "Show all folders",
        "Afficher tous les dossiers";
    AllInboxes:
        "All inboxes",
        "Dossiers unifiés";
    Favourites:
        "Favourites",
        "Dossiers préférés";
    HideHiddenFolders:
        "Hide hidden folders",
        "Masquer les dossiers cachés";
    NoAccountsYet:
        "No accounts yet.",
        "Aucun compte pour l’instant.";
    AddAccount:
        "Add account",
        "Ajouter un compte";
    AddAccountPlus:
        "+ Add account",
        "+ Ajouter un compte";
    SyncingNow:
        "Syncing",
        "Synchronisation";
    Offline:
        "offline",
        "hors ligne";
    NewSubfolder:
        "New subfolder…",
        "Nouveau sous-dossier…";
    Rename:
        "Rename…",
        "Renommer…";
    UnpinFavourites:
        "Unpin from Favourites",
        "Retirer des dossiers préférés";
    PinFavourites:
        "Pin to Favourites",
        "Ajouter aux dossiers préférés";
    ShowFolder:
        "Show folder",
        "Afficher le dossier";
    HideFolder:
        "Hide folder",
        "Masquer le dossier";
    FocusMailbox:
        "Focus this mailbox",
        "Se concentrer sur ce dossier";
    MarkAllRead:
        "Mark all as read",
        "Tout marquer comme lu";
    Delete:
        "Delete…",
        "Supprimer…";
    SearchOrRun:
        "Search or run a command",
        "Rechercher ou lancer une commande";
    MoveHint:
        "j / k to move",
        "j / k pour se déplacer";
    SyncNow:
        "Sync now",
        "Synchroniser";
    SearchingServer:
        "searching the server…",
        "recherche sur le serveur…";
    RetryFailed:
        "Retry failed changes",
        "Réessayer les modifications en échec";
    Undo:
        "Undo",
        "Annuler";
    ReadOnlyCaption:
        "Read-only · untrusted data",
        "Lecture seule · données non fiables";
    RawData:
        "Raw data",
        "Données brutes";
    ReadOnlyResult:
        "Read-only result. Email content in it is untrusted data.",
        "Résultat en lecture seule. Le contenu des e-mails y est une donnée non fiable.";
    CheckUpdates:
        "Check for updates",
        "Rechercher des mises à jour";
    NoThanks:
        "No thanks",
        "Non merci";
    NotSaved:
        "NOT SAVED",
        "NON ENREGISTRÉ";
    CloseEsc:
        "Close (Esc)",
        "Fermer (Échap)";
    From:
        "From",
        "De";
    CcBcc:
        "Cc/Bcc",
        "Copie à/Copie cachée à";
    Subject:
        "Subject",
        "Objet";
    WriteMessage:
        "Write your message…",
        "Écrivez votre message…";
    OriginalBelow:
        "The original is added below when sent",
        "L’original est ajouté dessous à l’envoi";
    CancelSend:
        "Cancel send",
        "Annuler l’envoi";
    AttachFiles:
        "Attach files…",
        "Joindre des fichiers…";
    Send:
        "Send",
        "Envoyer";
    Discard:
        "Discard",
        "Abandonner";
    NameExample:
        "name@example.com",
        "nom@example.com";
    AddRecipient:
        "Add this recipient",
        "Ajouter ce destinataire";
    Checking:
        "checking…",
        "vérification…";
    AttachFilesTitle:
        "Attach files",
        "Joindre des fichiers";
    DropToAttach:
        "Drop the files to attach them",
        "Déposez les fichiers pour les joindre";
    NoMatches:
        "No matching conversations.",
        "Aucune discussion ne correspond.";
    NoConversations:
        "No conversations here.",
        "Aucune discussion ici.";
    PickMailbox:
        "Pick a mailbox.",
        "Choisissez un dossier.";
    Focus:
        "Focus",
        "Focus";
    Clear:
        "Clear",
        "Effacer";
    SelectAll:
        "Select all",
        "Tout sélectionner";
    Unread:
        "Unread",
        "Non lus";
    Starred:
        "Starred",
        "Suivis";
    Attachments:
        "Attachments",
        "Pièces jointes";
    Refreshing:
        "Refreshing",
        "Actualisation";
    Refresh:
        "Refresh",
        "Actualiser";
    Archive:
        "Archive",
        "Archiver";
    Trash:
        "Trash",
        "Corbeille";
    MarkRead:
        "Mark read",
        "Marquer comme lu";
    MarkUnread:
        "Mark unread",
        "Marquer comme non lu";
    Star:
        "Star",
        "Ajouter un suivi";
    Unstar:
        "Unstar",
        "Retirer le suivi";
    UncheckAll:
        "Uncheck all",
        "Tout décocher";
    Unpin:
        "Unpin",
        "Détacher";
    Pin:
        "Pin",
        "Épingler";
    NoSender:
        "(no sender)",
        "(sans expéditeur)";
    NoSubject:
        "(no subject)",
        "(pas de sujet)";
    Pinned:
        "Pinned",
        "Épinglé";
    MoveTo:
        "Move to…",
        "Déplacer vers…";
    QuotedText:
        "quoted text",
        "le texte cité";
    Signature:
        "signature",
        "la signature";
    NoConversationOpen:
        "No conversation open",
        "Aucune discussion ouverte";
    Reply:
        "Reply",
        "Répondre";
    ReplyAll:
        "Reply all",
        "Répondre à tous";
    Forward:
        "Forward",
        "Transférer";
    UnstarKey:
        "Unstar (S)",
        "Retirer le suivi (S)";
    StarKey:
        "Star (S)",
        "Ajouter un suivi (S)";
    MarkReadKey:
        "Mark read (U)",
        "Marquer comme lu (U)";
    MarkUnreadKey:
        "Mark unread (U)",
        "Marquer comme non lu (U)";
    ArchiveKey:
        "Archive (E)",
        "Archiver (E)";
    TrashKey:
        "Trash (#)",
        "Corbeille (#)";
    JunkKey:
        "Junk (!)",
        "Indésirable (!)";
    NotJunkKey:
        "Not junk (!)",
        "Pas indésirable (!)";
    MarkJunk:
        "Junk",
        "Indésirable";
    NotJunk:
        "Not junk",
        "Pas indésirable";
    MoveKey:
        "Move… (V)",
        "Déplacer… (V)";
    NextConversation:
        "Next conversation (j)",
        "Discussion suivante (j)";
    PreviousConversation:
        "Previous conversation (k)",
        "Discussion précédente (k)";
    OneMessage:
        "1 MESSAGE",
        "1 MESSAGE";
    OpenOriginal:
        "Open original",
        "Ouvrir l’original";
    OpenOriginalArrow:
        "Open original ↗",
        "Ouvrir l’original ↗";
    OpenOriginalHint:
        "A sanitized copy (no scripts, no images, nothing remote) in your browser",
        "Une copie assainie (ni scripts, ni images, rien de distant) dans votre navigateur";
    Loading:
        "Loading…",
        "Chargement…";
    PicturesShown:
        "Showing the pictures in this message.",
        "Les images de ce message sont affichées.";
    HideImages:
        "Hide images",
        "Masquer les images";
    PicturesHidden:
        "Pictures in this message are hidden.",
        "Les images de ce message sont masquées.";
    ShowImages:
        "Show images",
        "Afficher les images";
    PicturesHint:
        "Only pictures carried in the message; nothing is downloaded",
        "Seulement les images contenues dans le message ; rien n’est téléchargé";
    NoText:
        "(no text)",
        "(pas de texte)";
    Appearance:
        "Appearance",
        "Apparence";
    Keyboard:
        "Keyboard",
        "Clavier";
    General:
        "General",
        "Général";
    Accounts:
        "Accounts",
        "Comptes";
    CloseSettings:
        "Close settings (Esc)",
        "Fermer les paramètres (Échap)";
    NotTestedYet:
        "not tested yet",
        "pas encore testé";
    Connected:
        "connected",
        "connecté";
    Test:
        "Test",
        "Tester";
    ReEnterPassword:
        "Re-enter password",
        "Saisir à nouveau le mot de passe";
    KeepIt:
        "Keep it",
        "Le garder";
    RemoveDots:
        "Remove…",
        "Retirer…";
    Theme:
        "Theme",
        "Thème";
    ThemeHint:
        "Follow the desktop, or always light or dark",
        "Suivre le bureau, ou toujours clair ou sombre";
    System:
        "System",
        "Système";
    Light:
        "Light",
        "Clair";
    Dark:
        "Dark",
        "Sombre";
    LightTheme:
        "Light theme",
        "Thème clair";
    DarkTheme:
        "Dark theme",
        "Thème sombre";
    Density:
        "Density",
        "Densité";
    DensityHint:
        "How tall the rows of the thread list are",
        "La hauteur des lignes de la liste";
    Cozy:
        "Cozy 48",
        "Détendue 48";
    Compact:
        "Compact 40",
        "Compacte 40";
    PreviewDensity:
        "Preview 72",
        "Aperçu 72";
    TextSize:
        "Text size",
        "Taille du texte";
    TextSizeHint:
        "Ctrl + plus and minus change it for a while",
        "Ctrl + plus et moins la changent pour un moment";
    Preset:
        "Preset",
        "Préréglage";
    PresetHint:
        "The keys everything starts from",
        "Les touches de départ";
    FilterCommands:
        "Filter the commands",
        "Filtrer les commandes";
    Mail:
        "Mail",
        "Courrier";
    Navigation:
        "Navigation",
        "Navigation";
    Account:
        "Account",
        "Compte";
    App:
        "App",
        "Application";
    Change:
        "Change…",
        "Changer…";
    UndoSend:
        "Undo send",
        "Annulation de l’envoi";
    UndoSendHint:
        "How long a sent message waits before it leaves",
        "Le temps qu’un message attend avant de partir";
    Off:
        "Off",
        "Désactivé";
    CheckMail:
        "Check for mail",
        "Relève du courrier";
    CheckMailHint:
        "How often the inbox is polled without IDLE",
        "La fréquence de relève sans IDLE";
    MarkAsRead:
        "Mark as read",
        "Marquer comme lu";
    MarkAsReadHint:
        "When an open conversation is marked read (other values: set_setting in the palette)",
        "Quand une discussion ouverte est marquée comme lue (autres valeurs : set_setting dans la palette)";
    Never:
        "Never",
        "Jamais";
    Immediately:
        "Immediately",
        "Immédiatement";
    After2s:
        "After 2 s",
        "Après 2 s";
    After5s:
        "After 5 s",
        "Après 5 s";
    Notifications:
        "Notifications",
        "Notifications";
    NotificationsHint:
        "New mail, failed sends and agent requests while dail is in the background",
        "Nouveau courrier, envois en échec et demandes d’agents quand dail est en arrière-plan";
    UndoHistory:
        "Undo history",
        "Historique d’annulation";
    UndoHistoryHint:
        "How many steps undo remembers",
        "Le nombre d’étapes gardées";
    Updates:
        "Updates",
        "Mises à jour";
    CheckAutomatically:
        "Check automatically",
        "Vérifier automatiquement";
    CheckAutomaticallyHint:
        "Asks GitHub for the signed release list; nothing about you is sent",
        "Demande à GitHub la liste signée des versions ; rien sur vous n’est envoyé";
    HowOften:
        "How often",
        "Fréquence";
    HowOftenHint:
        "Between automatic checks",
        "Entre deux vérifications automatiques";
    ChangePassword:
        "Change the password",
        "Changer le mot de passe";
    AddAnAccount:
        "Add an account",
        "Ajouter un compte";
    Cancel:
        "Cancel",
        "Annuler";
    Next:
        "Next",
        "Suivant";
    EnterEmail:
        "Enter an email address like you@example.com.",
        "Saisissez une adresse comme vous@example.com.";
    EmailAddress:
        "Email address",
        "Adresse e-mail";
    NameOptional:
        "Name (optional)",
        "Nom (facultatif)";
    ShownInSidebar:
        "shown in the sidebar",
        "affiché dans la barre latérale";
    ImapServer:
        "IMAP server",
        "Serveur IMAP";
    SmtpServer:
        "SMTP server",
        "Serveur SMTP";
    HostPortHint:
        "host:port, optionally :tls or :starttls (143 and 587 use STARTTLS)",
        "hôte:port, avec :tls ou :starttls en option (143 et 587 utilisent STARTTLS)";
    FindServers:
        "Find servers",
        "Trouver les serveurs";
    Looking:
        "Looking…",
        "Recherche…";
    FindServersHint:
        "Asks the provider's web server, then Mozilla's list.",
        "Interroge le serveur web du fournisseur, puis la liste de Mozilla.";
    Back:
        "Back",
        "Retour";
    OauthOnly:
        "{provider} addresses need a sign-in that dail does not support yet. Use another address for now.",
        "Les adresses {provider} demandent une connexion que dail ne prend pas encore en charge. Utilisez une autre adresse pour l’instant.";
    CreateAppPassword:
        "Create an App Password",
        "Créer un mot de passe d’application";
    PrivacyPolicy:
        "Privacy policy",
        "Politique de confidentialité";
    AppPassword:
        "App Password",
        "Mot de passe d’application";
    Password:
        "Password",
        "Mot de passe";
    KeyringOnly:
        "Stored in the system keyring only; dail never writes it anywhere else.",
        "Stocké uniquement dans le trousseau du système ; dail ne l’écrit nulle part ailleurs.";
    KeystoreOnly:
        "Stored in Keystore-protected storage only; dail never writes it anywhere else.",
        "Stocké uniquement dans un espace protégé par le Keystore ; dail ne l’écrit nulle part ailleurs.";
    SaveAndTest:
        "Save and test",
        "Enregistrer et tester";
    AddAndTest:
        "Add and test",
        "Ajouter et tester";
    ConnectedLogin:
        "Connected. IMAP and SMTP both accepted the login.",
        "Connecté. IMAP et SMTP ont accepté tous deux la connexion.";
    Close:
        "Close",
        "Fermer";
    KeepAnyway:
        "Keep it anyway",
        "Le garder quand même";
    TryAnotherPassword:
        "Try another password",
        "Essayer un autre mot de passe";
    Provider1:
        "1 / 4  Provider",
        "1 / 4  Fournisseur";
    Address2:
        "2 / 4  Address",
        "2 / 4  Adresse";
    Password3:
        "3 / 4  Password",
        "3 / 4  Mot de passe";
    Connection4:
        "4 / 4  Connection",
        "4 / 4  Connexion";
    AMessage:
        "a message",
        "un message";
    ApproveSend:
        "Approve send",
        "Approuver l’envoi";
    Edit:
        "Edit",
        "Modifier";
    Reject:
        "Reject",
        "Rejeter";
    StillChecked:
        "attachments are still being checked",
        "les pièces jointes sont encore en cours de vérification";
    SaveAttachmentsInto:
        "Save attachments into",
        "Enregistrer les pièces jointes dans";
    OneFile:
        "1 file",
        "1 fichier";
    SaveAll:
        "Save all",
        "Tout enregistrer";
    SaveAllDots:
        "Save all…",
        "Tout enregistrer…";
    OpenArrow:
        "Open ↗",
        "Ouvrir ↗";
    HidePreview:
        "Hide preview",
        "Masquer l’aperçu";
    Preview:
        "Preview",
        "Aperçu";
    SaveDots:
        "Save…",
        "Enregistrer…";
    LoadingPreview:
        "Loading the preview…",
        "Chargement de l’aperçu…";
    Dismiss:
        "Dismiss",
        "Ignorer";
    NothingMatches:
        "Nothing matches.",
        "Aucun résultat.";
    Error:
        "Error",
        "Erreur";
    Sending:
        "Sending…",
        "Envoi…";
    Then:
        "then",
        "puis";
    NotSentNotice:
        "Not sent.",
        "Non envoyé.";
    AttachmentsChecked:
        "Its attachments are still being checked.",
        "Ses pièces jointes sont encore en cours de vérification.";
    SendRequests:
        " Send requests ",
        " Demandes d’envoi ";
    NoSendRequests:
        "No send requests.",
        "Aucune demande d’envoi.";
    ApprovalsCheckingFooter:
        "attachments are being checked · e edit · r reject · q close",
        "vérification des pièces jointes · e modifier · r rejeter · q fermer";
    ApprovalsFooter:
        "a approve · e edit · r reject · j/k next · q close",
        "a approuver · e modifier · r rejeter · j/k suivant · q fermer";
    QClose:
        "q close",
        "q fermer";
    OriginalBelowTui:
        "(the original is added below when sent)",
        "(l’original est ajouté dessous à l’envoi)";
    ReadOnlyWhileSent:
        "read-only while it is sent · z cancels",
        "lecture seule pendant l’envoi · z annule";
    ComposerFooter:
        "Ctrl+S send · Ctrl+O $EDITOR · Ctrl+K “Attach file” · Esc close",
        "Ctrl+S envoyer · Ctrl+O $EDITOR · Ctrl+K « Attach file » · Échap fermer";
    FromColon:
        "From: ",
        "De : ";
    PaletteTitle:
        " Palette ",
        " Palette ";
    NothingMatchesTui:
        "  Nothing matches.",
        "  Aucun résultat.";
    EnterUseEscBack:
        "Enter to use · Esc to go back",
        "Entrée pour utiliser · Échap pour revenir";
    PaletteFooter:
        "↑↓ move · Enter choose · Esc back",
        "↑↓ déplacer · Entrée choisir · Échap retour";
    ScrollClose:
        "↑↓ Space b scroll · q or Esc close",
        "↑↓ Espace b défiler · q ou Échap fermer";
    ScrollView:
        "↑↓ Space b scroll · r view · q or Esc close",
        "↑↓ Espace b défiler · r vue · q ou Échap fermer";
    ScrollOpen:
        "↑↓ scroll · Tab Enter open · r raw data · q or Esc close",
        "↑↓ défiler · Tab Entrée ouvrir · r données brutes · q ou Échap fermer";
    NoConversationTui:
        "No conversation open. Enter opens the selected one.",
        "Aucune discussion ouverte. Entrée ouvre celle qui est sélectionnée.";
    Keys:
        "Keys",
        "Touches";
    NotTested:
        "not tested",
        "non testé";
    SettingsTitle:
        " Settings ",
        " Paramètres ";
    SettingsFooter:
        "Tab section · j/k move · Enter change · Esc close",
        "Tab section · j/k déplacer · Entrée changer · Échap fermer";
    AddAccountTitle:
        " Add account ",
        " Ajouter un compte ";
    Step1:
        "1/4 Provider",
        "1/4 Fournisseur";
    Step2:
        "2/4 Address",
        "2/4 Adresse";
    Step3:
        "3/4 Password",
        "3/4 Mot de passe";
    Step4:
        "4/4 Connection",
        "4/4 Connexion";
    ChooseFooter:
        "↑↓ choose · Enter next · Esc cancel",
        "↑↓ choisir · Entrée suivant · Échap annuler";
    Email:
        "Email",
        "E-mail";
    ImapHostPort:
        "IMAP host:port",
        "IMAP hôte:port";
    SmtpHostPort:
        "SMTP host:port",
        "SMTP hôte:port";
    PortsHint:
        "Ports 143 and 587 use STARTTLS, others TLS; add :tls or :starttls to choose.",
        "Les ports 143 et 587 utilisent STARTTLS, les autres TLS ; ajoutez :tls ou :starttls pour choisir.";
    CtrlFHint:
        "Ctrl+F finds the servers (asks the provider's web server, then Mozilla's list).",
        "Ctrl+F trouve les serveurs (interroge le serveur web du fournisseur, puis la liste de Mozilla).";
    FieldFooter:
        "Tab field · Enter next · Esc back",
        "Tab champ · Entrée suivant · Échap retour";
    KeyringTui:
        "It goes to the system keyring, never into dail's files.",
        "Il va dans le trousseau du système, jamais dans les fichiers de dail.";
    SaveTestFooter:
        "Enter save and test · Esc cancel",
        "Entrée enregistrer et tester · Échap annuler";
    AddTestFooter:
        "Enter add and test · Esc back",
        "Entrée ajouter et tester · Échap retour";
    ConnectedCap:
        "Connected.",
        "Connecté.";
    SyncCloseFooter:
        "Enter syncs now · Esc closes",
        "Entrée synchronise · Échap ferme";
    LoginFailedCap:
        "The login failed:",
        "La connexion a échoué :";
    PasswordClose:
        "p another password · k close",
        "p autre mot de passe · k fermer";
    PasswordKeep:
        "p another password · k keep it anyway",
        "p autre mot de passe · k le garder quand même";
    TestingLogin:
        "Testing the login…",
        "Test de la connexion…";
    QuitConfirm:
        "Quitting cancels the send still in its undo window. q quits, any other key stays",
        "Quitter annule l’envoi encore dans son délai d’annulation. q quitte, toute autre touche reste";
    PickMailboxTui:
        "Pick a mailbox: g i for the inbox.",
        "Choisissez un dossier : g i pour le courrier entrant.";
    PressEnterToAdd:
        "Press Enter to add one here, or run in another shell:",
        "Appuyez sur Entrée pour en ajouter un ici, ou lancez dans un autre terminal :";
    AccountShowsUp:
        "The account shows up here once it is added.",
        "Le compte apparaît ici une fois ajouté.";
    SetEditor:
        "Set $VISUAL or $EDITOR to write the body in your editor",
        "Définissez $VISUAL ou $EDITOR pour écrire le message dans votre éditeur";
    StillHere:
        "Still here; the send goes out when its window closes.",
        "Toujours là ; l’envoi part à la fin de son délai.";
    MouseOn:
        "Mouse on: tap to select, swipe to scroll",
        "Souris active : touchez pour sélectionner, glissez pour défiler";
    MouseOff:
        "Mouse off: select text with the terminal; m turns it back on",
        "Souris inactive : sélectionnez le texte avec le terminal ; m la réactive";
    HelpNote:
        "Problems, the keys of the registry, the TUI's own. q, Esc or ? closes.",
        "Problèmes, touches du registre, touches du terminal. q, Échap ou ? ferme.";
    TerminalTooSmall:
        "Terminal too small",
        "Terminal trop petit";
    Mailboxes:
        "Mailboxes",
        "Dossiers";
    Message:
        "Message",
        "Message";
    Draft:
        "Draft",
        "Brouillon";
    Conversations:
        "Conversations",
        "Discussions";
    FocusFooter:
        "Focus · palette: Exit focus",
        "Focus · palette : Exit focus";
    OfflineSuffix:
        " offline",
        " hors ligne";
    FixPalettePassword:
        " (palette: Change account password)",
        " (palette : Change account password)";
    FixPaletteSync:
        " (palette: Sync now)",
        " (palette : Sync now)";
    FixPaletteTest:
        " (palette: Test account connection)",
        " (palette : Test account connection)";
    UpdatesQuestion:
        "Check for updates? (settings: update_checks)",
        "Rechercher des mises à jour ? (paramètres : update_checks)";
    HelpHint:
        "? help",
        "? aide";
    Problems:
        "Problems\n",
        "Problèmes\n";
    FolderList:
        "{name} folders",
        "Dossiers de {name}";
    SubfolderList:
        "{name} subfolders",
        "Sous-dossiers de {name}";
    ShowHidden:
        "Show hidden ({n})",
        "Afficher les masqués ({n})";
    ChangesWait:
        "{n} change(s) wait for the server",
        "{n} modification(s) en attente du serveur";
    CollapseWhat:
        "Collapse {what}",
        "Réduire {what}";
    ExpandWhat:
        "Expand {what}",
        "Développer {what}";
    FetchingOlder:
        "Fetching older mail: {cached} of {wanted} cached",
        "Récupération du courrier ancien : {cached} sur {wanted} en cache";
    SyncNowKeys:
        "Sync now ({keys})",
        "Synchroniser ({keys})";
    DraftSaveFailed:
        "Draft save failed: {error}",
        "Échec de l’enregistrement du brouillon : {error}";
    NotSent:
        "Not sent: {error}",
        "Non envoyé : {error}";
    RemoveName:
        "Remove {name}",
        "Retirer {name}";
    SearchTitle:
        "Search: {query}",
        "Recherche : {query}";
    UnreadCount:
        "{n} unread",
        "{n} non lus";
    CheckedCount:
        "{n} checked",
        "{n} cochés";
    LabelConversation:
        "{label} conversation",
        "{label} la discussion";
    HideWhat:
        "Hide {what}",
        "Masquer {what}";
    ShowWhat:
        "Show {what}",
        "Afficher {what}";
    NOfM:
        "{n} of {m}",
        "{n} sur {m}";
    MessagesCount:
        "{n} MESSAGES",
        "{n} MESSAGES";
    ExpandMessage:
        "Expand message from {from}",
        "Développer le message de {from}";
    CollapseMessage:
        "Collapse message from {from}",
        "Réduire le message de {from}";
    ToList:
        "to {to}",
        "pour {to}";
    BodyFailed:
        "The body could not be downloaded: {error}",
        "Le message n’a pas pu être téléchargé : {error}";
    ActionFrom:
        "{action} from {sender}",
        "{action} de {sender}";
    LinksCount:
        "Links ({n})",
        "Liens ({n})";
    OpenInBrowser:
        "Open in your browser:\n{url}",
        "Ouvrir dans votre navigateur :\n{url}";
    FailedError:
        "failed: {error}",
        "échec : {error}";
    RemoveAccountCache:
        "Remove {email} and its mail cache",
        "Supprimer {email} et son cache de courrier";
    ChangeKeysOf:
        "Change the keys of {name}",
        "Changer les touches de {name}";
    TestConnection:
        "{email}  Connection",
        "{email}  Connexion";
    FoundAt:
        "Found at {source}",
        "Trouvé sur {source}";
    NotFound:
        "Not found ({error}); enter them yourself.",
        "Introuvable ({error}) ; saisissez-les vous-même.";
    SigningIn:
        "Signing in to {email}…",
        "Connexion à {email}…";
    LoginFailedError:
        "The login failed: {error}",
        "La connexion a échoué : {error}";
    NMessages:
        "{n} messages",
        "{n} messages";
    FromName:
        "from {from}",
        "de {from}";
    NFiles:
        "{n} files",
        "{n} fichiers";
    AttachmentsSummary:
        "Attachments ({files} · {size})",
        "Pièces jointes ({files} · {size})";
    SavingInto:
        "Saving into {dir}…",
        "Enregistrement dans {dir}…";
    SaveFailed:
        "Could not save the attachments: {error}",
        "Impossible d’enregistrer les pièces jointes : {error}";
    NoPreview:
        "No preview: {error}",
        "Pas d’aperçu : {error}";
    SavedAs:
        "Saved as {name}",
        "Enregistré sous {name}";
    SavedFiles:
        "Saved {n} files: {names}",
        "{n} fichiers enregistrés : {names}";
    AgentRequest:
        "Agent request: send {what} — check it before approving",
        "Demande d’agent : envoyer {what} — vérifiez avant d’approuver";
    AgentWantsSend:
        "! An agent wants to send {what}",
        "! Un agent veut envoyer {what}";
    ApprovalsHintTui:
        " — Ctrl+K, “Show send requests”",
        " — Ctrl+K, « Show send requests »";
    SendNowQuestion:
        "Send “{subject}” now? y sends (with the undo window), any other key doesn't",
        "Envoyer « {subject} » maintenant ? y envoie (avec le délai d’annulation), toute autre touche non";
    FromLine:
        "  From: {from}",
        "  De : {from}";
    SubjectLine:
        "Subject: {subject}",
        "Objet : {subject}";
    FromHeader:
        "From: {from}",
        "De : {from}";
    DateLine:
        "In: {mailbox}",
        "Dans : {mailbox}";
    AttachmentsCount:
        "Attachments ({n})",
        "Pièces jointes ({n})";
    ChangePasswordOf:
        "Change the password · {email}",
        "Changer le mot de passe · {email}";
    AddAccountStep:
        "Add account · {step}",
        "Ajouter un compte · {step}";
    FoundFill:
        "Found at {source}: {imap} and {smtp}. Ctrl+F fills them in.",
        "Trouvé sur {source} : {imap} et {smtp}. Ctrl+F les remplit.";
    EditorFailed:
        "The editor failed: {reason}",
        "L’éditeur a échoué : {reason}";
    TitleFocus:
        "{title} · Focus",
        "{title} · Focus";
    TitleOnly:
        "{title} · only {only}",
        "{title} · seulement {only}";
    SendingCancel:
        "{sending} · z cancels",
        "{sending} · z annule";
    MoreDetails:
        " (+{n} more, ? for details)",
        " (+{n} autres, ? pour les détails)";
    RetryPalette:
        "{text} (retry: palette)",
        "{text} (réessayer : palette)";
    UndoLabel:
        "“{label}” z undo",
        "« {label} » z annuler";
    BadgeSettings:
        "{badge} (settings)",
        "{badge} (paramètres)";
    PresetGmailHint:
        "Needs an App Password (2-Step Verification on).",
        "Nécessite un mot de passe d’application (validation en deux étapes activée).";
    PresetHostingerHint:
        "Your mailbox password.",
        "Le mot de passe de votre compte.";
    PresetCustomHint:
        "Enter the servers yourself.",
        "Saisissez les serveurs vous-même.";
    PresetGmail:
        "Gmail",
        "Gmail";
    PresetHostinger:
        "Hostinger Mail",
        "Hostinger Mail";
    PresetOther:
        "Other IMAP / SMTP",
        "Autre IMAP / SMTP";
    HintChoose:
        "choose",
        "choisir";
    HintBack:
        "back",
        "retour";
    HintOpen:
        "open",
        "ouvrir";
    HintEverything:
        "everything",
        "tout";
    HintNext:
        "next",
        "suivant";
    HintPrevious:
        "previous",
        "précédent";
    HintSelect:
        "select",
        "sélectionner";
    ToLabel:
        "To",
        "Pour";
    ReleaseNotes:
        "  release notes",
        "  notes de version";
    DateLabel:
        "Date: {date} ({age})",
        "Date : {date} ({age})";
    UnknownSender:
        "(unknown sender)",
        "(expéditeur inconnu)";
    SendingFailed:
        "Sending failed",
        "Échec de l’envoi";
    AnAgentWantsToSendAMessage:
        "An agent wants to send a message",
        "Un agent veut envoyer un message";
    To:
        "To:",
        "Pour :";
    Cc:
        "Cc:",
        "Copie à :";
    Bcc:
        "Bcc:",
        "Copie cachée à :";
    BccX:
        "Bcc",
        "Copie cachée à";
    SendingX:
        "sending…",
        "envoi…";
    NotSavedToTheServer:
        "not saved to the server",
        "non enregistré sur le serveur";
    Saving:
        "saving…",
        "enregistrement…";
    Saved:
        "saved",
        "enregistré";
    DraftX:
        "draft",
        "brouillon";
    NewMessage:
        "New message",
        "Nouveau message";
    January:
        "January",
        "janvier";
    February:
        "February",
        "février";
    March:
        "March",
        "mars";
    April:
        "April",
        "avril";
    May:
        "May",
        "mai";
    June:
        "June",
        "juin";
    July:
        "July",
        "juillet";
    August:
        "August",
        "août";
    September:
        "September",
        "septembre";
    October:
        "October",
        "octobre";
    November:
        "November",
        "novembre";
    December:
        "December",
        "décembre";
    Jan:
        "Jan",
        "janv.";
    Feb:
        "Feb",
        "févr.";
    Mar:
        "Mar",
        "mars";
    Apr:
        "Apr",
        "avr.";
    Jun:
        "Jun",
        "juin";
    Jul:
        "Jul",
        "juil.";
    Aug:
        "Aug",
        "août";
    Sep:
        "Sep",
        "sept.";
    Oct:
        "Oct",
        "oct.";
    Nov:
        "Nov",
        "nov.";
    Dec:
        "Dec",
        "déc.";
    Later:
        "Later",
        "Plus tard";
    Today:
        "Today",
        "Aujourd’hui";
    Yesterday:
        "Yesterday",
        "Hier";
    ThisWeek:
        "This week",
        "Cette semaine";
    EarlierThisMonth:
        "Earlier this month",
        "Plus tôt ce mois-ci";
    Now:
        "now",
        "maintenant";
    JustNow:
        "just now",
        "à l’instant";
    Inbox:
        "Inbox",
        "Courrier entrant";
    Sent:
        "Sent",
        "Envoyés";
    Drafts:
        "Drafts",
        "Brouillons";
    Junk:
        "Junk",
        "Indésirables";
    AllMail:
        "All Mail",
        "Tous les messages";
    WhichConversation:
        "Which conversation?",
        "Quelle discussion ?";
    WhichDraft:
        "Which draft?",
        "Quel brouillon ?";
    WhichAccount:
        "Which account?",
        "Quel compte ?";
    RemoveThisAccountThisCannotBeUndone:
        "Remove this account? This cannot be undone.",
        "Supprimer ce compte ? C’est irréversible.";
    InstallTheUpdateDailAsksToRestart:
        "Install the update? dail asks to restart afterwards.",
        "Installer la mise à jour ? dail demandera de redémarrer ensuite.";
    ItsName:
        "its name",
        "son nom";
    NewNameFullPath:
        "New name (full path)?",
        "Nouveau nom (chemin complet) ?";
    DeleteThisFolderAndAllMailIn:
        "Delete this folder and all mail in it? This cannot be undone.",
        "Supprimer ce dossier et tout son courrier ? C’est irréversible.";
    FindTheMailServersOfWhichAddress:
        "Find the mail servers of which address?",
        "Trouver les serveurs de quelle adresse ?";
    YouExampleOrgAsksTheProviderThen:
        "you@example.org (asks the provider, then Mozilla)",
        "vous@example.org (interroge le fournisseur, puis Mozilla)";
    WhichTheme:
        "Which theme?",
        "Quel thème ?";
    SystemLightOrDarkThenATheme:
        "system, light or dark, then a theme for that mode: dark dail-dark-hc",
        "system, light ou dark, puis un thème pour ce mode : dark dail-dark-hc";
    WhichCommand:
        "Which command?",
        "Quelle commande ?";
    ItsNameEGArchiveThreads:
        "its name, e.g. archive_threads",
        "son nom, p. ex. archive_threads";
    WhichKeysCommaSeparatedEmptyThePreset:
        "Which keys? (comma-separated; empty: the preset's)",
        "Quelles touches ? (séparées par des virgules ; vide : celles du préréglage)";
    WhichSetting:
        "Which setting?",
        "Quel paramètre ?";
    NewDisplayName:
        "New display name",
        "Nouveau nom affiché";
    WhichMailbox:
        "Which mailbox?",
        "Quel dossier ?";
    KeepHowManyOfTheNewestMessages:
        "Keep how many of the newest messages offline?",
        "Garder combien de messages récents hors ligne ?";
    ANumber0KeepsAllOfIt:
        "a number; 0 keeps all of it",
        "un nombre ; 0 garde tout";
    GoToMailbox:
        "Go to mailbox",
        "Aller au dossier";
    WhichAddress:
        "Which address?",
        "Quelle adresse ?";
    ItsSignatureEmptyNone:
        "Its signature? (empty: none)",
        "Sa signature ? (vide : aucune)";
    NameCompany:
        "Name · Company",
        "Nom · Société";
    RemoveWhichAddress:
        "Remove which address?",
        "Retirer quelle adresse ?";
    SendFromWhichAddressEmptyTheAccount:
        "Send from which address? (empty: the account's own)",
        "Envoyer depuis quelle adresse ? (vide : celle du compte)";
    SendFromWhichAddress:
        "Send from which address?",
        "Envoyer depuis quelle adresse ?";
    ListContactsMatching:
        "List contacts matching…",
        "Lister les contacts correspondant à…";
    ANameOrAnAddress:
        "a name or an address",
        "un nom ou une adresse";
    SelectWhichConversation:
        "Select which conversation?",
        "Sélectionner quelle discussion ?";
    OpenWhichConversation:
        "Open which conversation?",
        "Ouvrir quelle discussion ?";
    ShowImagesFromWhichSender:
        "Show images from which sender?",
        "Afficher les images de quel expéditeur ?";
    HideImagesFromWhichSender:
        "Hide images from which sender?",
        "Masquer les images de quel expéditeur ?";
    SaveTheAttachmentsIntoWhichFolder:
        "Save the attachments into which folder?",
        "Enregistrer les pièces jointes dans quel dossier ?";
    AnAbsolutePathEGHomeMe:
        "an absolute path, e.g. /home/me/Downloads",
        "un chemin absolu, p. ex. /home/moi/Téléchargements";
    MoveWhichConversation:
        "Move which conversation?",
        "Déplacer quelle discussion ?";
    DiscardWhichChange:
        "Discard which change?",
        "Abandonner quelle modification ?";
    AttachWhichFile:
        "Attach which file?",
        "Joindre quel fichier ?";
    AbsolutePathEGHomeMeReport:
        "absolute path, e.g. /home/me/report.pdf",
        "chemin absolu, p. ex. /home/moi/rapport.pdf";
    AttachToWhichDraft:
        "Attach to which draft?",
        "Joindre à quel brouillon ?";
    RemoveWhichAttachment:
        "Remove which attachment?",
        "Supprimer quelle pièce jointe ?";
    FromWhichDraft:
        "From which draft?",
        "De quel brouillon ?";
    SearchMailResultsAsData:
        "Search mail (results as data)",
        "Rechercher dans le courrier (résultats en données)";
    Search:
        "Search",
        "Rechercher";
    FocusWhichPane:
        "Focus which pane?",
        "Activer quel panneau ?";
    FocusOnWhichMailbox:
        "Focus on which mailbox?",
        "Focus sur quel dossier ?";
    NameOfTheNewFolder:
        "Name of the new folder?",
        "Nom du nouveau dossier ?";
    APathLikeWorkClients:
        "a path like Work/Clients",
        "un chemin comme Travail/Clients";
    RenameWhichFolder:
        "Rename which folder?",
        "Renommer quel dossier ?";
    DeleteWhichFolder:
        "Delete which folder?",
        "Supprimer quel dossier ?";
    PinWhichMailbox:
        "Pin which mailbox?",
        "Épingler quel dossier ?";
    UnpinWhichMailbox:
        "Unpin which mailbox?",
        "Détacher quel dossier ?";
    HideWhichMailbox:
        "Hide which mailbox?",
        "Masquer quel dossier ?";
    ShowWhichHiddenMailbox:
        "Show which hidden mailbox?",
        "Afficher quel dossier masqué ?";
    MarkWhichMailboxRead:
        "Mark which mailbox read?",
        "Marquer quel dossier comme lu ?";
    ShowOnlyWhichThreads:
        "Show only which threads?",
        "Afficher seulement quelles discussions ?";
    All:
        "all",
        "toutes";
    CheckForUpdates:
        "Check for updates?",
        "Rechercher des mises à jour ?";
    OnOrOffThenHoursBetweenChecks:
        "on or off, then hours between checks: on 24",
        "on ou off, puis les heures entre deux vérifications : on 24";
    ApproveWhichSendRequest:
        "Approve which send request?",
        "Approuver quelle demande d’envoi ?";
    RejectWhichSendRequest:
        "Reject which send request?",
        "Rejeter quelle demande d’envoi ?";
    TypeThenEnter:
        "Type, then Enter",
        "Saisissez, puis Entrée";
    CannotBeUndone:
        "cannot be undone",
        "irréversible";
    Filter:
        "Filter…",
        "Filtrer…";
    TypeACommandOrSearch:
        "Type a command or search…",
        "Saisissez une commande ou une recherche…";
    Recent:
        "Recent",
        "Récents";
    Threads:
        "Threads",
        "Discussions";
    Date:
        "Date",
        "Date";
    Mailbox:
        "Mailbox",
        "Dossier";
    Size:
        "Size",
        "Taille";
    Flags:
        "Flags",
        "Indicateurs";
    Yes:
        "yes",
        "oui";
    No:
        "no",
        "non";
    None:
        "none",
        "aucune";
    OpenConversation:
        "Open conversation",
        "Ouvrir la discussion";
    LastMessage:
        "Last message",
        "Dernier message";
    Query:
        "Query",
        "Requête";
    NoMailboxes:
        "No mailboxes",
        "Aucun dossier";
    NoContacts:
        "No contacts",
        "Aucun contact";
    NoPendingChanges:
        "No pending changes",
        "Aucune modification en attente";
    NoSendRequestsX:
        "No send requests",
        "Aucune demande d’envoi";
    Testing:
        "Testing…",
        "Test…";
    Address:
        "Address",
        "Adresse";
    Domain:
        "Domain",
        "Domaine";
    FoundIn:
        "Found in",
        "Trouvé dans";
    State:
        "State",
        "État";
    Value:
        "Value",
        "Valeur";
    Daily:
        "Daily",
        "Quotidien";
    Weekly:
        "Weekly",
        "Hebdomadaire";
    Monthly:
        "Monthly",
        "Mensuel";
    CheckNow:
        "Check now",
        "Vérifier maintenant";
    Download:
        "Download",
        "Télécharger";
    Install:
        "Install…",
        "Installer…";
    SkipThisVersion:
        "Skip this version",
        "Ignorer cette version";
    ReleaseNotesX:
        "Release notes",
        "Notes de version";
    ThisBuildDoesNotCheckForUpdates:
        "This build does not check for updates",
        "Cette version ne vérifie pas les mises à jour";
    DailIsUpToDate:
        "dail is up to date",
        "dail est à jour";
    NotCheckedYet:
        "Not checked yet",
        "Pas encore vérifié";
    CheckingForUpdates:
        "Checking for updates…",
        "Recherche de mises à jour…";
    RestartToUpdate:
        "Restart to update",
        "Redémarrer pour mettre à jour";
    Checked:
        "checked",
        "cochée";
    UnreadX:
        "unread",
        "non lu";
    StarredX:
        "starred",
        "suivi";
    PinnedX:
        "pinned",
        "épinglé";
    WithAttachments:
        "with attachments",
        "avec pièces jointes";
    Unnamed:
        "(unnamed)",
        "(sans nom)";
    AccountX:
        "account",
        "compte";
    AndMore:
        "and {n} more",
        " et {n} autres";
    NewMessages:
        "{n} new messages",
        "{n} nouveaux messages";
    MessagesCouldNotBeSent:
        "{n} messages could not be sent",
        "{n} messages n’ont pas pu être envoyés";
    ToX:
        "To {to}: {subject}",
        "À {to} : {subject}";
    AgentsWantToSendMessages:
        "Agents want to send {n} messages",
        "Des agents veulent envoyer {n} messages";
    SendingInS:
        "sending in {n} s",
        "envoi dans {n} s";
    InS:
        "in {n}s",
        "dans {n} s";
    InM:
        "in {n}m",
        "dans {n} min";
    InH:
        "in {n}h",
        "dans {n} h";
    MAgo:
        "{n}m ago",
        "il y a {n} min";
    HAgo:
        "{n}h ago",
        "il y a {n} h";
    DAgo:
        "{n}d ago",
        "il y a {n} j";
    SendingInSX:
        "Sending in {n} s",
        "Envoi dans {n} s";
    RemoveAndItsPassword:
        "Remove {email} and its password",
        "Supprimer {email} et son mot de passe";
    InstallDail:
        "Install dail {version}",
        "Installer dail {version}";
    NewFolderIn:
        "New folder in {path}",
        "Nouveau dossier dans {path}";
    NowX:
        "now {now}",
        "actuellement {now}";
    DeleteX:
        "Delete {path}",
        "Supprimer {path}";
    MarkAllUnreadMessagesReadUndoRestores:
        "Mark all {n} unread messages read? Undo restores only the cached ones.",
        "Marquer les {n} messages non lus comme lus ? L’annulation ne restaure que ceux en cache.";
    EmptyTrashOfForever:
        "Delete the {n} messages in the Trash of {email} forever? This cannot be undone.",
        "Supprimer définitivement les {n} messages de la corbeille de {email} ? C’est irréversible.";
    EmptyTrash:
        "Empty Trash",
        "Vider la corbeille";
    EmptyTrashN:
        "Empty Trash ({n})",
        "Vider la corbeille ({n})";
    EmptyTrashOfWhichAccount:
        "Empty the Trash of which account?",
        "Vider la corbeille de quel compte ?";
    EmptyMailboxOp:
        "empty {mailbox}",
        "vider {mailbox}";
    MarkMessagesRead:
        "Mark {n} messages read",
        "Marquer {n} messages comme lus";
    NewValueFor:
        "New value for {key}",
        "Nouvelle valeur pour {key}";
    NowXX:
        "{allowed}, now {current}",
        "{allowed}, actuellement {current}";
    GoTo:
        "Go to {name}",
        "Aller à {name}";
    Use:
        "Use “{query}”",
        "Utiliser « {query} »";
    SearchMailFor:
        "Search mail for “{query}”",
        "Rechercher « {query} » dans le courrier";
    UnreadXX:
        "{unread} unread · {total}",
        "{unread} non lus · {total}";
    Attempts:
        "{state} · {n} attempts",
        "{state} · {n} tentatives";
    ToName:
        "To {to}",
        "Pour {to}";
    SignedIn:
        "Signed in {age}",
        "Connecté {age}";
    DailIsAvailable:
        "dail {version} is available",
        "dail {version} est disponible";
    DownloadingDail:
        "Downloading dail {version}…",
        "Téléchargement de dail {version}…";
    DailIsDownloadedAndVerified:
        "dail {version} is downloaded and verified",
        "dail {version} est téléchargé et vérifié";
    InstallingDail:
        "Installing dail {version}…",
        "Installation de dail {version}…";
    DailIsInstalledRestartDailToUse:
        "dail {version} is installed: restart dail to use it",
        "dail {version} est installé : redémarrez dail pour l’utiliser";
    UpdateFailed:
        "Update failed: {reason}",
        "Échec de la mise à jour : {reason}";
    CheckedX:
        "checked {age}",
        "vérifié {age}";
    Update:
        "Update {version}",
        "Mettre à jour vers {version}";
    EveryH:
        "every {n} h",
        "toutes les {n} h";
    UnreadXXX:
        "{label}, {n} unread",
        "{label}, {n} non lus";
    UpdateQuestion:
        "Check for new versions of dail?",
        "Rechercher les nouvelles versions de dail ?";
    UpdateQuestionDetail:
        "Once a day dail asks GitHub for the signed release list. Nothing about you or your mail is sent. You can change this in Settings.",
        "Une fois par jour, dail demande à GitHub la liste signée des versions. Rien sur vous ou votre courrier n’est envoyé. Vous pouvez changer cela dans les paramètres.";
    LanguageSetting:
        "Language",
        "Langue";
    LanguageHint:
        "The language of the app",
        "La langue de l’application";
    AddAnAccountInDailBeforeSharing:
        "Add an account in dail before sharing to it.",
        "Ajoutez un compte dans dail avant d’y partager.";
    RestorePanes:
        "Restore panes",
        "Restaurer les panneaux";
    MaximizeMessage:
        "Maximize message",
        "Agrandir le message";
    About:
        "About",
        "À propos";
    ShowFolderNames:
        "Show folder names",
        "Afficher les noms des dossiers";
    IconsOnly:
        "Icons only",
        "Icônes seules";
    NoAccountsYetAddOneToRead:
        "No accounts yet. Add one to read your mail here.",
        "Aucun compte pour l’instant. Ajoutez-en un pour lire votre courrier ici.";
    PickAMailboxFromTheMenu:
        "Pick a mailbox from the menu.",
        "Choisissez un dossier dans le menu.";
    ForwardedMessage:
        "Forwarded message",
        "Message transféré";
    QuotedMessage:
        "Quoted message",
        "Message cité";
    Check:
        "Check",
        "Vérifier";
    Open:
        "Open",
        "Ouvrir";
    Skip:
        "Skip",
        "Ignorer";
    NewMail:
        "New mail",
        "Nouveau courrier";
    AMessageWasNotSent:
        "A message was not sent",
        "Un message n’a pas été envoyé";
    NewMailArrivesAsItComes:
        "New mail arrives as it comes",
        "Le courrier arrive au fil de l’eau";
    Stop:
        "Stop",
        "Arrêter";
    Battery:
        "Battery",
        "Batterie";
    ConnectingToYourInboxes:
        "Connecting to your inboxes",
        "Connexion à vos comptes";
    Watching1Inbox:
        "Watching 1 inbox",
        "Surveillance d’un dossier";
    QuotedTextX:
        "Quoted text",
        "Texte cité";
    SignatureX:
        "Signature",
        "Signature";
    Move:
        "Move…",
        "Déplacer…";
    CloseMessage:
        "Close message",
        "Fermer le message";
    PreviousConversationX:
        "Previous conversation",
        "Discussion précédente";
    NextConversationX:
        "Next conversation",
        "Discussion suivante";
    Copy:
        "Copy",
        "Copier";
    LinkCopied:
        "Link copied.",
        "Lien copié.";
    Save:
        "Save",
        "Enregistrer";
    DailDoesNotShowThisKindOf:
        "dail does not show this kind of file. Save it to open it in another app.",
        "dail n’affiche pas ce type de fichier. Enregistrez-le pour l’ouvrir dans une autre application.";
    ThisPictureCouldNotBeShown:
        "This picture could not be shown.",
        "Cette image n’a pas pu être affichée.";
    ThisPageCouldNotBeShown:
        "This page could not be shown.",
        "Cette page n’a pas pu être affichée.";
    TheFileCouldNotBeRead:
        "The file could not be read.",
        "Le fichier n’a pas pu être lu.";
    TheMessageIsNotDownloadedYet:
        "The message is not downloaded yet.",
        "Le message n’est pas encore téléchargé.";
    On:
        "On",
        "Activé";
    CozyX:
        "Cozy",
        "Détendue";
    CompactX:
        "Compact",
        "Compacte";
    OnlyWhileOpen:
        "Only while open",
        "Seulement ouverte";
    Periodic:
        "Periodic",
        "Périodique";
    Push:
        "Push",
        "Push";
    CheckTheInboxEvery:
        "Check the inbox every",
        "Relever le dossier toutes les";
    MessagesFetchedFirst:
        "Messages fetched first",
        "Messages récupérés d’abord";
    ListDensity:
        "List density",
        "Densité de la liste";
    MailChecksInTheBackground:
        "Mail checks in the background",
        "Relève en arrière-plan";
    CheckEvery:
        "Check every",
        "Vérifier toutes les";
    TerminalColours:
        "Terminal colours",
        "Couleurs du terminal";
    MarkAsReadAfter:
        "Mark as read after",
        "Marquer comme lu après";
    RefreshAMailboxOnOpenAfter:
        "Refresh a mailbox on open after",
        "Actualiser un dossier à l’ouverture après";
    SyncEveryFolderEvery:
        "Sync every folder every",
        "Synchroniser tous les dossiers toutes les";
    AfterArchiving:
        "After archiving",
        "Après l’archivage";
    HowLongASendCanStillBe:
        "How long a send can still be cancelled.",
        "Le temps pendant lequel un envoi peut être annulé.";
    WhileTheAppIsOpen:
        "While the app is open.",
        "Tant que l’application est ouverte.";
    NewMailAndFailedSendsWhileDail:
        "New mail and failed sends, while dail is off screen.",
        "Nouveau courrier et envois en échec, quand dail n’est pas à l’écran.";
    PerMailboxOnItsFirstSync:
        "Per mailbox, on its first sync.",
        "Par dossier, à sa première synchronisation.";
    Mode:
        "Mode",
        "Mode";
    SystemFollowsAndroidSDarkTheme:
        "System follows Android's dark theme.",
        "Système suit le thème sombre d’Android.";
    TextSizeFollowsAndroidSFontSize:
        "Text size follows Android's font size.",
        "La taille du texte suit celle d’Android.";
    ChangeAKey:
        "Change a key…",
        "Changer une touche…";
    HardwareKeyboard:
        "Hardware keyboard",
        "Clavier physique";
    ForAKeyboardOnATabletOr:
        "For a keyboard on a tablet or a Chromebook: the desktop's keys.",
        "Pour un clavier sur tablette ou Chromebook : les touches du bureau.";
    Bindings:
        "Bindings",
        "Raccourcis";
    BatteryOptimization:
        "Battery optimization",
        "Optimisation de la batterie";
    NotificationSettings:
        "Notification settings",
        "Paramètres de notification";
    SystemSettings:
        "System settings",
        "Paramètres système";
    BackgroundNotifications:
        "Background & notifications",
        "Arrière-plan et notifications";
    NotTestedX:
        "Not tested",
        "Non testé";
    ConnectedX:
        "Connected",
        "Connecté";
    Remove:
        "Remove",
        "Retirer";
    AnotherAccount:
        "Another account",
        "Un autre compte";
    GmailHostingerMailOrAnyIMAPAnd:
        "Gmail, Hostinger Mail or any IMAP and SMTP server.",
        "Gmail, Hostinger Mail ou tout serveur IMAP et SMTP.";
    EnterTheServersYourselfOrFindThem:
        "Enter the servers yourself, or find them.",
        "Saisissez les serveurs vous-même, ou trouvez-les.";
    ShownInTheDrawer:
        "shown in the drawer",
        "affiché dans le tiroir";
    View:
        "View",
        "Vue";
    SearchingTheServer:
        "Searching the server…",
        "Recherche sur le serveur…";
    Selected:
        "{n} selected",
        "{n} sélectionnés";
    NotSentX:
        "Not sent: “{subject}”: {error}",
        "Non envoyé : « {subject} » : {error}";
    NewMailInConversations:
        "New mail in {n} conversations",
        "Nouveau courrier dans {n} discussions";
    WatchingInboxes:
        "Watching {n} inboxes",
        "Surveillance de {n} dossiers";
    HideImagesFrom:
        "Hide images from {who}",
        "Masquer les images de {who}";
    ShowImagesFrom:
        "Show images from {who}",
        "Afficher les images de {who}";
    SaveAllX:
        "Save all ({n})",
        "Tout enregistrer ({n})";
    Failed:
        "Failed: {error}",
        "Échec : {error}";
    TheServerSearchFailed:
        "The server search failed: {error}",
        "La recherche sur le serveur a échoué : {error}";
    NativeSaved:
        "Saved.",
        "Enregistré.";
    NativeSavedN:
        "Saved {n} files.",
        "{n} fichiers enregistrés.";
    NativeSavedPart:
        "Saved {n} of {m} files.",
        "{n} fichiers sur {m} enregistrés.";
    NativeNoLinkApp:
        "No app can open this link.",
        "Aucune application ne peut ouvrir ce lien.";
    NativeNoSettings:
        "This phone has no such settings screen.",
        "Ce téléphone n’a pas cet écran de paramètres.";
    NativeRelease:
        "Release to open",
        "Relâchez pour ouvrir";
    NativeSave:
        "Save {name}",
        "Enregistrer {name}";
    NativeOpenLink:
        "Open this link?",
        "Ouvrir ce lien ?";
    NativePage:
        "Page {n} of {m}",
        "Page {n} sur {m}";
    NativeSelect:
        "Select",
        "Sélectionner";
    NativeFailed:
        "Failed",
        "Échec";
    NativePassed:
        "Passed",
        "Réussi";
    NativePending:
        "Pending",
        "En attente";
    AboutDail:
        "About dail",
        "À propos de dail";
    AboutDailVersion:
        "About dail ({version})",
        "À propos de dail ({version})";
    AboutDesktopText:
        "Keyboard-first email client that agents can drive. Licence: {licence}.",
        "Client de messagerie piloté au clavier, que des agents peuvent commander. Licence : {licence}.";
    AboutMobileText:
        "Email client for IMAP and SMTP accounts ({licence}). The app bundles these typefaces, icons, PDF fonts, Rust crates and Android libraries under their licences (SPDX identifiers: name, version, licence).",
        "Client de messagerie pour comptes IMAP et SMTP ({licence}). L’application intègre ces polices, icônes, polices PDF, crates Rust et bibliothèques Android sous leurs licences (identifiants SPDX : nom, version, licence).";
    SourceCode:
        "Source code",
        "Code source";
    Releases:
        "Releases",
        "Versions";
    RustCrates:
        "Rust crates",
        "Crates Rust";
    AndroidLibraries:
        "Android libraries",
        "Bibliothèques Android";
    PdfFontsAndCmaps:
        "PDF fonts and CMaps",
        "Polices PDF et CMaps";
    PdfCraftViewer:
        "PdfCraft for Android (adapted viewer)",
        "PdfCraft pour Android (visionneuse adaptée)";
    SharedRefused:
        "{n} shared files could not be attached: only files shared by other apps can.",
        "{n} fichiers partagés n’ont pas pu être joints : seuls les fichiers partagés par d’autres applications peuvent l’être.";
    SharedRefusedOne:
        "1 shared file could not be attached: only files shared by other apps can.",
        "1 fichier partagé n’a pas pu être joint : seuls les fichiers partagés par d’autres applications peuvent l’être.";
    LoginFailedColon:
        "The login failed: {error}",
        "La connexion a échoué : {error}";
    HintUp:
        "up",
        "haut";
    HintDown:
        "down",
        "bas";
    WhichKeys:
        "Which keys?",
        "Quels raccourcis ?";
    GmailOrVim:
        "gmail or vim",
        "gmail ou vim";
    FilterHint:
        "unread, starred, attachments (empty: all); now: {now}",
        "non lu, suivi, pièces jointes (vide : tout) ; actuellement : {now}";
    AttachmentsX:
        "attachments",
        "pièces jointes";
    ReadFlag:
        "read",
        "lu";
    AnsweredFlag:
        "answered",
        "répondu";
    WindowN:
        " · window {n}",
        " · fenêtre {n}";
    SetFlagOn:
        "set {flag} on {n} message(s)",
        "activer {flag} sur {n} message(s)";
    ClearFlagOn:
        "clear {flag} on {n} message(s)",
        "retirer {flag} sur {n} message(s)";
    SetFlagAll:
        "set {flag} on all of {mailbox}",
        "activer {flag} sur tout {mailbox}";
    ClearFlagAll:
        "clear {flag} on all of {mailbox}",
        "retirer {flag} sur tout {mailbox}";
    NoPreviewFor:
        "no preview for {type}",
        "pas d’aperçu pour {type}";
    ImageNotInMessage:
        "the image is not in the message",
        "l’image n’est pas dans le message";
    NotAnImage:
        "{type} is not an image",
        "{type} n’est pas une image";
    AttachmentNotStored:
        "the attachment is not in the local store",
        "la pièce jointe n’est pas dans le stockage local";
    CouldNotOpenBrowser:
        "Could not open the browser: {error}",
        "Impossible d’ouvrir le navigateur : {error}";
    MessageGone:
        "The message is not in the local store any more.",
        "Le message n’est plus dans le stockage local.";
    AttachmentGone:
        "The attachment is not in the local store any more.",
        "La pièce jointe n’est plus dans le stockage local.";
    CouldNotWriteOriginal:
        "Could not write the original: {error}",
        "Impossible d’écrire l’original : {error}";
    CouldNotWriteAttachment:
        "Could not write the attachment: {error}",
        "Impossible d’écrire la pièce jointe : {error}";
    NoEditor:
        "no editor",
        "aucun éditeur";
    EditorExited:
        "{program} exited with {status}",
        "{program} s’est terminé avec {status}";
    TerminalCantDraw:
        "the terminal can't be drawn: {error}",
        "impossible de dessiner le terminal : {error}";
    InputClosed:
        "input closed",
        "entrée fermée";
    TerminalWentAway:
        "the terminal went away",
        "le terminal a disparu";
    ReceivedSignal:
        "received {name}",
        "{name} reçu";
    ConsoleClose:
        "console close",
        "fermeture de la console";
    UnexpectedReceipt:
        "unexpected receipt: {error}",
        "accusé de réception inattendu : {error}";
    ToNameLower:
        "to {name}",
        "à {name}";
}

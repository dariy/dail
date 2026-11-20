//! The strings of every language. A row is a key with its English, French, Spanish and Romanian
//! templates, so a key cannot miss a language; the doc of a key is its English text.

/// Declares [`Key`], [`Key::ALL`] and a function per language from one table.
macro_rules! strings {
    ($($key:ident: $english:literal, $french:literal, $spanish:literal, $romanian:literal;)*) => {
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

        /// The Spanish table.
        pub const fn spanish(key: Key) -> &'static str {
            match key {
                $(Key::$key => $spanish,)*
            }
        }

        /// The Romanian table.
        pub const fn romanian(key: Key) -> &'static str {
            match key {
                $(Key::$key => $romanian,)*
            }
        }
    };
}

strings! {
    StoreFailed:
        "local store failed",
        "échec du stockage local",
        "error del almacenamiento local",
        "eroare la stocarea locală";
    Quarantined:
        "{count} saved items could not be read",
        "{count} éléments enregistrés n’ont pas pu être lus",
        "{count} elementos guardados no se pudieron leer",
        "{count} elemente salvate nu au putut fi citite";
    QuarantinedHint:
        "They are kept aside in the profile, not deleted: a later version of dail may read them. Report it with the version you updated from.",
        "Ils sont mis de côté dans le profil, pas supprimés : une version ultérieure de dail pourra peut-être les lire. Signalez-le en indiquant la version précédente.",
        "Se guardan aparte en el perfil, no se eliminan: una versión posterior de dail podría leerlos. Infórmalo indicando la versión desde la que actualizaste.",
        "Sunt păstrate deoparte în profil, nu șterse: o versiune ulterioară de dail le-ar putea citi. Raportați problema, menționând versiunea de la care ați actualizat.";
    KeyringFailed:
        "keyring failed",
        "échec du trousseau",
        "error del llavero",
        "eroare la portofelul de chei";
    KeyringHint:
        "The system keyring (Secret Service on Linux, Credential Manager on Windows) is locked or missing: unlock it, or start a keyring service.",
        "Le trousseau du système (Secret Service sous Linux, Gestionnaire d’identifiants sous Windows) est verrouillé ou absent : déverrouillez-le ou démarrez un service de trousseau.",
        "El llavero del sistema (Secret Service en Linux, Administrador de credenciales en Windows) está bloqueado o no existe: desbloquéalo o inicia un servicio de llavero.",
        "Portofelul de chei al sistemului (Secret Service pe Linux, Credential Manager pe Windows) este blocat sau lipsește: deblocați-l sau porniți un serviciu de chei.";
    LoginFailed:
        "{email}: login failed",
        "{email} : échec de la connexion",
        "{email}: error al iniciar sesión",
        "{email}: autentificarea a eșuat";
    SyncFailed:
        "{email}: sync failed",
        "{email} : échec de la synchronisation",
        "{email}: error de sincronización",
        "{email}: sincronizarea a eșuat";
    FixPassword:
        "re-enter password",
        "saisir à nouveau le mot de passe",
        "volver a escribir la contraseña",
        "reintroduceți parola";
    FixRetry:
        "retry",
        "réessayer",
        "reintentar",
        "reîncercați";
    FixTest:
        "test again",
        "tester à nouveau",
        "probar de nuevo",
        "testați din nou";
    HintPasswordGmail:
        "Gmail refused the password. It needs an App Password (Google account → Security → 2-Step Verification → App passwords), not your Google password.",
        "Gmail a refusé le mot de passe. Il faut un mot de passe d’application (compte Google → Sécurité → Validation en deux étapes → Mots de passe des applications), pas votre mot de passe Google.",
        "Gmail rechazó la contraseña. Necesita una contraseña de aplicación (cuenta de Google → Seguridad → Verificación en dos pasos → Contraseñas de aplicaciones), no tu contraseña de Google.",
        "Gmail a refuzat parola. Este nevoie de o parolă pentru aplicații (Cont Google → Securitate → Verificare în doi pași → Parole pentru aplicații), nu de parola contului Google.";
    HintPassword:
        "The server refused the password (or none is stored). Enter it again; if you changed it on the web, use the new one.",
        "Le serveur a refusé le mot de passe (ou aucun n’est enregistré). Saisissez-le à nouveau ; si vous l’avez changé sur le web, utilisez le nouveau.",
        "El servidor rechazó la contraseña (o no hay ninguna guardada). Escríbela de nuevo; si la cambiaste en la web, usa la nueva.",
        "Serverul a refuzat parola (sau nu este salvată niciuna). Introduceți-o din nou; dacă ați schimbat-o pe web, folosiți-o pe cea nouă.";
    HintQuota:
        "The mailbox is full. Delete or archive mail and empty the Trash, then retry.",
        "La boîte aux lettres est pleine. Supprimez ou archivez du courrier et videz la corbeille, puis réessayez.",
        "El buzón está lleno. Elimina o archiva correo y vacía la papelera; luego reintenta.",
        "Căsuța poștală este plină. Ștergeți sau arhivați mesaje și goliți coșul de gunoi, apoi reîncercați.";
    HintTlsCiphers:
        "The server offers only TLS versions or ciphers dail does not support (dail needs TLS 1.2 or 1.3 with ChaCha20-Poly1305 or AES-GCM). Ask the provider; nothing in dail's settings changes this.",
        "Le serveur ne propose que des versions ou des chiffrements TLS que dail ne prend pas en charge (dail exige TLS 1.2 ou 1.3 avec ChaCha20-Poly1305 ou AES-GCM). Contactez le fournisseur ; aucun réglage de dail n’y change rien.",
        "El servidor solo ofrece versiones o cifrados TLS que dail no admite (dail necesita TLS 1.2 o 1.3 con ChaCha20-Poly1305 o AES-GCM). Consulta al proveedor; ningún ajuste de dail lo cambia.",
        "Serverul oferă doar versiuni sau cifruri TLS pe care dail nu le acceptă (dail are nevoie de TLS 1.2 sau 1.3 cu ChaCha20-Poly1305 ori AES-GCM). Contactați furnizorul; nicio setare din dail nu schimbă asta.";
    HintCertificate:
        "The server's certificate is not trusted: expired, self-signed or for another name. Check the server name. If you run the server and it uses its own certificate, trust it with “trust…”.",
        "Le certificat du serveur n’est pas fiable : expiré, auto-signé ou émis pour un autre nom. Vérifiez le nom du serveur. Si le serveur est le vôtre et utilise son propre certificat, acceptez-le avec « faire confiance… ».",
        "El certificado del servidor no es de confianza: caducado, autofirmado o emitido para otro nombre. Comprueba el nombre del servidor. Si el servidor es tuyo y usa su propio certificado, puedes confiar en él con «confiar…».",
        "Certificatul serverului nu este de încredere: expirat, autosemnat sau emis pentru alt nume. Verificați numele serverului. Dacă serverul este al dvs. și folosește propriul certificat, îl puteți accepta cu „acceptați…”.";
    HintNetwork:
        "The server could not be reached. Check the connection and the server name; syncing tries again by itself.",
        "Le serveur est injoignable. Vérifiez la connexion et le nom du serveur ; la synchronisation réessaie d’elle-même.",
        "No se pudo conectar con el servidor. Comprueba la conexión y el nombre del servidor; la sincronización vuelve a intentarlo sola.",
        "Serverul nu a putut fi contactat. Verificați conexiunea și numele serverului; sincronizarea reîncearcă singură.";
    Syncing:
        "⟳ syncing…",
        "⟳ synchronisation…",
        "⟳ sincronizando…",
        "⟳ se sincronizează…";
    Synced:
        "synced {age}",
        "synchronisé {age}",
        "sincronizado {age}",
        "sincronizat {age}";
    NoAccounts:
        "no accounts",
        "aucun compte",
        "sin cuentas",
        "niciun cont";
    NotSynced:
        "not synced",
        "non synchronisé",
        "sin sincronizar",
        "nesincronizat";
    NextPoll:
        "next {when}",
        "prochaine {when}",
        "próxima {when}",
        "următoarea {when}";
    Pending:
        "{pending} pending",
        "{pending} en attente",
        "{pending} pendientes",
        "{pending} în așteptare";
    PendingFailed:
        "{pending} pending · {failed} failed",
        "{pending} en attente · {failed} en échec",
        "{pending} pendientes · {failed} con error",
        "{pending} în așteptare · {failed} eșuate";
    CollapseSidebar:
        "Collapse sidebar (Ctrl+B)",
        "Réduire la barre latérale (Ctrl+B)",
        "Contraer la barra lateral (Ctrl+B)",
        "Restrângeți bara laterală (Ctrl+B)";
    ExpandSidebar:
        "Expand sidebar (Ctrl+B)",
        "Développer la barre latérale (Ctrl+B)",
        "Expandir la barra lateral (Ctrl+B)",
        "Extindeți bara laterală (Ctrl+B)";
    Compose:
        "Compose",
        "Rédiger",
        "Redactar",
        "Scrieți";
    ComposeKey:
        "Compose (C)",
        "Rédiger (C)",
        "Redactar (C)",
        "Scrieți (C)";
    Settings:
        "Settings",
        "Paramètres",
        "Ajustes",
        "Setări";
    ShowAllFolders:
        "Show all folders",
        "Afficher tous les dossiers",
        "Mostrar todas las carpetas",
        "Afișați toate dosarele";
    AllInboxes:
        "All inboxes",
        "Dossiers unifiés",
        "Todas las bandejas de entrada",
        "Toate căsuțele de intrare";
    Favourites:
        "Favourites",
        "Dossiers préférés",
        "Favoritos",
        "Favorite";
    HideHiddenFolders:
        "Hide hidden folders",
        "Masquer les dossiers cachés",
        "Ocultar las carpetas ocultas",
        "Ascundeți dosarele ascunse";
    NoAccountsYet:
        "No accounts yet.",
        "Aucun compte pour l’instant.",
        "Aún no hay cuentas.",
        "Încă nu există conturi.";
    AddAccount:
        "Add account",
        "Ajouter un compte",
        "Añadir cuenta",
        "Adăugați un cont";
    AddAccountPlus:
        "+ Add account",
        "+ Ajouter un compte",
        "+ Añadir cuenta",
        "+ Adăugați un cont";
    SyncingNow:
        "Syncing",
        "Synchronisation",
        "Sincronizando",
        "Se sincronizează";
    Offline:
        "offline",
        "hors ligne",
        "sin conexión",
        "offline";
    NewSubfolder:
        "New subfolder…",
        "Nouveau sous-dossier…",
        "Nueva subcarpeta…",
        "Subdosar nou…";
    Rename:
        "Rename…",
        "Renommer…",
        "Cambiar nombre…",
        "Redenumiți…";
    UnpinFavourites:
        "Unpin from Favourites",
        "Retirer des dossiers préférés",
        "Quitar de Favoritos",
        "Eliminați din Favorite";
    PinFavourites:
        "Pin to Favourites",
        "Ajouter aux dossiers préférés",
        "Fijar en Favoritos",
        "Fixați în Favorite";
    ShowFolder:
        "Show folder",
        "Afficher le dossier",
        "Mostrar la carpeta",
        "Afișați dosarul";
    HideFolder:
        "Hide folder",
        "Masquer le dossier",
        "Ocultar la carpeta",
        "Ascundeți dosarul";
    FocusMailbox:
        "Focus this mailbox",
        "Se concentrer sur ce dossier",
        "Centrarse en esta carpeta",
        "Concentrați-vă pe acest dosar";
    MarkAllRead:
        "Mark all as read",
        "Tout marquer comme lu",
        "Marcar todo como leído",
        "Marcați tot ca citit";
    Delete:
        "Delete…",
        "Supprimer…",
        "Eliminar…",
        "Ștergeți…";
    SearchOrRun:
        "Search or run a command",
        "Rechercher ou lancer une commande",
        "Busca o ejecuta un comando",
        "Căutați sau rulați o comandă";
    MoveHint:
        "j / k to move",
        "j / k pour se déplacer",
        "j / k para moverte",
        "j / k pentru deplasare";
    SyncNow:
        "Sync now",
        "Synchroniser",
        "Sincronizar ahora",
        "Sincronizați acum";
    SearchingServer:
        "searching the server…",
        "recherche sur le serveur…",
        "buscando en el servidor…",
        "se caută pe server…";
    RetryFailed:
        "Retry failed changes",
        "Réessayer les modifications en échec",
        "Reintentar los cambios fallidos",
        "Reîncercați modificările eșuate";
    Undo:
        "Undo",
        "Annuler",
        "Deshacer",
        "Anulați";
    ReadOnlyCaption:
        "Read-only · untrusted data",
        "Lecture seule · données non fiables",
        "Solo lectura · datos no fiables",
        "Doar citire · date nesigure";
    RawData:
        "Raw data",
        "Données brutes",
        "Datos sin procesar",
        "Date brute";
    ReadOnlyResult:
        "Read-only result. Email content in it is untrusted data.",
        "Résultat en lecture seule. Le contenu des e-mails y est une donnée non fiable.",
        "Resultado de solo lectura. El contenido de correo que incluye son datos no fiables.",
        "Rezultat doar pentru citire. Conținutul de e-mail din el reprezintă date nesigure.";
    CheckUpdates:
        "Check for updates",
        "Rechercher des mises à jour",
        "Buscar actualizaciones",
        "Căutați actualizări";
    NoThanks:
        "No thanks",
        "Non merci",
        "No, gracias",
        "Nu, mulțumesc";
    NotSaved:
        "NOT SAVED",
        "NON ENREGISTRÉ",
        "SIN GUARDAR",
        "NESALVAT";
    CloseEsc:
        "Close (Esc)",
        "Fermer (Échap)",
        "Cerrar (Esc)",
        "Închideți (Esc)";
    From:
        "From",
        "De",
        "De",
        "De la";
    CcBcc:
        "Cc/Bcc",
        "Copie à/Copie cachée à",
        "CC/CCO",
        "Cc/Bcc";
    Subject:
        "Subject",
        "Objet",
        "Asunto",
        "Subiect";
    WriteMessage:
        "Write your message…",
        "Écrivez votre message…",
        "Escribe tu mensaje…",
        "Scrieți mesajul…";
    OriginalBelow:
        "The original is added below when sent",
        "L’original est ajouté dessous à l’envoi",
        "El original se añade debajo al enviar",
        "Originalul este adăugat dedesubt la trimitere";
    CancelSend:
        "Cancel send",
        "Annuler l’envoi",
        "Cancelar el envío",
        "Anulați trimiterea";
    AttachFiles:
        "Attach files…",
        "Joindre des fichiers…",
        "Adjuntar archivos…",
        "Atașați fișiere…";
    Send:
        "Send",
        "Envoyer",
        "Enviar",
        "Trimiteți";
    Discard:
        "Discard",
        "Abandonner",
        "Descartar",
        "Renunțați";
    NameExample:
        "name@example.com",
        "nom@example.com",
        "nombre@example.com",
        "nume@example.com";
    AddRecipient:
        "Add this recipient",
        "Ajouter ce destinataire",
        "Añadir este destinatario",
        "Adăugați acest destinatar";
    Checking:
        "checking…",
        "vérification…",
        "comprobando…",
        "se verifică…";
    AttachFilesTitle:
        "Attach files",
        "Joindre des fichiers",
        "Adjuntar archivos",
        "Atașați fișiere";
    DropToAttach:
        "Drop the files to attach them",
        "Déposez les fichiers pour les joindre",
        "Suelta los archivos para adjuntarlos",
        "Plasați fișierele aici pentru a le atașa";
    NoMatches:
        "No matching conversations.",
        "Aucune discussion ne correspond.",
        "Ninguna conversación coincide.",
        "Nicio conversație nu se potrivește.";
    NoConversations:
        "No conversations here.",
        "Aucune discussion ici.",
        "No hay conversaciones aquí.",
        "Nu există conversații aici.";
    PickMailbox:
        "Pick a mailbox.",
        "Choisissez un dossier.",
        "Elige una carpeta.",
        "Alegeți un dosar.";
    Focus:
        "Focus",
        "Focus",
        "Enfoque",
        "Concentrare";
    Clear:
        "Clear",
        "Effacer",
        "Borrar",
        "Ștergeți";
    SelectAll:
        "Select all",
        "Tout sélectionner",
        "Seleccionar todo",
        "Selectați tot";
    Unread:
        "Unread",
        "Non lus",
        "No leídos",
        "Necitite";
    Starred:
        "Starred",
        "Suivis",
        "Destacados",
        "Cu stea";
    Attachments:
        "Attachments",
        "Pièces jointes",
        "Archivos adjuntos",
        "Atașamente";
    Refreshing:
        "Refreshing",
        "Actualisation",
        "Actualizando",
        "Se actualizează";
    Refresh:
        "Refresh",
        "Actualiser",
        "Actualizar",
        "Actualizați";
    Archive:
        "Archive",
        "Archiver",
        "Archivar",
        "Arhivați";
    Trash:
        "Trash",
        "Corbeille",
        "Papelera",
        "Coș de gunoi";
    MarkRead:
        "Mark read",
        "Marquer comme lu",
        "Marcar como leído",
        "Marcați ca citit";
    MarkUnread:
        "Mark unread",
        "Marquer comme non lu",
        "Marcar como no leído",
        "Marcați ca necitit";
    Star:
        "Star",
        "Ajouter un suivi",
        "Destacar",
        "Marcați cu stea";
    Unstar:
        "Unstar",
        "Retirer le suivi",
        "Quitar destacado",
        "Eliminați steaua";
    UncheckAll:
        "Uncheck all",
        "Tout décocher",
        "Desmarcar todo",
        "Debifați tot";
    Unpin:
        "Unpin",
        "Détacher",
        "Desfijar",
        "Anulați fixarea";
    Pin:
        "Pin",
        "Épingler",
        "Fijar",
        "Fixați";
    NoSender:
        "(no sender)",
        "(sans expéditeur)",
        "(sin remitente)",
        "(fără expeditor)";
    NoSubject:
        "(no subject)",
        "(pas de sujet)",
        "(sin asunto)",
        "(fără subiect)";
    Pinned:
        "Pinned",
        "Épinglé",
        "Fijado",
        "Fixat";
    MoveTo:
        "Move to…",
        "Déplacer vers…",
        "Mover a…",
        "Mutați în…";
    QuotedText:
        "quoted text",
        "le texte cité",
        "el texto citado",
        "textul citat";
    Signature:
        "signature",
        "la signature",
        "la firma",
        "semnătura";
    NoConversationOpen:
        "No conversation open",
        "Aucune discussion ouverte",
        "Ninguna conversación abierta",
        "Nicio conversație deschisă";
    Reply:
        "Reply",
        "Répondre",
        "Responder",
        "Răspundeți";
    ReplyAll:
        "Reply all",
        "Répondre à tous",
        "Responder a todos",
        "Răspundeți tuturor";
    Forward:
        "Forward",
        "Transférer",
        "Reenviar",
        "Redirecționați";
    UnstarKey:
        "Unstar (S)",
        "Retirer le suivi (S)",
        "Quitar destacado (S)",
        "Eliminați steaua (S)";
    StarKey:
        "Star (S)",
        "Ajouter un suivi (S)",
        "Destacar (S)",
        "Marcați cu stea (S)";
    MarkReadKey:
        "Mark read (U)",
        "Marquer comme lu (U)",
        "Marcar como leído (U)",
        "Marcați ca citit (U)";
    MarkUnreadKey:
        "Mark unread (U)",
        "Marquer comme non lu (U)",
        "Marcar como no leído (U)",
        "Marcați ca necitit (U)";
    ArchiveKey:
        "Archive (E)",
        "Archiver (E)",
        "Archivar (E)",
        "Arhivați (E)";
    TrashKey:
        "Trash (#)",
        "Corbeille (#)",
        "Papelera (#)",
        "Coș de gunoi (#)";
    JunkKey:
        "Junk (!)",
        "Indésirable (!)",
        "Spam (!)",
        "Spam (!)";
    NotJunkKey:
        "Not junk (!)",
        "Pas indésirable (!)",
        "No es spam (!)",
        "Nu este spam (!)";
    MarkJunk:
        "Junk",
        "Indésirable",
        "Spam",
        "Spam";
    NotJunk:
        "Not junk",
        "Pas indésirable",
        "No es spam",
        "Nu este spam";
    MoveKey:
        "Move… (V)",
        "Déplacer… (V)",
        "Mover… (V)",
        "Mutați… (V)";
    NextConversation:
        "Next conversation (j)",
        "Discussion suivante (j)",
        "Conversación siguiente (j)",
        "Conversația următoare (j)";
    PreviousConversation:
        "Previous conversation (k)",
        "Discussion précédente (k)",
        "Conversación anterior (k)",
        "Conversația anterioară (k)";
    OneMessage:
        "1 MESSAGE",
        "1 MESSAGE",
        "1 MENSAJE",
        "1 MESAJ";
    OpenOriginal:
        "Open original",
        "Ouvrir l’original",
        "Abrir el original",
        "Deschideți originalul";
    OpenOriginalArrow:
        "Open original ↗",
        "Ouvrir l’original ↗",
        "Abrir el original ↗",
        "Deschideți originalul ↗";
    OpenOriginalHint:
        "A sanitized copy (no scripts, no images, nothing remote) in your browser",
        "Une copie assainie (ni scripts, ni images, rien de distant) dans votre navigateur",
        "Una copia saneada (sin scripts, sin imágenes, nada remoto) en tu navegador",
        "O copie igienizată (fără scripturi, fără imagini, nimic de la distanță) în browser";
    Loading:
        "Loading…",
        "Chargement…",
        "Cargando…",
        "Se încarcă…";
    PicturesShown:
        "Showing the pictures in this message.",
        "Les images de ce message sont affichées.",
        "Se muestran las imágenes de este mensaje.",
        "Imaginile din acest mesaj sunt afișate.";
    HideImages:
        "Hide images",
        "Masquer les images",
        "Ocultar imágenes",
        "Ascundeți imaginile";
    PicturesHidden:
        "Pictures in this message are hidden.",
        "Les images de ce message sont masquées.",
        "Las imágenes de este mensaje están ocultas.",
        "Imaginile din acest mesaj sunt ascunse.";
    ShowImages:
        "Show images",
        "Afficher les images",
        "Mostrar imágenes",
        "Afișați imaginile";
    PicturesHint:
        "Only pictures carried in the message; nothing is downloaded",
        "Seulement les images contenues dans le message ; rien n’est téléchargé",
        "Solo las imágenes incluidas en el mensaje; no se descarga nada",
        "Doar imaginile incluse în mesaj; nu se descarcă nimic";
    NoText:
        "(no text)",
        "(pas de texte)",
        "(sin texto)",
        "(fără text)";
    Appearance:
        "Appearance",
        "Apparence",
        "Apariencia",
        "Aspect";
    Keyboard:
        "Keyboard",
        "Clavier",
        "Teclado",
        "Tastatură";
    General:
        "General",
        "Général",
        "General",
        "General";
    Accounts:
        "Accounts",
        "Comptes",
        "Cuentas",
        "Conturi";
    CloseSettings:
        "Close settings (Esc)",
        "Fermer les paramètres (Échap)",
        "Cerrar los ajustes (Esc)",
        "Închideți setările (Esc)";
    NotTestedYet:
        "not tested yet",
        "pas encore testé",
        "aún sin probar",
        "netestat încă";
    Connected:
        "connected",
        "connecté",
        "conectado",
        "conectat";
    Test:
        "Test",
        "Tester",
        "Probar",
        "Testați";
    ReEnterPassword:
        "Re-enter password",
        "Saisir à nouveau le mot de passe",
        "Volver a escribir la contraseña",
        "Reintroduceți parola";
    KeepIt:
        "Keep it",
        "Le garder",
        "Conservarla",
        "Păstrați";
    RemoveDots:
        "Remove…",
        "Retirer…",
        "Quitar…",
        "Eliminați…";
    Theme:
        "Theme",
        "Thème",
        "Tema",
        "Temă";
    ThemeHint:
        "Follow the desktop, or always light or dark",
        "Suivre le bureau, ou toujours clair ou sombre",
        "Seguir el escritorio, o siempre claro u oscuro",
        "Urmați desktopul sau mereu luminos ori întunecat";
    System:
        "System",
        "Système",
        "Sistema",
        "Sistem";
    Light:
        "Light",
        "Clair",
        "Claro",
        "Luminos";
    Dark:
        "Dark",
        "Sombre",
        "Oscuro",
        "Întunecat";
    LightTheme:
        "Light theme",
        "Thème clair",
        "Tema claro",
        "Temă luminoasă";
    DarkTheme:
        "Dark theme",
        "Thème sombre",
        "Tema oscuro",
        "Temă întunecată";
    Density:
        "Density",
        "Densité",
        "Densidad",
        "Densitate";
    DensityHint:
        "How tall the rows of the thread list are",
        "La hauteur des lignes de la liste",
        "La altura de las filas de la lista de conversaciones",
        "Înălțimea rândurilor din lista de conversații";
    Cozy:
        "Cozy 48",
        "Détendue 48",
        "Holgada 48",
        "Aerisită 48";
    Compact:
        "Compact 40",
        "Compacte 40",
        "Compacta 40",
        "Compactă 40";
    PreviewDensity:
        "Preview 72",
        "Aperçu 72",
        "Vista previa 72",
        "Previzualizare 72";
    AppearancePreview:
        "Preview",
        "Aperçu",
        "Vista previa",
        "Previzualizare";
    SampleSubject1:
        "Photos from Saturday",
        "Les photos de samedi",
        "Fotos del sábado",
        "Fotografii de sâmbătă";
    SampleSnippet1:
        "Here are the best ones; the rest are in the shared album.",
        "Voici les meilleures ; les autres sont dans l’album partagé.",
        "Aquí van las mejores; el resto está en el álbum compartido.",
        "Iată cele mai bune; restul sunt în albumul comun.";
    SampleSubject2:
        "Your trip to Lisbon is confirmed",
        "Votre voyage à Lisbonne est confirmé",
        "Tu viaje a Lisboa está confirmado",
        "Călătoria dvs. la Lisabona este confirmată";
    SampleSnippet2:
        "Booking reference and check-in times inside.",
        "Référence de réservation et horaires d’enregistrement à l’intérieur.",
        "Dentro, el código de reserva y los horarios de facturación.",
        "În interior: codul rezervării și orele de check-in.";
    SampleSubject3:
        "Notes from the planning meeting",
        "Notes de la réunion de planification",
        "Notas de la reunión de planificación",
        "Notițe de la ședința de planificare";
    SampleSnippet3:
        "Thanks, I added the dates we agreed on.",
        "Merci, j’ai ajouté les dates convenues.",
        "Gracias, añadí las fechas que acordamos.",
        "Mulțumesc, am adăugat datele stabilite.";
    TextSize:
        "Text size",
        "Taille du texte",
        "Tamaño del texto",
        "Dimensiunea textului";
    TextSizeHint:
        "Ctrl + plus and minus change it for a while",
        "Ctrl + plus et moins la changent pour un moment",
        "Ctrl + más y menos lo cambian durante un rato",
        "Ctrl + plus și minus o schimbă temporar";
    Preset:
        "Preset",
        "Préréglage",
        "Preajuste",
        "Presetare";
    PresetHint:
        "The keys everything starts from",
        "Les touches de départ",
        "Las teclas de partida",
        "Tastele de pornire";
    FilterCommands:
        "Filter the commands",
        "Filtrer les commandes",
        "Filtrar los comandos",
        "Filtrați comenzile";
    Mail:
        "Mail",
        "Courrier",
        "Correo",
        "E-mail";
    Navigation:
        "Navigation",
        "Navigation",
        "Navegación",
        "Navigare";
    Account:
        "Account",
        "Compte",
        "Cuenta",
        "Cont";
    App:
        "App",
        "Application",
        "Aplicación",
        "Aplicație";
    Change:
        "Change…",
        "Changer…",
        "Cambiar…",
        "Schimbați…";
    UndoSend:
        "Undo send",
        "Annulation de l’envoi",
        "Deshacer el envío",
        "Anularea trimiterii";
    UndoSendHint:
        "How long a sent message waits before it leaves",
        "Le temps qu’un message attend avant de partir",
        "Cuánto espera un mensaje enviado antes de salir",
        "Cât așteaptă un mesaj trimis înainte de a pleca";
    Off:
        "Off",
        "Désactivé",
        "Desactivado",
        "Dezactivat";
    CheckMail:
        "Check for mail",
        "Relève du courrier",
        "Comprobar correo",
        "Verificarea e-mailului";
    CheckMailHint:
        "How often the inbox is polled without IDLE",
        "La fréquence de relève sans IDLE",
        "Cada cuánto se consulta la bandeja de entrada sin IDLE",
        "Cât de des este verificată căsuța de intrare fără IDLE";
    MarkAsRead:
        "Mark as read",
        "Marquer comme lu",
        "Marcar como leído",
        "Marcare ca citit";
    MarkAsReadHint:
        "When an open conversation is marked read (other values: set_setting in the palette)",
        "Quand une discussion ouverte est marquée comme lue (autres valeurs : set_setting dans la palette)",
        "Cuándo se marca como leída una conversación abierta (otros valores: set_setting en la paleta)",
        "Când este marcată ca citită o conversație deschisă (alte valori: set_setting în paletă)";
    Never:
        "Never",
        "Jamais",
        "Nunca",
        "Niciodată";
    Immediately:
        "Immediately",
        "Immédiatement",
        "Inmediatamente",
        "Imediat";
    After2s:
        "After 2 s",
        "Après 2 s",
        "A los 2 s",
        "După 2 s";
    After5s:
        "After 5 s",
        "Après 5 s",
        "A los 5 s",
        "După 5 s";
    Notifications:
        "Notifications",
        "Notifications",
        "Notificaciones",
        "Notificări";
    NotificationsHint:
        "New mail, failed sends and agent requests while dail is in the background",
        "Nouveau courrier, envois en échec et demandes d’agents quand dail est en arrière-plan",
        "Correo nuevo, envíos fallidos y solicitudes de agentes mientras dail está en segundo plano",
        "E-mailuri noi, trimiteri eșuate și cereri ale agenților cât timp dail rulează în fundal";
    UndoHistory:
        "Undo history",
        "Historique d’annulation",
        "Historial de deshacer",
        "Istoricul anulărilor";
    UndoHistoryHint:
        "How many steps undo remembers",
        "Le nombre d’étapes gardées",
        "Cuántos pasos recuerda deshacer",
        "Câți pași rețin anulările";
    Updates:
        "Updates",
        "Mises à jour",
        "Actualizaciones",
        "Actualizări";
    CheckAutomatically:
        "Check automatically",
        "Vérifier automatiquement",
        "Buscar automáticamente",
        "Verificați automat";
    CheckAutomaticallyHint:
        "Asks GitHub for the signed release list; nothing about you is sent",
        "Demande à GitHub la liste signée des versions ; rien sur vous n’est envoyé",
        "Pide a GitHub la lista firmada de versiones; no se envía nada sobre ti",
        "Cere de la GitHub lista semnată de versiuni; nu se trimite nimic despre dvs.";
    HowOften:
        "How often",
        "Fréquence",
        "Frecuencia",
        "Frecvență";
    HowOftenHint:
        "Between automatic checks",
        "Entre deux vérifications automatiques",
        "Entre búsquedas automáticas",
        "Între verificările automate";
    ChangePassword:
        "Change the password",
        "Changer le mot de passe",
        "Cambiar la contraseña",
        "Schimbați parola";
    AddAnAccount:
        "Add an account",
        "Ajouter un compte",
        "Añadir una cuenta",
        "Adăugați un cont";
    Cancel:
        "Cancel",
        "Annuler",
        "Cancelar",
        "Anulați";
    Next:
        "Next",
        "Suivant",
        "Siguiente",
        "Înainte";
    EnterEmail:
        "Enter an email address like you@example.com.",
        "Saisissez une adresse comme vous@example.com.",
        "Escribe una dirección como tu@example.com.",
        "Introduceți o adresă precum dvs@example.com.";
    EmailAddress:
        "Email address",
        "Adresse e-mail",
        "Dirección de correo",
        "Adresă de e-mail";
    NameOptional:
        "Name (optional)",
        "Nom (facultatif)",
        "Nombre (opcional)",
        "Nume (opțional)";
    ShownInSidebar:
        "shown in the sidebar",
        "affiché dans la barre latérale",
        "se muestra en la barra lateral",
        "afișat în bara laterală";
    ImapServer:
        "IMAP server",
        "Serveur IMAP",
        "Servidor IMAP",
        "Server IMAP";
    SmtpServer:
        "SMTP server",
        "Serveur SMTP",
        "Servidor SMTP",
        "Server SMTP";
    HostPortHint:
        "host:port, optionally :tls or :starttls (143 and 587 use STARTTLS)",
        "hôte:port, avec :tls ou :starttls en option (143 et 587 utilisent STARTTLS)",
        "host:puerto, opcionalmente :tls o :starttls (143 y 587 usan STARTTLS)",
        "gazdă:port, opțional :tls sau :starttls (143 și 587 folosesc STARTTLS)";
    FindServers:
        "Find servers",
        "Trouver les serveurs",
        "Buscar servidores",
        "Găsiți serverele";
    Looking:
        "Looking…",
        "Recherche…",
        "Buscando…",
        "Se caută…";
    FindServersHint:
        "Asks the provider's web server, then Mozilla's list.",
        "Interroge le serveur web du fournisseur, puis la liste de Mozilla.",
        "Pregunta al servidor web del proveedor y luego a la lista de Mozilla.",
        "Întreabă serverul web al furnizorului, apoi lista Mozilla.";
    ServersNote:
        "host:port, optionally :tls or :starttls. Find servers asks the provider, then Mozilla's list.",
        "hôte:port, avec :tls ou :starttls en option. « Trouver les serveurs » interroge le fournisseur, puis la liste de Mozilla.",
        "host:puerto, opcionalmente :tls o :starttls. «Buscar servidores» pregunta al proveedor y luego a la lista de Mozilla.",
        "gazdă:port, opțional :tls sau :starttls. „Găsiți serverele” întreabă furnizorul, apoi lista Mozilla.";
    Back:
        "Back",
        "Retour",
        "Atrás",
        "Înapoi";
    OauthOnly:
        "{provider} addresses need a sign-in that dail does not support yet. Use another address for now.",
        "Les adresses {provider} demandent une connexion que dail ne prend pas encore en charge. Utilisez une autre adresse pour l’instant.",
        "Las direcciones de {provider} necesitan un inicio de sesión que dail aún no admite. Por ahora, usa otra dirección.",
        "Adresele {provider} necesită o autentificare pe care dail nu o acceptă încă. Deocamdată folosiți altă adresă.";
    CreateAppPassword:
        "Create an App Password",
        "Créer un mot de passe d’application",
        "Crear una contraseña de aplicación",
        "Creați o parolă pentru aplicații";
    PrivacyPolicy:
        "Privacy policy",
        "Politique de confidentialité",
        "Política de privacidad",
        "Politica de confidențialitate";
    AppPassword:
        "App Password",
        "Mot de passe d’application",
        "Contraseña de aplicación",
        "Parolă pentru aplicații";
    Password:
        "Password",
        "Mot de passe",
        "Contraseña",
        "Parolă";
    KeyringOnly:
        "Stored in the system keyring only; dail never writes it anywhere else.",
        "Stocké uniquement dans le trousseau du système ; dail ne l’écrit nulle part ailleurs.",
        "Se guarda solo en el llavero del sistema; dail nunca la escribe en otro sitio.",
        "Este stocată doar în portofelul de chei al sistemului; dail nu o scrie niciodată în altă parte.";
    KeystoreOnly:
        "Stored in Keystore-protected storage only; dail never writes it anywhere else.",
        "Stocké uniquement dans un espace protégé par le Keystore ; dail ne l’écrit nulle part ailleurs.",
        "Se guarda solo en un almacenamiento protegido por Keystore; dail nunca la escribe en otro sitio.",
        "Este stocată doar într-un spațiu protejat de Keystore; dail nu o scrie niciodată în altă parte.";
    SaveAndTest:
        "Save and test",
        "Enregistrer et tester",
        "Guardar y probar",
        "Salvați și testați";
    AddAndTest:
        "Add and test",
        "Ajouter et tester",
        "Añadir y probar",
        "Adăugați și testați";
    ConnectedLogin:
        "Connected. IMAP and SMTP both accepted the login.",
        "Connecté. IMAP et SMTP ont accepté tous deux la connexion.",
        "Conectado. IMAP y SMTP aceptaron el inicio de sesión.",
        "Conectat. Atât IMAP, cât și SMTP au acceptat autentificarea.";
    Close:
        "Close",
        "Fermer",
        "Cerrar",
        "Închideți";
    KeepAnyway:
        "Keep it anyway",
        "Le garder quand même",
        "Conservarla de todos modos",
        "Păstrați oricum";
    TryAnotherPassword:
        "Try another password",
        "Essayer un autre mot de passe",
        "Probar otra contraseña",
        "Încercați altă parolă";
    Provider1:
        "1 / 4  Provider",
        "1 / 4  Fournisseur",
        "1 / 4  Proveedor",
        "1 / 4  Furnizor";
    Address2:
        "2 / 4  Address",
        "2 / 4  Adresse",
        "2 / 4  Dirección",
        "2 / 4  Adresă";
    Password3:
        "3 / 4  Password",
        "3 / 4  Mot de passe",
        "3 / 4  Contraseña",
        "3 / 4  Parolă";
    Connection4:
        "4 / 4  Connection",
        "4 / 4  Connexion",
        "4 / 4  Conexión",
        "4 / 4  Conexiune";
    AMessage:
        "a message",
        "un message",
        "un mensaje",
        "un mesaj";
    ApproveSend:
        "Approve send",
        "Approuver l’envoi",
        "Aprobar el envío",
        "Aprobați trimiterea";
    Edit:
        "Edit",
        "Modifier",
        "Editar",
        "Editați";
    Reject:
        "Reject",
        "Rejeter",
        "Rechazar",
        "Respingeți";
    StillChecked:
        "attachments are still being checked",
        "les pièces jointes sont encore en cours de vérification",
        "aún se están comprobando los adjuntos",
        "atașamentele sunt încă în verificare";
    SaveAttachmentsInto:
        "Save attachments into",
        "Enregistrer les pièces jointes dans",
        "Guardar los adjuntos en",
        "Salvați atașamentele în";
    OneFile:
        "1 file",
        "1 fichier",
        "1 archivo",
        "1 fișier";
    SaveAll:
        "Save all",
        "Tout enregistrer",
        "Guardar todo",
        "Salvați tot";
    SaveAllDots:
        "Save all…",
        "Tout enregistrer…",
        "Guardar todo…",
        "Salvați tot…";
    OpenArrow:
        "Open ↗",
        "Ouvrir ↗",
        "Abrir ↗",
        "Deschideți ↗";
    HidePreview:
        "Hide preview",
        "Masquer l’aperçu",
        "Ocultar la vista previa",
        "Ascundeți previzualizarea";
    Preview:
        "Preview",
        "Aperçu",
        "Vista previa",
        "Previzualizare";
    SaveDots:
        "Save…",
        "Enregistrer…",
        "Guardar…",
        "Salvați…";
    LoadingPreview:
        "Loading the preview…",
        "Chargement de l’aperçu…",
        "Cargando la vista previa…",
        "Se încarcă previzualizarea…";
    Dismiss:
        "Dismiss",
        "Ignorer",
        "Descartar",
        "Închideți";
    NothingMatches:
        "Nothing matches.",
        "Aucun résultat.",
        "Nada coincide.",
        "Nimic nu se potrivește.";
    Error:
        "Error",
        "Erreur",
        "Error",
        "Eroare";
    Sending:
        "Sending…",
        "Envoi…",
        "Enviando…",
        "Se trimite…";
    Then:
        "then",
        "puis",
        "luego",
        "apoi";
    NotSentNotice:
        "Not sent.",
        "Non envoyé.",
        "No enviado.",
        "Netrimis.";
    AttachmentsChecked:
        "Its attachments are still being checked.",
        "Ses pièces jointes sont encore en cours de vérification.",
        "Aún se están comprobando sus adjuntos.",
        "Atașamentele sale sunt încă în verificare.";
    SendRequests:
        " Send requests ",
        " Demandes d’envoi ",
        " Solicitudes de envío ",
        " Cereri de trimitere ";
    NoSendRequests:
        "No send requests.",
        "Aucune demande d’envoi.",
        "No hay solicitudes de envío.",
        "Nu există cereri de trimitere.";
    ApprovalsCheckingFooter:
        "attachments are being checked · e edit · r reject · q close",
        "vérification des pièces jointes · e modifier · r rejeter · q fermer",
        "comprobando adjuntos · e editar · r rechazar · q cerrar",
        "se verifică atașamentele · e editați · r respingeți · q închideți";
    ApprovalsFooter:
        "a approve · e edit · r reject · j/k next · q close",
        "a approuver · e modifier · r rejeter · j/k suivant · q fermer",
        "a aprobar · e editar · r rechazar · j/k siguiente · q cerrar",
        "a aprobați · e editați · r respingeți · j/k următoarea · q închideți";
    QClose:
        "q close",
        "q fermer",
        "q cerrar",
        "q închideți";
    OriginalBelowTui:
        "(the original is added below when sent)",
        "(l’original est ajouté dessous à l’envoi)",
        "(el original se añade debajo al enviar)",
        "(originalul este adăugat dedesubt la trimitere)";
    ReadOnlyWhileSent:
        "read-only while it is sent · z cancels",
        "lecture seule pendant l’envoi · z annule",
        "solo lectura mientras se envía · z cancela",
        "doar citire cât se trimite · z anulează";
    ComposerFooter:
        "Ctrl+S send · Ctrl+O $EDITOR · Ctrl+K “Attach file” · Esc close",
        "Ctrl+S envoyer · Ctrl+O $EDITOR · Ctrl+K « Attach file » · Échap fermer",
        "Ctrl+S enviar · Ctrl+O $EDITOR · Ctrl+K «Attach file» · Esc cerrar",
        "Ctrl+S trimiteți · Ctrl+O $EDITOR · Ctrl+K „Attach file” · Esc închideți";
    FromColon:
        "From: ",
        "De : ",
        "De:  ",
        "De la:  ";
    PaletteTitle:
        " Palette ",
        " Palette ",
        " Paleta ",
        " Paletă ";
    NothingMatchesTui:
        "  Nothing matches.",
        "  Aucun résultat.",
        "  Nada coincide.",
        "  Nimic nu se potrivește.";
    EnterUseEscBack:
        "Enter to use · Esc to go back",
        "Entrée pour utiliser · Échap pour revenir",
        "Intro para usar · Esc para volver",
        "Enter pentru a folosi · Esc pentru a reveni";
    PaletteFooter:
        "↑↓ move · Enter choose · Esc back",
        "↑↓ déplacer · Entrée choisir · Échap retour",
        "↑↓ mover · Intro elegir · Esc atrás",
        "↑↓ deplasare · Enter alegeți · Esc înapoi";
    ScrollClose:
        "↑↓ Space b scroll · q or Esc close",
        "↑↓ Espace b défiler · q ou Échap fermer",
        "↑↓ Espacio b desplazar · q o Esc cerrar",
        "↑↓ Spațiu b derulare · q sau Esc închideți";
    ScrollView:
        "↑↓ Space b scroll · r view · q or Esc close",
        "↑↓ Espace b défiler · r vue · q ou Échap fermer",
        "↑↓ Espacio b desplazar · r vista · q o Esc cerrar",
        "↑↓ Spațiu b derulare · r vizualizare · q sau Esc închideți";
    ScrollOpen:
        "↑↓ scroll · Tab Enter open · r raw data · q or Esc close",
        "↑↓ défiler · Tab Entrée ouvrir · r données brutes · q ou Échap fermer",
        "↑↓ desplazar · Tab Intro abrir · r datos sin procesar · q o Esc cerrar",
        "↑↓ derulare · Tab Enter deschideți · r date brute · q sau Esc închideți";
    NoConversationTui:
        "No conversation open. Enter opens the selected one.",
        "Aucune discussion ouverte. Entrée ouvre celle qui est sélectionnée.",
        "Ninguna conversación abierta. Intro abre la seleccionada.",
        "Nicio conversație deschisă. Enter o deschide pe cea selectată.";
    Keys:
        "Keys",
        "Touches",
        "Teclas",
        "Taste";
    NotTested:
        "not tested",
        "non testé",
        "sin probar",
        "netestat";
    SettingsTitle:
        " Settings ",
        " Paramètres ",
        " Ajustes ",
        " Setări ";
    SettingsFooter:
        "Tab section · j/k move · Enter change · Esc close",
        "Tab section · j/k déplacer · Entrée changer · Échap fermer",
        "Tab sección · j/k mover · Intro cambiar · Esc cerrar",
        "Tab secțiune · j/k deplasare · Enter schimbați · Esc închideți";
    AddAccountTitle:
        " Add account ",
        " Ajouter un compte ",
        " Añadir cuenta ",
        " Adăugați un cont ";
    Step1:
        "1/4 Provider",
        "1/4 Fournisseur",
        "1/4 Proveedor",
        "1/4 Furnizor";
    Step2:
        "2/4 Address",
        "2/4 Adresse",
        "2/4 Dirección",
        "2/4 Adresă";
    Step3:
        "3/4 Password",
        "3/4 Mot de passe",
        "3/4 Contraseña",
        "3/4 Parolă";
    Step4:
        "4/4 Connection",
        "4/4 Connexion",
        "4/4 Conexión",
        "4/4 Conexiune";
    ChooseFooter:
        "↑↓ choose · Enter next · Esc cancel",
        "↑↓ choisir · Entrée suivant · Échap annuler",
        "↑↓ elegir · Intro siguiente · Esc cancelar",
        "↑↓ alegeți · Enter înainte · Esc anulați";
    Email:
        "Email",
        "E-mail",
        "Correo",
        "E-mail";
    ImapHostPort:
        "IMAP host:port",
        "IMAP hôte:port",
        "IMAP host:puerto",
        "IMAP gazdă:port";
    SmtpHostPort:
        "SMTP host:port",
        "SMTP hôte:port",
        "SMTP host:puerto",
        "SMTP gazdă:port";
    PortsHint:
        "Ports 143 and 587 use STARTTLS, others TLS; add :tls or :starttls to choose.",
        "Les ports 143 et 587 utilisent STARTTLS, les autres TLS ; ajoutez :tls ou :starttls pour choisir.",
        "Los puertos 143 y 587 usan STARTTLS, los demás TLS; añade :tls o :starttls para elegir.",
        "Porturile 143 și 587 folosesc STARTTLS, celelalte TLS; adăugați :tls sau :starttls pentru a alege.";
    CtrlFHint:
        "Ctrl+F finds the servers (asks the provider's web server, then Mozilla's list).",
        "Ctrl+F trouve les serveurs (interroge le serveur web du fournisseur, puis la liste de Mozilla).",
        "Ctrl+F busca los servidores (pregunta al servidor web del proveedor y luego a la lista de Mozilla).",
        "Ctrl+F găsește serverele (întreabă serverul web al furnizorului, apoi lista Mozilla).";
    FieldFooter:
        "Tab field · Enter next · Esc back",
        "Tab champ · Entrée suivant · Échap retour",
        "Tab campo · Intro siguiente · Esc atrás",
        "Tab câmp · Enter înainte · Esc înapoi";
    KeyringTui:
        "It goes to the system keyring, never into dail's files.",
        "Il va dans le trousseau du système, jamais dans les fichiers de dail.",
        "Va al llavero del sistema, nunca a los archivos de dail.",
        "Ajunge în portofelul de chei al sistemului, niciodată în fișierele dail.";
    SaveTestFooter:
        "Enter save and test · Esc cancel",
        "Entrée enregistrer et tester · Échap annuler",
        "Intro guardar y probar · Esc cancelar",
        "Enter salvați și testați · Esc anulați";
    AddTestFooter:
        "Enter add and test · Esc back",
        "Entrée ajouter et tester · Échap retour",
        "Intro añadir y probar · Esc atrás",
        "Enter adăugați și testați · Esc înapoi";
    ConnectedCap:
        "Connected.",
        "Connecté.",
        "Conectado.",
        "Conectat.";
    SyncCloseFooter:
        "Enter syncs now · Esc closes",
        "Entrée synchronise · Échap ferme",
        "Intro sincroniza ahora · Esc cierra",
        "Enter sincronizează acum · Esc închide";
    LoginFailedCap:
        "The login failed:",
        "La connexion a échoué :",
        "Error al iniciar sesión:",
        "Autentificarea a eșuat:";
    PasswordClose:
        "p another password · k close",
        "p autre mot de passe · k fermer",
        "p otra contraseña · k cerrar",
        "p altă parolă · k închideți";
    PasswordKeep:
        "p another password · k keep it anyway",
        "p autre mot de passe · k le garder quand même",
        "p otra contraseña · k conservarla de todos modos",
        "p altă parolă · k păstrați oricum";
    TestingLogin:
        "Testing the login…",
        "Test de la connexion…",
        "Probando el inicio de sesión…",
        "Se testează autentificarea…";
    QuitConfirm:
        "Quitting cancels the send still in its undo window. q quits, any other key stays",
        "Quitter annule l’envoi encore dans son délai d’annulation. q quitte, toute autre touche reste",
        "Salir cancela el envío que sigue en su plazo para deshacer. q sale, cualquier otra tecla se queda",
        "Ieșirea anulează trimiterea aflată încă în intervalul de anulare. q iese, orice altă tastă rămâne";
    PickMailboxTui:
        "Pick a mailbox: g i for the inbox.",
        "Choisissez un dossier : g i pour le courrier entrant.",
        "Elige una carpeta: g i para la bandeja de entrada.",
        "Alegeți un dosar: g i pentru căsuța de intrare.";
    PressEnterToAdd:
        "Press Enter to add one here, or run in another shell:",
        "Appuyez sur Entrée pour en ajouter un ici, ou lancez dans un autre terminal :",
        "Pulsa Intro para añadir una aquí, o ejecuta en otro terminal:",
        "Apăsați Enter pentru a adăuga unul aici sau rulați în alt terminal:";
    AccountShowsUp:
        "The account shows up here once it is added.",
        "Le compte apparaît ici une fois ajouté.",
        "La cuenta aparece aquí en cuanto se añade.",
        "Contul apare aici după ce este adăugat.";
    SetEditor:
        "Set $VISUAL or $EDITOR to write the body in your editor",
        "Définissez $VISUAL ou $EDITOR pour écrire le message dans votre éditeur",
        "Define $VISUAL o $EDITOR para escribir el mensaje en tu editor",
        "Setați $VISUAL sau $EDITOR pentru a scrie mesajul în editorul dvs.";
    StillHere:
        "Still here; the send goes out when its window closes.",
        "Toujours là ; l’envoi part à la fin de son délai.",
        "Sigue aquí; el envío sale cuando termina su plazo.",
        "Încă aici; mesajul pleacă la încheierea intervalului.";
    MouseOn:
        "Mouse on: tap to select, swipe to scroll",
        "Souris active : touchez pour sélectionner, glissez pour défiler",
        "Ratón activado: toca para seleccionar, desliza para desplazar",
        "Mouse activ: atingeți pentru a selecta, glisați pentru a derula";
    MouseOff:
        "Mouse off: select text with the terminal; m turns it back on",
        "Souris inactive : sélectionnez le texte avec le terminal ; m la réactive",
        "Ratón desactivado: selecciona el texto con el terminal; m lo vuelve a activar",
        "Mouse inactiv: selectați textul cu terminalul; m îl reactivează";
    HelpNote:
        "Problems, the keys of the registry, the TUI's own. q, Esc or ? closes.",
        "Problèmes, touches du registre, touches du terminal. q, Échap ou ? ferme.",
        "Problemas, las teclas del registro y las propias del terminal. q, Esc o ? cierra.",
        "Probleme, tastele registrului și cele ale terminalului. q, Esc sau ? închide.";
    TerminalTooSmall:
        "Terminal too small",
        "Terminal trop petit",
        "Terminal demasiado pequeño",
        "Terminal prea mic";
    Mailboxes:
        "Mailboxes",
        "Dossiers",
        "Carpetas",
        "Dosare";
    Message:
        "Message",
        "Message",
        "Mensaje",
        "Mesaj";
    Draft:
        "Draft",
        "Brouillon",
        "Borrador",
        "Ciornă";
    Conversations:
        "Conversations",
        "Discussions",
        "Conversaciones",
        "Conversații";
    FocusFooter:
        "Focus · palette: Exit focus",
        "Focus · palette : Exit focus",
        "Enfoque · paleta: Exit focus",
        "Concentrare · paletă: Exit focus";
    OfflineSuffix:
        " offline",
        " hors ligne",
        " sin conexión",
        " offline";
    FixPalettePassword:
        " (palette: Change account password)",
        " (palette : Change account password)",
        " (paleta: Change account password)",
        " (paletă: Change account password)";
    FixPaletteSync:
        " (palette: Sync now)",
        " (palette : Sync now)",
        " (paleta: Sync now)",
        " (paletă: Sync now)";
    FixPaletteTest:
        " (palette: Test account connection)",
        " (palette : Test account connection)",
        " (paleta: Test account connection)",
        " (paletă: Test account connection)";
    UpdatesQuestion:
        "Check for updates? (settings: update_checks)",
        "Rechercher des mises à jour ? (paramètres : update_checks)",
        "¿Buscar actualizaciones? (ajustes: update_checks)",
        "Căutați actualizări? (setări: update_checks)";
    HelpHint:
        "? help",
        "? aide",
        "? ayuda",
        "? ajutor";
    Problems:
        "Problems\n",
        "Problèmes\n",
        "Problemas\n",
        "Probleme\n";
    FolderList:
        "{name} folders",
        "Dossiers de {name}",
        "Carpetas de {name}",
        "Dosarele {name}";
    SubfolderList:
        "{name} subfolders",
        "Sous-dossiers de {name}",
        "Subcarpetas de {name}",
        "Subdosarele {name}";
    ShowHidden:
        "Show hidden ({n})",
        "Afficher les masqués ({n})",
        "Mostrar ocultas ({n})",
        "Afișați ascunsele ({n})";
    ChangesWait:
        "{n} change(s) wait for the server",
        "{n} modification(s) en attente du serveur",
        "{n} cambio(s) esperan al servidor",
        "{n} modificare(i) așteaptă serverul";
    CollapseWhat:
        "Collapse {what}",
        "Réduire {what}",
        "Contraer {what}",
        "Restrângeți {what}";
    ExpandWhat:
        "Expand {what}",
        "Développer {what}",
        "Expandir {what}",
        "Extindeți {what}";
    FetchingOlder:
        "Fetching older mail: {cached} of {wanted} cached",
        "Récupération du courrier ancien : {cached} sur {wanted} en cache",
        "Descargando correo anterior: {cached} de {wanted} en caché",
        "Se descarcă e-mailurile mai vechi: {cached} din {wanted} în cache";
    SyncNowKeys:
        "Sync now ({keys})",
        "Synchroniser ({keys})",
        "Sincronizar ahora ({keys})",
        "Sincronizați acum ({keys})";
    DraftSaveFailed:
        "Draft save failed: {error}",
        "Échec de l’enregistrement du brouillon : {error}",
        "Error al guardar el borrador: {error}",
        "Salvarea ciornei a eșuat: {error}";
    NotSent:
        "Not sent: {error}",
        "Non envoyé : {error}",
        "No enviado: {error}",
        "Netrimis: {error}";
    RemoveName:
        "Remove {name}",
        "Retirer {name}",
        "Quitar {name}",
        "Eliminați {name}";
    SearchTitle:
        "Search: {query}",
        "Recherche : {query}",
        "Búsqueda: {query}",
        "Căutare: {query}";
    UnreadCount:
        "{n} unread",
        "{n} non lus",
        "{n} no leídos",
        "{n} necitite";
    CheckedCount:
        "{n} checked",
        "{n} cochés",
        "{n} marcados",
        "{n} bifate";
    LabelConversation:
        "{label} conversation",
        "{label} la discussion",
        "{label} la conversación",
        "{label} conversația";
    HideWhat:
        "Hide {what}",
        "Masquer {what}",
        "Ocultar {what}",
        "Ascundeți {what}";
    ShowWhat:
        "Show {what}",
        "Afficher {what}",
        "Mostrar {what}",
        "Afișați {what}";
    NOfM:
        "{n} of {m}",
        "{n} sur {m}",
        "{n} de {m}",
        "{n} din {m}";
    MessagesCount:
        "{n} MESSAGES",
        "{n} MESSAGES",
        "{n} MENSAJES",
        "{n} MESAJE";
    ExpandMessage:
        "Expand message from {from}",
        "Développer le message de {from}",
        "Expandir el mensaje de {from}",
        "Extindeți mesajul de la {from}";
    CollapseMessage:
        "Collapse message from {from}",
        "Réduire le message de {from}",
        "Contraer el mensaje de {from}",
        "Restrângeți mesajul de la {from}";
    ToList:
        "to {to}",
        "pour {to}",
        "para {to}",
        "către {to}";
    BodyFailed:
        "The body could not be downloaded: {error}",
        "Le message n’a pas pu être téléchargé : {error}",
        "No se pudo descargar el mensaje: {error}",
        "Mesajul nu a putut fi descărcat: {error}";
    ActionFrom:
        "{action} from {sender}",
        "{action} de {sender}",
        "{action} de {sender}",
        "{action} de la {sender}";
    LinksCount:
        "Links ({n})",
        "Liens ({n})",
        "Enlaces ({n})",
        "Linkuri ({n})";
    OpenInBrowser:
        "Open in your browser:\n{url}",
        "Ouvrir dans votre navigateur :\n{url}",
        "Abrir en tu navegador:\n{url}",
        "Deschideți în browser:\n{url}";
    FailedError:
        "failed: {error}",
        "échec : {error}",
        "error: {error}",
        "eșuat: {error}";
    RemoveAccountCache:
        "Remove {email} and its mail cache",
        "Supprimer {email} et son cache de courrier",
        "Quitar {email} y su caché de correo",
        "Eliminați {email} și memoria cache de e-mail";
    ChangeKeysOf:
        "Change the keys of {name}",
        "Changer les touches de {name}",
        "Cambiar las teclas de {name}",
        "Schimbați tastele pentru {name}";
    TestConnection:
        "{email}  Connection",
        "{email}  Connexion",
        "{email}  Conexión",
        "{email}  Conexiune";
    FoundAt:
        "Found at {source}",
        "Trouvé sur {source}",
        "Encontrado en {source}",
        "Găsit la {source}";
    NotFound:
        "Not found ({error}); enter them yourself.",
        "Introuvable ({error}) ; saisissez-les vous-même.",
        "No encontrado ({error}); escríbelos tú.",
        "Negăsit ({error}); introduceți-le manual.";
    SigningIn:
        "Signing in to {email}…",
        "Connexion à {email}…",
        "Iniciando sesión en {email}…",
        "Se face autentificarea pentru {email}…";
    LoginFailedError:
        "The login failed: {error}",
        "La connexion a échoué : {error}",
        "Error al iniciar sesión: {error}",
        "Autentificarea a eșuat: {error}";
    NMessages:
        "{n} messages",
        "{n} messages",
        "{n} mensajes",
        "{n} mesaje";
    FromName:
        "from {from}",
        "de {from}",
        "de {from}",
        "de la {from}";
    NFiles:
        "{n} files",
        "{n} fichiers",
        "{n} archivos",
        "{n} fișiere";
    AttachmentsSummary:
        "Attachments ({files} · {size})",
        "Pièces jointes ({files} · {size})",
        "Adjuntos ({files} · {size})",
        "Atașamente ({files} · {size})";
    SavingInto:
        "Saving into {dir}…",
        "Enregistrement dans {dir}…",
        "Guardando en {dir}…",
        "Se salvează în {dir}…";
    SaveFailed:
        "Could not save the attachments: {error}",
        "Impossible d’enregistrer les pièces jointes : {error}",
        "No se pudieron guardar los adjuntos: {error}",
        "Atașamentele nu au putut fi salvate: {error}";
    NoPreview:
        "No preview: {error}",
        "Pas d’aperçu : {error}",
        "Sin vista previa: {error}",
        "Fără previzualizare: {error}";
    SavedAs:
        "Saved as {name}",
        "Enregistré sous {name}",
        "Guardado como {name}",
        "Salvat ca {name}";
    SavedFiles:
        "Saved {n} files: {names}",
        "{n} fichiers enregistrés : {names}",
        "{n} archivos guardados: {names}",
        "{n} fișiere salvate: {names}";
    AgentRequest:
        "Agent request: send {what} — check it before approving",
        "Demande d’agent : envoyer {what} — vérifiez avant d’approuver",
        "Solicitud de un agente: enviar {what}; revísalo antes de aprobarlo",
        "Cerere de la un agent: trimiterea {what}; verificați înainte de a aproba";
    AgentWantsSend:
        "! An agent wants to send {what}",
        "! Un agent veut envoyer {what}",
        "! Un agente quiere enviar {what}",
        "! Un agent vrea să trimită {what}";
    ApprovalsHintTui:
        " — Ctrl+K, “Show send requests”",
        " — Ctrl+K, « Show send requests »",
        " — Ctrl+K, «Show send requests»",
        " — Ctrl+K, „Show send requests”";
    SendNowQuestion:
        "Send “{subject}” now? y sends (with the undo window), any other key doesn't",
        "Envoyer « {subject} » maintenant ? y envoie (avec le délai d’annulation), toute autre touche non",
        "¿Enviar «{subject}» ahora? y envía (con el plazo para deshacer), cualquier otra tecla no",
        "Trimiteți „{subject}” acum? y trimite (cu intervalul de anulare), orice altă tastă nu";
    FromLine:
        "  From: {from}",
        "  De : {from}",
        "  De: {from}",
        "  De la: {from}";
    SubjectLine:
        "Subject: {subject}",
        "Objet : {subject}",
        "Asunto: {subject}",
        "Subiect: {subject}";
    FromHeader:
        "From: {from}",
        "De : {from}",
        "De: {from}",
        "De la: {from}";
    DateLine:
        "In: {mailbox}",
        "Dans : {mailbox}",
        "En: {mailbox}",
        "În: {mailbox}";
    AttachmentsCount:
        "Attachments ({n})",
        "Pièces jointes ({n})",
        "Adjuntos ({n})",
        "Atașamente ({n})";
    ChangePasswordOf:
        "Change the password · {email}",
        "Changer le mot de passe · {email}",
        "Cambiar la contraseña · {email}",
        "Schimbați parola · {email}";
    AddAccountStep:
        "Add account · {step}",
        "Ajouter un compte · {step}",
        "Añadir cuenta · {step}",
        "Adăugați un cont · {step}";
    FoundFill:
        "Found at {source}: {imap} and {smtp}. Ctrl+F fills them in.",
        "Trouvé sur {source} : {imap} et {smtp}. Ctrl+F les remplit.",
        "Encontrado en {source}: {imap} y {smtp}. Ctrl+F los rellena.",
        "Găsit la {source}: {imap} și {smtp}. Ctrl+F le completează.";
    EditorFailed:
        "The editor failed: {reason}",
        "L’éditeur a échoué : {reason}",
        "El editor falló: {reason}",
        "Editorul a eșuat: {reason}";
    TitleFocus:
        "{title} · Focus",
        "{title} · Focus",
        "{title} · Enfoque",
        "{title} · Concentrare";
    TitleOnly:
        "{title} · only {only}",
        "{title} · seulement {only}",
        "{title} · solo {only}",
        "{title} · doar {only}";
    SendingCancel:
        "{sending} · z cancels",
        "{sending} · z annule",
        "{sending} · z cancela",
        "{sending} · z anulează";
    MoreDetails:
        " (+{n} more, ? for details)",
        " (+{n} autres, ? pour les détails)",
        " (+{n} más, ? para detalles)",
        " (+{n} în plus, ? pentru detalii)";
    RetryPalette:
        "{text} (retry: palette)",
        "{text} (réessayer : palette)",
        "{text} (reintentar: paleta)",
        "{text} (reîncercați: paletă)";
    UndoLabel:
        "“{label}” z undo",
        "« {label} » z annuler",
        "«{label}» z deshacer",
        "„{label}” z anulați";
    BadgeSettings:
        "{badge} (settings)",
        "{badge} (paramètres)",
        "{badge} (ajustes)",
        "{badge} (setări)";
    PresetGmailHint:
        "Needs an App Password (2-Step Verification on).",
        "Nécessite un mot de passe d’application (validation en deux étapes activée).",
        "Necesita una contraseña de aplicación (verificación en dos pasos activada).",
        "Necesită o parolă pentru aplicații (cu verificarea în doi pași activată).";
    PresetHostingerHint:
        "Your mailbox password.",
        "Le mot de passe de votre compte.",
        "La contraseña de tu buzón.",
        "Parola căsuței dvs. poștale.";
    PresetCustomHint:
        "Enter the servers yourself.",
        "Saisissez les serveurs vous-même.",
        "Escribe los servidores tú.",
        "Introduceți serverele manual.";
    PresetGmail:
        "Gmail",
        "Gmail",
        "Gmail",
        "Gmail";
    PresetHostinger:
        "Hostinger Mail",
        "Hostinger Mail",
        "Hostinger Mail",
        "Hostinger Mail";
    PresetOther:
        "Other IMAP / SMTP",
        "Autre IMAP / SMTP",
        "Otro IMAP / SMTP",
        "Alt IMAP / SMTP";
    HintChoose:
        "choose",
        "choisir",
        "elegir",
        "alegeți";
    HintBack:
        "back",
        "retour",
        "atrás",
        "înapoi";
    HintOpen:
        "open",
        "ouvrir",
        "abrir",
        "deschideți";
    HintEverything:
        "everything",
        "tout",
        "todo",
        "tot";
    HintNext:
        "next",
        "suivant",
        "siguiente",
        "următorul";
    HintPrevious:
        "previous",
        "précédent",
        "anterior",
        "anteriorul";
    HintSelect:
        "select",
        "sélectionner",
        "seleccionar",
        "selectați";
    ToLabel:
        "To",
        "Pour",
        "Para",
        "Către";
    ReleaseNotes:
        "  release notes",
        "  notes de version",
        "  notas de la versión",
        "  note de lansare";
    DateLabel:
        "Date: {date} ({age})",
        "Date : {date} ({age})",
        "Fecha: {date} ({age})",
        "Data: {date} ({age})";
    UnknownSender:
        "(unknown sender)",
        "(expéditeur inconnu)",
        "(remitente desconocido)",
        "(expeditor necunoscut)";
    SendingFailed:
        "Sending failed",
        "Échec de l’envoi",
        "Error al enviar",
        "Trimiterea a eșuat";
    AnAgentWantsToSendAMessage:
        "An agent wants to send a message",
        "Un agent veut envoyer un message",
        "Un agente quiere enviar un mensaje",
        "Un agent vrea să trimită un mesaj";
    To:
        "To:",
        "Pour :",
        "Para:",
        "Către:";
    Cc:
        "Cc:",
        "Copie à :",
        "CC:",
        "Cc:";
    Bcc:
        "Bcc:",
        "Copie cachée à :",
        "CCO:",
        "Bcc:";
    BccX:
        "Bcc",
        "Copie cachée à",
        "CCO",
        "Bcc";
    SendingX:
        "sending…",
        "envoi…",
        "enviando…",
        "se trimite…";
    NotSavedToTheServer:
        "not saved to the server",
        "non enregistré sur le serveur",
        "no guardado en el servidor",
        "nesalvat pe server";
    Saving:
        "saving…",
        "enregistrement…",
        "guardando…",
        "se salvează…";
    Saved:
        "saved",
        "enregistré",
        "guardado",
        "salvat";
    DraftX:
        "draft",
        "brouillon",
        "borrador",
        "ciornă";
    NewMessage:
        "New message",
        "Nouveau message",
        "Mensaje nuevo",
        "Mesaj nou";
    January:
        "January",
        "janvier",
        "enero",
        "ianuarie";
    February:
        "February",
        "février",
        "febrero",
        "februarie";
    March:
        "March",
        "mars",
        "marzo",
        "martie";
    April:
        "April",
        "avril",
        "abril",
        "aprilie";
    May:
        "May",
        "mai",
        "mayo",
        "mai";
    June:
        "June",
        "juin",
        "junio",
        "iunie";
    July:
        "July",
        "juillet",
        "julio",
        "iulie";
    August:
        "August",
        "août",
        "agosto",
        "august";
    September:
        "September",
        "septembre",
        "septiembre",
        "septembrie";
    October:
        "October",
        "octobre",
        "octubre",
        "octombrie";
    November:
        "November",
        "novembre",
        "noviembre",
        "noiembrie";
    December:
        "December",
        "décembre",
        "diciembre",
        "decembrie";
    Jan:
        "Jan",
        "janv.",
        "ene.",
        "ian.";
    Feb:
        "Feb",
        "févr.",
        "feb.",
        "feb.";
    Mar:
        "Mar",
        "mars",
        "mar.",
        "mar.";
    Apr:
        "Apr",
        "avr.",
        "abr.",
        "apr.";
    Jun:
        "Jun",
        "juin",
        "jun.",
        "iun.";
    Jul:
        "Jul",
        "juil.",
        "jul.",
        "iul.";
    Aug:
        "Aug",
        "août",
        "ago.",
        "aug.";
    Sep:
        "Sep",
        "sept.",
        "sept.",
        "sept.";
    Oct:
        "Oct",
        "oct.",
        "oct.",
        "oct.";
    Nov:
        "Nov",
        "nov.",
        "nov.",
        "nov.";
    Dec:
        "Dec",
        "déc.",
        "dic.",
        "dec.";
    Later:
        "Later",
        "Plus tard",
        "Más tarde",
        "Mai târziu";
    Today:
        "Today",
        "Aujourd’hui",
        "Hoy",
        "Azi";
    Yesterday:
        "Yesterday",
        "Hier",
        "Ayer",
        "Ieri";
    ThisWeek:
        "This week",
        "Cette semaine",
        "Esta semana",
        "Săptămâna aceasta";
    EarlierThisMonth:
        "Earlier this month",
        "Plus tôt ce mois-ci",
        "Antes este mes",
        "Mai devreme luna aceasta";
    Now:
        "now",
        "maintenant",
        "ahora",
        "acum";
    JustNow:
        "just now",
        "à l’instant",
        "hace un momento",
        "chiar acum";
    Inbox:
        "Inbox",
        "Courrier entrant",
        "Bandeja de entrada",
        "Căsuță de intrare";
    Sent:
        "Sent",
        "Envoyés",
        "Enviados",
        "Trimise";
    Drafts:
        "Drafts",
        "Brouillons",
        "Borradores",
        "Ciorne";
    Junk:
        "Junk",
        "Indésirables",
        "Spam",
        "Spam";
    AllMail:
        "All Mail",
        "Tous les messages",
        "Todo el correo",
        "Toate e-mailurile";
    WhichConversation:
        "Which conversation?",
        "Quelle discussion ?",
        "¿Qué conversación?",
        "Ce conversație?";
    WhichDraft:
        "Which draft?",
        "Quel brouillon ?",
        "¿Qué borrador?",
        "Ce ciornă?";
    WhichAccount:
        "Which account?",
        "Quel compte ?",
        "¿Qué cuenta?",
        "Ce cont?";
    RemoveThisAccountThisCannotBeUndone:
        "Remove this account? This cannot be undone.",
        "Supprimer ce compte ? C’est irréversible.",
        "¿Quitar esta cuenta? No se puede deshacer.",
        "Eliminați acest cont? Acțiunea nu poate fi anulată.";
    InstallTheUpdateDailAsksToRestart:
        "Install the update? dail asks to restart afterwards.",
        "Installer la mise à jour ? dail demandera de redémarrer ensuite.",
        "¿Instalar la actualización? dail pedirá reiniciarse después.",
        "Instalați actualizarea? dail va cere apoi repornirea.";
    ItsName:
        "its name",
        "son nom",
        "su nombre",
        "numele său";
    NewNameFullPath:
        "New name (full path)?",
        "Nouveau nom (chemin complet) ?",
        "¿Nombre nuevo (ruta completa)?",
        "Nume nou (cale completă)?";
    DeleteThisFolderAndAllMailIn:
        "Delete this folder and all mail in it? This cannot be undone.",
        "Supprimer ce dossier et tout son courrier ? C’est irréversible.",
        "¿Eliminar esta carpeta y todo su correo? No se puede deshacer.",
        "Ștergeți acest dosar și toate mesajele din el? Acțiunea nu poate fi anulată.";
    FindTheMailServersOfWhichAddress:
        "Find the mail servers of which address?",
        "Trouver les serveurs de quelle adresse ?",
        "¿Buscar los servidores de qué dirección?",
        "Găsiți serverele cărei adrese?";
    YouExampleOrgAsksTheProviderThen:
        "you@example.org (asks the provider, then Mozilla)",
        "vous@example.org (interroge le fournisseur, puis Mozilla)",
        "tu@example.org (pregunta al proveedor y luego a Mozilla)",
        "dvs@example.org (întreabă furnizorul, apoi Mozilla)";
    WhichTheme:
        "Which theme?",
        "Quel thème ?",
        "¿Qué tema?",
        "Ce temă?";
    SystemLightOrDarkThenATheme:
        "system, light or dark, then a theme for that mode: dark dail-dark-hc",
        "system, light ou dark, puis un thème pour ce mode : dark dail-dark-hc",
        "system, light o dark, y luego un tema para ese modo: dark dail-dark-hc",
        "system, light sau dark, apoi o temă pentru acel mod: dark dail-dark-hc";
    WhichCommand:
        "Which command?",
        "Quelle commande ?",
        "¿Qué comando?",
        "Ce comandă?";
    ItsNameEGArchiveThreads:
        "its name, e.g. archive_threads",
        "son nom, p. ex. archive_threads",
        "su nombre, p. ej. archive_threads",
        "numele ei, de ex. archive_threads";
    WhichKeysCommaSeparatedEmptyThePreset:
        "Which keys? (comma-separated; empty: the preset's)",
        "Quelles touches ? (séparées par des virgules ; vide : celles du préréglage)",
        "¿Qué teclas? (separadas por comas; vacío: las del preajuste)",
        "Ce taste? (separate prin virgulă; gol: cele ale presetării)";
    WhichSetting:
        "Which setting?",
        "Quel paramètre ?",
        "¿Qué ajuste?",
        "Ce setare?";
    NewDisplayName:
        "New display name",
        "Nouveau nom affiché",
        "Nombre visible nuevo",
        "Nume afișat nou";
    WhichMailbox:
        "Which mailbox?",
        "Quel dossier ?",
        "¿Qué carpeta?",
        "Ce dosar?";
    KeepHowManyOfTheNewestMessages:
        "Keep how many of the newest messages offline?",
        "Garder combien de messages récents hors ligne ?",
        "¿Cuántos de los mensajes más recientes conservar sin conexión?",
        "Câte dintre cele mai noi mesaje păstrați offline?";
    ANumber0KeepsAllOfIt:
        "a number; 0 keeps all of it",
        "un nombre ; 0 garde tout",
        "un número; 0 lo conserva todo",
        "un număr; 0 le păstrează pe toate";
    GoToMailbox:
        "Go to mailbox",
        "Aller au dossier",
        "Ir a la carpeta",
        "Mergeți la dosar";
    WhichAddress:
        "Which address?",
        "Quelle adresse ?",
        "¿Qué dirección?",
        "Ce adresă?";
    ItsSignatureEmptyNone:
        "Its signature? (empty: none)",
        "Sa signature ? (vide : aucune)",
        "¿Su firma? (vacío: ninguna)",
        "Semnătura ei? (gol: niciuna)";
    NameCompany:
        "Name · Company",
        "Nom · Société",
        "Nombre · Empresa",
        "Nume · Companie";
    RemoveWhichAddress:
        "Remove which address?",
        "Retirer quelle adresse ?",
        "¿Quitar qué dirección?",
        "Eliminați ce adresă?";
    TrustCertificateDots:
        "Trust certificate…",
        "Faire confiance au certificat…",
        "Confiar en el certificado…",
        "Acceptați certificatul…";
    ForgetCertificateDots:
        "Forget certificate…",
        "Oublier le certificat…",
        "Olvidar el certificado…",
        "Uitați certificatul…";
    FixTrust:
        "trust…",
        "faire confiance…",
        "confiar…",
        "acceptați…";
    FixPaletteTrust:
        " (palette: Trust a server's certificate)",
        " (palette : Trust a server's certificate)",
        " (paleta: Trust a server's certificate)",
        " (paletă: Trust a server's certificate)";
    TrustWhichCertificate:
        "Trust which certificate?",
        "Faire confiance à quel certificat ?",
        "¿En qué certificado confiar?",
        "Ce certificat acceptați?";
    ForgetWhichCertificate:
        "Forget which certificate?",
        "Oublier quel certificat ?",
        "¿Qué certificado olvidar?",
        "Ce certificat uitați?";
    CertForAccount:
        "{subject} — for {email}",
        "{subject} — pour {email}",
        "{subject} — para {email}",
        "{subject} — pentru {email}";
    CertSelfSigned:
        "Self-signed",
        "Auto-signé",
        "Autofirmado",
        "Autosemnat";
    CertIssuer:
        "Issued by {issuer}",
        "Émis par {issuer}",
        "Emitido por {issuer}",
        "Emis de {issuer}";
    CertValidity:
        "Valid from {from} to {until}",
        "Valide du {from} au {until}",
        "Válido del {from} al {until}",
        "Valabil de la {from} până la {until}";
    CertFingerprint:
        "SHA-256 {sha256}",
        "SHA-256 {sha256}",
        "SHA-256 {sha256}",
        "SHA-256 {sha256}";
    SendFromWhichAddressEmptyTheAccount:
        "Send from which address? (empty: the account's own)",
        "Envoyer depuis quelle adresse ? (vide : celle du compte)",
        "¿Enviar desde qué dirección? (vacío: la de la cuenta)",
        "Trimiteți de la ce adresă? (gol: cea a contului)";
    SendFromWhichAddress:
        "Send from which address?",
        "Envoyer depuis quelle adresse ?",
        "¿Enviar desde qué dirección?",
        "Trimiteți de la ce adresă?";
    ListContactsMatching:
        "List contacts matching…",
        "Lister les contacts correspondant à…",
        "Listar los contactos que coinciden con…",
        "Listați contactele care se potrivesc cu…";
    ANameOrAnAddress:
        "a name or an address",
        "un nom ou une adresse",
        "un nombre o una dirección",
        "un nume sau o adresă";
    SelectWhichConversation:
        "Select which conversation?",
        "Sélectionner quelle discussion ?",
        "¿Seleccionar qué conversación?",
        "Selectați ce conversație?";
    OpenWhichConversation:
        "Open which conversation?",
        "Ouvrir quelle discussion ?",
        "¿Abrir qué conversación?",
        "Deschideți ce conversație?";
    ShowImagesFromWhichSender:
        "Show images from which sender?",
        "Afficher les images de quel expéditeur ?",
        "¿Mostrar las imágenes de qué remitente?",
        "Afișați imaginile de la ce expeditor?";
    HideImagesFromWhichSender:
        "Hide images from which sender?",
        "Masquer les images de quel expéditeur ?",
        "¿Ocultar las imágenes de qué remitente?",
        "Ascundeți imaginile de la ce expeditor?";
    SaveTheAttachmentsIntoWhichFolder:
        "Save the attachments into which folder?",
        "Enregistrer les pièces jointes dans quel dossier ?",
        "¿Guardar los adjuntos en qué carpeta?",
        "Salvați atașamentele în ce dosar?";
    AnAbsolutePathEGHomeMe:
        "an absolute path, e.g. /home/me/Downloads",
        "un chemin absolu, p. ex. /home/moi/Téléchargements",
        "una ruta absoluta, p. ej. /home/yo/Descargas",
        "o cale absolută, de ex. /home/eu/Descărcări";
    MoveWhichConversation:
        "Move which conversation?",
        "Déplacer quelle discussion ?",
        "¿Mover qué conversación?",
        "Mutați ce conversație?";
    DiscardWhichChange:
        "Discard which change?",
        "Abandonner quelle modification ?",
        "¿Descartar qué cambio?",
        "Renunțați la ce modificare?";
    AttachWhichFile:
        "Attach which file?",
        "Joindre quel fichier ?",
        "¿Adjuntar qué archivo?",
        "Atașați ce fișier?";
    AbsolutePathEGHomeMeReport:
        "absolute path, e.g. /home/me/report.pdf",
        "chemin absolu, p. ex. /home/moi/rapport.pdf",
        "ruta absoluta, p. ej. /home/yo/informe.pdf",
        "cale absolută, de ex. /home/eu/raport.pdf";
    AttachToWhichDraft:
        "Attach to which draft?",
        "Joindre à quel brouillon ?",
        "¿Adjuntar a qué borrador?",
        "Atașați la ce ciornă?";
    RemoveWhichAttachment:
        "Remove which attachment?",
        "Supprimer quelle pièce jointe ?",
        "¿Quitar qué adjunto?",
        "Eliminați ce atașament?";
    FromWhichDraft:
        "From which draft?",
        "De quel brouillon ?",
        "¿De qué borrador?",
        "Din ce ciornă?";
    SearchMailResultsAsData:
        "Search mail (results as data)",
        "Rechercher dans le courrier (résultats en données)",
        "Buscar en el correo (resultados como datos)",
        "Căutați în e-mailuri (rezultate ca date)";
    Search:
        "Search",
        "Rechercher",
        "Buscar",
        "Căutați";
    FocusWhichPane:
        "Focus which pane?",
        "Activer quel panneau ?",
        "¿Activar qué panel?",
        "Activați ce panou?";
    FocusOnWhichMailbox:
        "Focus on which mailbox?",
        "Focus sur quel dossier ?",
        "¿Centrarse en qué carpeta?",
        "Concentrare pe ce dosar?";
    NameOfTheNewFolder:
        "Name of the new folder?",
        "Nom du nouveau dossier ?",
        "¿Nombre de la carpeta nueva?",
        "Numele dosarului nou?";
    APathLikeWorkClients:
        "a path like Work/Clients",
        "un chemin comme Travail/Clients",
        "una ruta como Trabajo/Clientes",
        "o cale precum Serviciu/Clienți";
    RenameWhichFolder:
        "Rename which folder?",
        "Renommer quel dossier ?",
        "¿Cambiar el nombre de qué carpeta?",
        "Redenumiți ce dosar?";
    DeleteWhichFolder:
        "Delete which folder?",
        "Supprimer quel dossier ?",
        "¿Eliminar qué carpeta?",
        "Ștergeți ce dosar?";
    PinWhichMailbox:
        "Pin which mailbox?",
        "Épingler quel dossier ?",
        "¿Fijar qué carpeta?",
        "Fixați ce dosar?";
    UnpinWhichMailbox:
        "Unpin which mailbox?",
        "Détacher quel dossier ?",
        "¿Desfijar qué carpeta?",
        "Anulați fixarea cărui dosar?";
    HideWhichMailbox:
        "Hide which mailbox?",
        "Masquer quel dossier ?",
        "¿Ocultar qué carpeta?",
        "Ascundeți ce dosar?";
    ShowWhichHiddenMailbox:
        "Show which hidden mailbox?",
        "Afficher quel dossier masqué ?",
        "¿Mostrar qué carpeta oculta?",
        "Afișați ce dosar ascuns?";
    MarkWhichMailboxRead:
        "Mark which mailbox read?",
        "Marquer quel dossier comme lu ?",
        "¿Marcar qué carpeta como leída?",
        "Marcați ce dosar ca citit?";
    ShowOnlyWhichThreads:
        "Show only which threads?",
        "Afficher seulement quelles discussions ?",
        "¿Mostrar solo qué conversaciones?",
        "Afișați doar ce conversații?";
    All:
        "all",
        "toutes",
        "todas",
        "toate";
    CheckForUpdates:
        "Check for updates?",
        "Rechercher des mises à jour ?",
        "¿Buscar actualizaciones?",
        "Căutați actualizări?";
    OnOrOffThenHoursBetweenChecks:
        "on or off, then hours between checks: on 24",
        "on ou off, puis les heures entre deux vérifications : on 24",
        "on u off, y luego las horas entre búsquedas: on 24",
        "on sau off, apoi orele dintre verificări: on 24";
    ApproveWhichSendRequest:
        "Approve which send request?",
        "Approuver quelle demande d’envoi ?",
        "¿Aprobar qué solicitud de envío?",
        "Aprobați ce cerere de trimitere?";
    RejectWhichSendRequest:
        "Reject which send request?",
        "Rejeter quelle demande d’envoi ?",
        "¿Rechazar qué solicitud de envío?",
        "Respingeți ce cerere de trimitere?";
    TypeThenEnter:
        "Type, then Enter",
        "Saisissez, puis Entrée",
        "Escribe y pulsa Intro",
        "Tastați, apoi Enter";
    CannotBeUndone:
        "cannot be undone",
        "irréversible",
        "no se puede deshacer",
        "ireversibil";
    Filter:
        "Filter…",
        "Filtrer…",
        "Filtrar…",
        "Filtrați…";
    TypeACommandOrSearch:
        "Type a command or search…",
        "Saisissez une commande ou une recherche…",
        "Escribe un comando o una búsqueda…",
        "Tastați o comandă sau o căutare…";
    Recent:
        "Recent",
        "Récents",
        "Recientes",
        "Recente";
    Threads:
        "Threads",
        "Discussions",
        "Conversaciones",
        "Conversații";
    Date:
        "Date",
        "Date",
        "Fecha",
        "Data";
    Mailbox:
        "Mailbox",
        "Dossier",
        "Carpeta",
        "Dosar";
    Size:
        "Size",
        "Taille",
        "Tamaño",
        "Dimensiune";
    Flags:
        "Flags",
        "Indicateurs",
        "Indicadores",
        "Indicatori";
    Yes:
        "yes",
        "oui",
        "sí",
        "da";
    No:
        "no",
        "non",
        "no",
        "nu";
    None:
        "none",
        "aucune",
        "ninguno",
        "niciunul";
    OpenConversation:
        "Open conversation",
        "Ouvrir la discussion",
        "Abrir la conversación",
        "Deschideți conversația";
    LastMessage:
        "Last message",
        "Dernier message",
        "Último mensaje",
        "Ultimul mesaj";
    Query:
        "Query",
        "Requête",
        "Consulta",
        "Interogare";
    NoMailboxes:
        "No mailboxes",
        "Aucun dossier",
        "No hay carpetas",
        "Niciun dosar";
    NoContacts:
        "No contacts",
        "Aucun contact",
        "No hay contactos",
        "Niciun contact";
    NoPendingChanges:
        "No pending changes",
        "Aucune modification en attente",
        "No hay cambios pendientes",
        "Nicio modificare în așteptare";
    NoSendRequestsX:
        "No send requests",
        "Aucune demande d’envoi",
        "No hay solicitudes de envío",
        "Nicio cerere de trimitere";
    Testing:
        "Testing…",
        "Test…",
        "Probando…",
        "Se testează…";
    Address:
        "Address",
        "Adresse",
        "Dirección",
        "Adresă";
    Domain:
        "Domain",
        "Domaine",
        "Dominio",
        "Domeniu";
    FoundIn:
        "Found in",
        "Trouvé dans",
        "Encontrado en",
        "Găsit în";
    State:
        "State",
        "État",
        "Estado",
        "Stare";
    Value:
        "Value",
        "Valeur",
        "Valor",
        "Valoare";
    Daily:
        "Daily",
        "Quotidien",
        "Diaria",
        "Zilnic";
    Weekly:
        "Weekly",
        "Hebdomadaire",
        "Semanal",
        "Săptămânal";
    Monthly:
        "Monthly",
        "Mensuel",
        "Mensual",
        "Lunar";
    CheckNow:
        "Check now",
        "Vérifier maintenant",
        "Buscar ahora",
        "Verificați acum";
    Download:
        "Download",
        "Télécharger",
        "Descargar",
        "Descărcați";
    Install:
        "Install…",
        "Installer…",
        "Instalar…",
        "Instalați…";
    SkipThisVersion:
        "Skip this version",
        "Ignorer cette version",
        "Omitir esta versión",
        "Omiteți această versiune";
    ReleaseNotesX:
        "Release notes",
        "Notes de version",
        "Notas de la versión",
        "Note de lansare";
    ThisBuildDoesNotCheckForUpdates:
        "This build does not check for updates",
        "Cette version ne vérifie pas les mises à jour",
        "Esta compilación no busca actualizaciones",
        "Această versiune nu caută actualizări";
    DailIsUpToDate:
        "dail is up to date",
        "dail est à jour",
        "dail está actualizado",
        "dail este la zi";
    NotCheckedYet:
        "Not checked yet",
        "Pas encore vérifié",
        "Aún no se ha comprobado",
        "Încă neverificat";
    CheckingForUpdates:
        "Checking for updates…",
        "Recherche de mises à jour…",
        "Buscando actualizaciones…",
        "Se caută actualizări…";
    RestartToUpdate:
        "Restart to update",
        "Redémarrer pour mettre à jour",
        "Reiniciar para actualizar",
        "Reporniți pentru actualizare";
    Checked:
        "checked",
        "cochée",
        "marcada",
        "bifată";
    UnreadX:
        "unread",
        "non lu",
        "no leído",
        "necitit";
    StarredX:
        "starred",
        "suivi",
        "destacado",
        "cu stea";
    PinnedX:
        "pinned",
        "épinglé",
        "fijado",
        "fixat";
    WithAttachments:
        "with attachments",
        "avec pièces jointes",
        "con adjuntos",
        "cu atașamente";
    Unnamed:
        "(unnamed)",
        "(sans nom)",
        "(sin nombre)",
        "(fără nume)";
    AccountX:
        "account",
        "compte",
        "cuenta",
        "cont";
    AndMore:
        " and {n} more",
        " et {n} autres",
        " y {n} más",
        " și încă {n}";
    NewMessages:
        "{n} new messages",
        "{n} nouveaux messages",
        "{n} mensajes nuevos",
        "{n} mesaje noi";
    MessagesCouldNotBeSent:
        "{n} messages could not be sent",
        "{n} messages n’ont pas pu être envoyés",
        "No se pudieron enviar {n} mensajes",
        "{n} mesaje nu au putut fi trimise";
    ToX:
        "To {to}: {subject}",
        "À {to} : {subject}",
        "Para {to}: {subject}",
        "Către {to}: {subject}";
    AgentsWantToSendMessages:
        "Agents want to send {n} messages",
        "Des agents veulent envoyer {n} messages",
        "Unos agentes quieren enviar {n} mensajes",
        "Niște agenți vor să trimită {n} mesaje";
    SendingInS:
        "sending in {n} s",
        "envoi dans {n} s",
        "enviando en {n} s",
        "se trimite în {n} s";
    InS:
        "in {n}s",
        "dans {n} s",
        "en {n} s",
        "în {n} s";
    InM:
        "in {n}m",
        "dans {n} min",
        "en {n} min",
        "în {n} min";
    InH:
        "in {n}h",
        "dans {n} h",
        "en {n} h",
        "în {n} h";
    MAgo:
        "{n}m ago",
        "il y a {n} min",
        "hace {n} min",
        "acum {n} min";
    HAgo:
        "{n}h ago",
        "il y a {n} h",
        "hace {n} h",
        "acum {n} h";
    DAgo:
        "{n}d ago",
        "il y a {n} j",
        "hace {n} d",
        "acum {n} z";
    SendingInSX:
        "Sending in {n} s",
        "Envoi dans {n} s",
        "Enviando en {n} s",
        "Se trimite în {n} s";
    RemoveAndItsPassword:
        "Remove {email} and its password",
        "Supprimer {email} et son mot de passe",
        "Quitar {email} y su contraseña",
        "Eliminați {email} și parola sa";
    InstallDail:
        "Install dail {version}",
        "Installer dail {version}",
        "Instalar dail {version}",
        "Instalați dail {version}";
    NewFolderIn:
        "New folder in {path}",
        "Nouveau dossier dans {path}",
        "Carpeta nueva en {path}",
        "Dosar nou în {path}";
    NowX:
        "now {now}",
        "actuellement {now}",
        "ahora {now}",
        "acum {now}";
    DeleteX:
        "Delete {path}",
        "Supprimer {path}",
        "Eliminar {path}",
        "Ștergeți {path}";
    MarkAllUnreadMessagesReadUndoRestores:
        "Mark all {n} unread messages read? Undo restores only the cached ones.",
        "Marquer les {n} messages non lus comme lus ? L’annulation ne restaure que ceux en cache.",
        "¿Marcar los {n} mensajes no leídos como leídos? Deshacer solo restaura los que están en caché.",
        "Marcați toate cele {n} mesaje necitite ca citite? Anularea le restabilește doar pe cele din cache.";
    EmptyTrashOfForever:
        "Delete the {n} messages in the Trash of {email} forever? This cannot be undone.",
        "Supprimer définitivement les {n} messages de la corbeille de {email} ? C’est irréversible.",
        "¿Eliminar para siempre los {n} mensajes de la papelera de {email}? No se puede deshacer.",
        "Ștergeți definitiv cele {n} mesaje din coșul de gunoi al {email}? Acțiunea nu poate fi anulată.";
    EmptyTrash:
        "Empty Trash",
        "Vider la corbeille",
        "Vaciar la papelera",
        "Goliți coșul de gunoi";
    EmptyTrashN:
        "Empty Trash ({n})",
        "Vider la corbeille ({n})",
        "Vaciar la papelera ({n})",
        "Goliți coșul de gunoi ({n})";
    EmptyTrashOfWhichAccount:
        "Empty the Trash of which account?",
        "Vider la corbeille de quel compte ?",
        "¿Vaciar la papelera de qué cuenta?",
        "Goliți coșul de gunoi al cărui cont?";
    EmptyJunk:
        "Empty Junk",
        "Vider les indésirables",
        "Vaciar el spam",
        "Goliți dosarul spam";
    EmptyJunkN:
        "Empty Junk ({n})",
        "Vider les indésirables ({n})",
        "Vaciar el spam ({n})",
        "Goliți dosarul spam ({n})";
    EmptyJunkOfForever:
        "Delete the {n} messages in the Junk folder of {email} forever? This cannot be undone.",
        "Supprimer définitivement les {n} messages indésirables de {email} ? C’est irréversible.",
        "¿Eliminar para siempre los {n} mensajes de la carpeta de spam de {email}? No se puede deshacer.",
        "Ștergeți definitiv cele {n} mesaje din dosarul spam al {email}? Acțiunea nu poate fi anulată.";
    EmptyJunkOfWhichAccount:
        "Empty the Junk folder of which account?",
        "Vider les indésirables de quel compte ?",
        "¿Vaciar la carpeta de spam de qué cuenta?",
        "Goliți dosarul spam al cărui cont?";
    EmptyMailboxOp:
        "empty {mailbox}",
        "vider {mailbox}",
        "vaciar {mailbox}",
        "golire {mailbox}";
    MarkMessagesRead:
        "Mark {n} messages read",
        "Marquer {n} messages comme lus",
        "Marcar {n} mensajes como leídos",
        "Marcați {n} mesaje ca citite";
    NewValueFor:
        "New value for {key}",
        "Nouvelle valeur pour {key}",
        "Valor nuevo para {key}",
        "Valoare nouă pentru {key}";
    NowXX:
        "{allowed}, now {current}",
        "{allowed}, actuellement {current}",
        "{allowed}, ahora {current}",
        "{allowed}, acum {current}";
    GoTo:
        "Go to {name}",
        "Aller à {name}",
        "Ir a {name}",
        "Mergeți la {name}";
    Use:
        "Use “{query}”",
        "Utiliser « {query} »",
        "Usar «{query}»",
        "Folosiți „{query}”";
    SearchMailFor:
        "Search mail for “{query}”",
        "Rechercher « {query} » dans le courrier",
        "Buscar «{query}» en el correo",
        "Căutați „{query}” în e-mailuri";
    UnreadXX:
        "{unread} unread · {total}",
        "{unread} non lus · {total}",
        "{unread} no leídos · {total}",
        "{unread} necitite · {total}";
    Attempts:
        "{state} · {n} attempts",
        "{state} · {n} tentatives",
        "{state} · {n} intentos",
        "{state} · {n} încercări";
    ToName:
        "To {to}",
        "Pour {to}",
        "Para {to}",
        "Către {to}";
    SignedIn:
        "Signed in {age}",
        "Connecté {age}",
        "Sesión iniciada {age}",
        "Autentificat {age}";
    DailIsAvailable:
        "dail {version} is available",
        "dail {version} est disponible",
        "dail {version} está disponible",
        "dail {version} este disponibil";
    DownloadingDail:
        "Downloading dail {version}…",
        "Téléchargement de dail {version}…",
        "Descargando dail {version}…",
        "Se descarcă dail {version}…";
    DailIsDownloadedAndVerified:
        "dail {version} is downloaded and verified",
        "dail {version} est téléchargé et vérifié",
        "dail {version} está descargado y verificado",
        "dail {version} este descărcat și verificat";
    InstallingDail:
        "Installing dail {version}…",
        "Installation de dail {version}…",
        "Instalando dail {version}…",
        "Se instalează dail {version}…";
    DailIsInstalledRestartDailToUse:
        "dail {version} is installed: restart dail to use it",
        "dail {version} est installé : redémarrez dail pour l’utiliser",
        "dail {version} está instalado: reinicia dail para usarlo",
        "dail {version} este instalat: reporniți dail pentru a-l folosi";
    UpdateFailed:
        "Update failed: {reason}",
        "Échec de la mise à jour : {reason}",
        "Error al actualizar: {reason}",
        "Actualizarea a eșuat: {reason}";
    CheckedX:
        "checked {age}",
        "vérifié {age}",
        "comprobado {age}",
        "verificat {age}";
    Update:
        "Update {version}",
        "Mettre à jour vers {version}",
        "Actualizar a {version}",
        "Actualizați la {version}";
    EveryH:
        "every {n} h",
        "toutes les {n} h",
        "cada {n} h",
        "la fiecare {n} h";
    UnreadXXX:
        "{label}, {n} unread",
        "{label}, {n} non lus",
        "{label}, {n} no leídos",
        "{label}, {n} necitite";
    UpdateQuestion:
        "Check for new versions of dail?",
        "Rechercher les nouvelles versions de dail ?",
        "¿Buscar versiones nuevas de dail?",
        "Căutați versiuni noi de dail?";
    UpdateQuestionDetail:
        "Once a day dail asks GitHub for the signed release list. Nothing about you or your mail is sent. You can change this in Settings.",
        "Une fois par jour, dail demande à GitHub la liste signée des versions. Rien sur vous ou votre courrier n’est envoyé. Vous pouvez changer cela dans les paramètres.",
        "Una vez al día, dail pide a GitHub la lista firmada de versiones. No se envía nada sobre ti ni sobre tu correo. Puedes cambiarlo en Ajustes.",
        "O dată pe zi, dail cere de la GitHub lista semnată de versiuni. Nu se trimite nimic despre dvs. sau despre e-mailurile dvs. Puteți schimba asta în Setări.";
    LanguageSetting:
        "Language",
        "Langue",
        "Idioma",
        "Limbă";
    LanguageHint:
        "The language of the app",
        "La langue de l’application",
        "El idioma de la aplicación",
        "Limba aplicației";
    AddAnAccountInDailBeforeSharing:
        "Add an account in dail before sharing to it.",
        "Ajoutez un compte dans dail avant d’y partager.",
        "Añade una cuenta en dail antes de compartir con ella.",
        "Adăugați un cont în dail înainte de a partaja către el.";
    RestorePanes:
        "Restore panes",
        "Restaurer les panneaux",
        "Restaurar los paneles",
        "Restabiliți panourile";
    MaximizeMessage:
        "Maximize message",
        "Agrandir le message",
        "Maximizar el mensaje",
        "Maximizați mesajul";
    About:
        "About",
        "À propos",
        "Acerca de",
        "Despre";
    ShowFolderNames:
        "Show folder names",
        "Afficher les noms des dossiers",
        "Mostrar los nombres de las carpetas",
        "Afișați numele dosarelor";
    IconsOnly:
        "Icons only",
        "Icônes seules",
        "Solo iconos",
        "Doar pictograme";
    NoAccountsYetAddOneToRead:
        "No accounts yet. Add one to read your mail here.",
        "Aucun compte pour l’instant. Ajoutez-en un pour lire votre courrier ici.",
        "Aún no hay cuentas. Añade una para leer tu correo aquí.",
        "Încă nu există conturi. Adăugați unul pentru a vă citi e-mailurile aici.";
    PickAMailboxFromTheMenu:
        "Pick a mailbox from the menu.",
        "Choisissez un dossier dans le menu.",
        "Elige una carpeta en el menú.",
        "Alegeți un dosar din meniu.";
    ForwardedMessage:
        "Forwarded message",
        "Message transféré",
        "Mensaje reenviado",
        "Mesaj redirecționat";
    QuotedMessage:
        "Quoted message",
        "Message cité",
        "Mensaje citado",
        "Mesaj citat";
    Check:
        "Check",
        "Vérifier",
        "Comprobar",
        "Verificați";
    Open:
        "Open",
        "Ouvrir",
        "Abrir",
        "Deschideți";
    Skip:
        "Skip",
        "Ignorer",
        "Omitir",
        "Omiteți";
    NewMail:
        "New mail",
        "Nouveau courrier",
        "Correo nuevo",
        "E-mail nou";
    AMessageWasNotSent:
        "A message was not sent",
        "Un message n’a pas été envoyé",
        "No se envió un mensaje",
        "Un mesaj nu a fost trimis";
    NewMailArrivesAsItComes:
        "New mail arrives as it comes",
        "Le courrier arrive au fil de l’eau",
        "El correo nuevo llega en cuanto se recibe",
        "E-mailurile noi sosesc pe măsură ce vin";
    Stop:
        "Stop",
        "Arrêter",
        "Detener",
        "Opriți";
    Battery:
        "Battery",
        "Batterie",
        "Batería",
        "Baterie";
    ConnectingToYourInboxes:
        "Connecting to your inboxes",
        "Connexion à vos comptes",
        "Conectando con tus bandejas de entrada",
        "Conectare la căsuțele dvs. de intrare";
    Watching1Inbox:
        "Watching 1 inbox",
        "Surveillance d’un dossier",
        "Vigilando 1 bandeja de entrada",
        "Se urmărește 1 căsuță de intrare";
    QuotedTextX:
        "Quoted text",
        "Texte cité",
        "Texto citado",
        "Text citat";
    SignatureX:
        "Signature",
        "Signature",
        "Firma",
        "Semnătură";
    Move:
        "Move…",
        "Déplacer…",
        "Mover…",
        "Mutați…";
    CloseMessage:
        "Close message",
        "Fermer le message",
        "Cerrar el mensaje",
        "Închideți mesajul";
    PreviousConversationX:
        "Previous conversation",
        "Discussion précédente",
        "Conversación anterior",
        "Conversația anterioară";
    NextConversationX:
        "Next conversation",
        "Discussion suivante",
        "Conversación siguiente",
        "Conversația următoare";
    Copy:
        "Copy",
        "Copier",
        "Copiar",
        "Copiați";
    CopyAllText:
        "Copy all text",
        "Copier tout le texte",
        "Copiar todo el texto",
        "Copiați tot textul";
    LinkCopied:
        "Link copied.",
        "Lien copié.",
        "Enlace copiado.",
        "Link copiat.";
    Save:
        "Save",
        "Enregistrer",
        "Guardar",
        "Salvați";
    DailDoesNotShowThisKindOf:
        "dail does not show this kind of file. Save it to open it in another app.",
        "dail n’affiche pas ce type de fichier. Enregistrez-le pour l’ouvrir dans une autre application.",
        "dail no muestra este tipo de archivo. Guárdalo para abrirlo en otra aplicación.",
        "dail nu afișează acest tip de fișier. Salvați-l pentru a-l deschide în altă aplicație.";
    ThisPictureCouldNotBeShown:
        "This picture could not be shown.",
        "Cette image n’a pas pu être affichée.",
        "No se pudo mostrar esta imagen.",
        "Această imagine nu a putut fi afișată.";
    ThisPageCouldNotBeShown:
        "This page could not be shown.",
        "Cette page n’a pas pu être affichée.",
        "No se pudo mostrar esta página.",
        "Această pagină nu a putut fi afișată.";
    TheFileCouldNotBeRead:
        "The file could not be read.",
        "Le fichier n’a pas pu être lu.",
        "No se pudo leer el archivo.",
        "Fișierul nu a putut fi citit.";
    TheMessageIsNotDownloadedYet:
        "The message is not downloaded yet.",
        "Le message n’est pas encore téléchargé.",
        "El mensaje aún no se ha descargado.",
        "Mesajul nu este descărcat încă.";
    On:
        "On",
        "Activé",
        "Activado",
        "Activat";
    CozyX:
        "Cozy",
        "Détendue",
        "Holgada",
        "Aerisită";
    CompactX:
        "Compact",
        "Compacte",
        "Compacta",
        "Compactă";
    OnlyWhileOpen:
        "Only while open",
        "Seulement ouverte",
        "Solo mientras está abierta",
        "Doar cât este deschisă";
    Periodic:
        "Periodic",
        "Périodique",
        "Periódica",
        "Periodic";
    Push:
        "Push",
        "Push",
        "Push",
        "Push";
    CheckTheInboxEvery:
        "Check the inbox every",
        "Relever le dossier toutes les",
        "Comprobar la bandeja de entrada cada",
        "Verificați căsuța de intrare la fiecare";
    MessagesFetchedFirst:
        "Messages fetched first",
        "Messages récupérés d’abord",
        "Mensajes descargados primero",
        "Mesaje descărcate inițial";
    ListDensity:
        "List density",
        "Densité de la liste",
        "Densidad de la lista",
        "Densitatea listei";
    MailChecksInTheBackground:
        "Mail checks in the background",
        "Relève en arrière-plan",
        "Comprobaciones de correo en segundo plano",
        "Verificarea e-mailurilor în fundal";
    CheckEvery:
        "Check every",
        "Vérifier toutes les",
        "Comprobar cada",
        "Verificați la fiecare";
    TerminalColours:
        "Terminal colours",
        "Couleurs du terminal",
        "Colores del terminal",
        "Culorile terminalului";
    MarkAsReadAfter:
        "Mark as read after",
        "Marquer comme lu après",
        "Marcar como leído tras",
        "Marcați ca citit după";
    RefreshAMailboxOnOpenAfter:
        "Refresh a mailbox on open after",
        "Actualiser un dossier à l’ouverture après",
        "Actualizar una carpeta al abrirla tras",
        "Actualizați un dosar la deschidere după";
    SyncEveryFolderEvery:
        "Sync every folder every",
        "Synchroniser tous les dossiers toutes les",
        "Sincronizar todas las carpetas cada",
        "Sincronizați toate dosarele la fiecare";
    AfterArchiving:
        "After archiving",
        "Après l’archivage",
        "Tras archivar",
        "După arhivare";
    HowLongASendCanStillBe:
        "How long a send can still be cancelled.",
        "Le temps pendant lequel un envoi peut être annulé.",
        "Cuánto tiempo se puede cancelar aún un envío.",
        "Cât timp mai poate fi anulată o trimitere.";
    WhileTheAppIsOpen:
        "While the app is open.",
        "Tant que l’application est ouverte.",
        "Mientras la aplicación está abierta.",
        "Cât timp aplicația este deschisă.";
    NewMailAndFailedSendsWhileDail:
        "New mail and failed sends, while dail is off screen.",
        "Nouveau courrier et envois en échec, quand dail n’est pas à l’écran.",
        "Correo nuevo y envíos fallidos, mientras dail no está en pantalla.",
        "E-mailuri noi și trimiteri eșuate, cât timp dail nu este pe ecran.";
    PerMailboxOnItsFirstSync:
        "Per mailbox, on its first sync.",
        "Par dossier, à sa première synchronisation.",
        "Por carpeta, en su primera sincronización.",
        "Pe dosar, la prima sincronizare.";
    Mode:
        "Mode",
        "Mode",
        "Modo",
        "Mod";
    SystemFollowsAndroidSDarkTheme:
        "System follows Android's dark theme.",
        "Système suit le thème sombre d’Android.",
        "Sistema sigue el tema oscuro de Android.",
        "Sistem urmează tema întunecată din Android.";
    TextSizeFollowsAndroidSFontSize:
        "Text size follows Android's font size.",
        "La taille du texte suit celle d’Android.",
        "El tamaño del texto sigue el tamaño de fuente de Android.",
        "Dimensiunea textului urmează dimensiunea fontului din Android.";
    ChangeAKey:
        "Change a key…",
        "Changer une touche…",
        "Cambiar una tecla…",
        "Schimbați o tastă…";
    HardwareKeyboard:
        "Hardware keyboard",
        "Clavier physique",
        "Teclado físico",
        "Tastatură fizică";
    ForAKeyboardOnATabletOr:
        "For a keyboard on a tablet or a Chromebook: the desktop's keys.",
        "Pour un clavier sur tablette ou Chromebook : les touches du bureau.",
        "Para un teclado en una tableta o un Chromebook: las teclas del escritorio.",
        "Pentru o tastatură pe tabletă sau Chromebook: tastele de pe desktop.";
    Bindings:
        "Bindings",
        "Raccourcis",
        "Atajos",
        "Comenzi rapide";
    BatteryOptimization:
        "Battery optimization",
        "Optimisation de la batterie",
        "Optimización de batería",
        "Optimizarea bateriei";
    NotificationSettings:
        "Notification settings",
        "Paramètres de notification",
        "Ajustes de notificaciones",
        "Setări pentru notificări";
    SystemSettings:
        "System settings",
        "Paramètres système",
        "Ajustes del sistema",
        "Setări de sistem";
    BackgroundNotifications:
        "Background & notifications",
        "Arrière-plan et notifications",
        "Segundo plano y notificaciones",
        "Fundal și notificări";
    NotTestedX:
        "Not tested",
        "Non testé",
        "Sin probar",
        "Netestat";
    ConnectedX:
        "Connected",
        "Connecté",
        "Conectado",
        "Conectat";
    Remove:
        "Remove",
        "Retirer",
        "Quitar",
        "Eliminați";
    AnotherAccount:
        "Another account",
        "Un autre compte",
        "Otra cuenta",
        "Alt cont";
    GmailHostingerMailOrAnyIMAPAnd:
        "Gmail, Hostinger Mail or any IMAP and SMTP server.",
        "Gmail, Hostinger Mail ou tout serveur IMAP et SMTP.",
        "Gmail, Hostinger Mail o cualquier servidor IMAP y SMTP.",
        "Gmail, Hostinger Mail sau orice server IMAP și SMTP.";
    EnterTheServersYourselfOrFindThem:
        "Enter the servers yourself, or find them.",
        "Saisissez les serveurs vous-même, ou trouvez-les.",
        "Escribe los servidores tú o búscalos.",
        "Introduceți serverele manual sau găsiți-le.";
    ShownInTheDrawer:
        "shown in the drawer",
        "affiché dans le tiroir",
        "se muestra en el panel lateral",
        "afișat în sertar";
    View:
        "View",
        "Vue",
        "Vista",
        "Vizualizare";
    SearchingTheServer:
        "Searching the server…",
        "Recherche sur le serveur…",
        "Buscando en el servidor…",
        "Se caută pe server…";
    Selected:
        "{n} selected",
        "{n} sélectionnés",
        "{n} seleccionados",
        "{n} selectate";
    NotSentX:
        "Not sent: “{subject}”: {error}",
        "Non envoyé : « {subject} » : {error}",
        "No enviado: «{subject}»: {error}",
        "Netrimis: „{subject}”: {error}";
    NewMailInConversations:
        "New mail in {n} conversations",
        "Nouveau courrier dans {n} discussions",
        "Correo nuevo en {n} conversaciones",
        "E-mailuri noi în {n} conversații";
    WatchingInboxes:
        "Watching {n} inboxes",
        "Surveillance de {n} dossiers",
        "Vigilando {n} bandejas de entrada",
        "Se urmăresc {n} căsuțe de intrare";
    HideImagesFrom:
        "Hide images from {who}",
        "Masquer les images de {who}",
        "Ocultar las imágenes de {who}",
        "Ascundeți imaginile de la {who}";
    ShowImagesFrom:
        "Show images from {who}",
        "Afficher les images de {who}",
        "Mostrar las imágenes de {who}",
        "Afișați imaginile de la {who}";
    SaveAllX:
        "Save all ({n})",
        "Tout enregistrer ({n})",
        "Guardar todo ({n})",
        "Salvați tot ({n})";
    Failed:
        "Failed: {error}",
        "Échec : {error}",
        "Error: {error}",
        "Eșuat: {error}";
    TheServerSearchFailed:
        "The server search failed: {error}",
        "La recherche sur le serveur a échoué : {error}",
        "Error en la búsqueda en el servidor: {error}",
        "Căutarea pe server a eșuat: {error}";
    NativeSaved:
        "Saved.",
        "Enregistré.",
        "Guardado.",
        "Salvat.";
    NativeSavedN:
        "Saved {n} files.",
        "{n} fichiers enregistrés.",
        "{n} archivos guardados.",
        "{n} fișiere salvate.";
    NativeSavedPart:
        "Saved {n} of {m} files.",
        "{n} fichiers sur {m} enregistrés.",
        "{n} de {m} archivos guardados.",
        "{n} din {m} fișiere salvate.";
    NativeNoLinkApp:
        "No app can open this link.",
        "Aucune application ne peut ouvrir ce lien.",
        "Ninguna aplicación puede abrir este enlace.",
        "Nicio aplicație nu poate deschide acest link.";
    NativeNoSettings:
        "This phone has no such settings screen.",
        "Ce téléphone n’a pas cet écran de paramètres.",
        "Este teléfono no tiene esa pantalla de ajustes.",
        "Acest telefon nu are un astfel de ecran de setări.";
    NativeRelease:
        "Release to open",
        "Relâchez pour ouvrir",
        "Suelta para abrir",
        "Eliberați pentru a deschide";
    NativeSave:
        "Save {name}",
        "Enregistrer {name}",
        "Guardar {name}",
        "Salvați {name}";
    NativeOpenLink:
        "Open this link?",
        "Ouvrir ce lien ?",
        "¿Abrir este enlace?",
        "Deschideți acest link?";
    NativePage:
        "Page {n} of {m}",
        "Page {n} sur {m}",
        "Página {n} de {m}",
        "Pagina {n} din {m}";
    NativeSelect:
        "Select",
        "Sélectionner",
        "Seleccionar",
        "Selectați";
    NativeFailed:
        "Failed",
        "Échec",
        "Error",
        "Eșuat";
    NativePassed:
        "Passed",
        "Réussi",
        "Correcto",
        "Reușit";
    NativePending:
        "Pending",
        "En attente",
        "Pendiente",
        "În așteptare";
    NativeMoreActions:
        "More actions",
        "Plus d’actions",
        "Más acciones",
        "Mai multe acțiuni";
    NativeFolderActions:
        "Folder actions",
        "Actions sur le dossier",
        "Acciones de la carpeta",
        "Acțiuni pentru dosar";
    AboutDail:
        "About dail",
        "À propos de dail",
        "Acerca de dail",
        "Despre dail";
    AboutDailVersion:
        "About dail ({version})",
        "À propos de dail ({version})",
        "Acerca de dail ({version})",
        "Despre dail ({version})";
    AboutDesktopText:
        "Keyboard-first email client that agents can drive. Licence: {licence}.",
        "Client de messagerie piloté au clavier, que des agents peuvent commander. Licence : {licence}.",
        "Cliente de correo pensado para el teclado que los agentes pueden manejar. Licencia: {licence}.",
        "Client de e-mail gândit pentru tastatură, pe care îl pot comanda agenții. Licență: {licence}.";
    AboutMobileText:
        "Email client for IMAP and SMTP accounts ({licence}). The app bundles these typefaces, icons, PDF fonts, Rust crates and Android libraries under their licences (SPDX identifiers: name, version, licence).",
        "Client de messagerie pour comptes IMAP et SMTP ({licence}). L’application intègre ces polices, icônes, polices PDF, crates Rust et bibliothèques Android sous leurs licences (identifiants SPDX : nom, version, licence).",
        "Cliente de correo para cuentas IMAP y SMTP ({licence}). La aplicación incluye estas tipografías, iconos, fuentes PDF, crates de Rust y bibliotecas de Android bajo sus licencias (identificadores SPDX: nombre, versión, licencia).",
        "Client de e-mail pentru conturi IMAP și SMTP ({licence}). Aplicația include aceste fonturi, pictograme, fonturi PDF, crate-uri Rust și biblioteci Android, sub licențele lor (identificatori SPDX: nume, versiune, licență).";
    SourceCode:
        "Source code",
        "Code source",
        "Código fuente",
        "Cod sursă";
    Releases:
        "Releases",
        "Versions",
        "Versiones",
        "Versiuni";
    RustCrates:
        "Rust crates",
        "Crates Rust",
        "Crates de Rust",
        "Crate-uri Rust";
    AndroidLibraries:
        "Android libraries",
        "Bibliothèques Android",
        "Bibliotecas de Android",
        "Biblioteci Android";
    PdfFontsAndCmaps:
        "PDF fonts and CMaps",
        "Polices PDF et CMaps",
        "Fuentes PDF y CMaps",
        "Fonturi PDF și CMap-uri";
    PdfCraftViewer:
        "PdfCraft for Android (adapted viewer)",
        "PdfCraft pour Android (visionneuse adaptée)",
        "PdfCraft para Android (visor adaptado)",
        "PdfCraft pentru Android (vizualizator adaptat)";
    SharedRefused:
        "{n} shared files could not be attached: only files shared by other apps can.",
        "{n} fichiers partagés n’ont pas pu être joints : seuls les fichiers partagés par d’autres applications peuvent l’être.",
        "No se pudieron adjuntar {n} archivos compartidos: solo se pueden los que comparten otras aplicaciones.",
        "{n} fișiere partajate nu au putut fi atașate: se pot atașa doar fișierele partajate de alte aplicații.";
    SharedRefusedOne:
        "1 shared file could not be attached: only files shared by other apps can.",
        "1 fichier partagé n’a pas pu être joint : seuls les fichiers partagés par d’autres applications peuvent l’être.",
        "No se pudo adjuntar 1 archivo compartido: solo se pueden los que comparten otras aplicaciones.",
        "1 fișier partajat nu a putut fi atașat: se pot atașa doar fișierele partajate de alte aplicații.";
    LoginFailedColon:
        "The login failed: {error}",
        "La connexion a échoué : {error}",
        "Error al iniciar sesión: {error}",
        "Autentificarea a eșuat: {error}";
    HintUp:
        "up",
        "haut",
        "arriba",
        "sus";
    HintDown:
        "down",
        "bas",
        "abajo",
        "jos";
    WhichKeys:
        "Which keys?",
        "Quels raccourcis ?",
        "¿Qué atajos?",
        "Ce comenzi rapide?";
    GmailOrVim:
        "gmail or vim",
        "gmail ou vim",
        "gmail o vim",
        "gmail sau vim";
    FilterHint:
        "unread, starred, attachments (empty: all); now: {now}",
        "non lu, suivi, pièces jointes (vide : tout) ; actuellement : {now}",
        "no leído, destacado, adjuntos (vacío: todo); ahora: {now}",
        "necitit, cu stea, atașamente (gol: tot); acum: {now}";
    AttachmentsX:
        "attachments",
        "pièces jointes",
        "adjuntos",
        "atașamente";
    ReadFlag:
        "read",
        "lu",
        "leído",
        "citit";
    AnsweredFlag:
        "answered",
        "répondu",
        "respondido",
        "răspuns";
    WindowN:
        " · window {n}",
        " · fenêtre {n}",
        " · ventana {n}",
        " · fereastră {n}";
    SetFlagOn:
        "set {flag} on {n} message(s)",
        "activer {flag} sur {n} message(s)",
        "activar {flag} en {n} mensaje(s)",
        "setați {flag} pe {n} mesaj(e)";
    ClearFlagOn:
        "clear {flag} on {n} message(s)",
        "retirer {flag} sur {n} message(s)",
        "quitar {flag} en {n} mensaje(s)",
        "eliminați {flag} de pe {n} mesaj(e)";
    SetFlagAll:
        "set {flag} on all of {mailbox}",
        "activer {flag} sur tout {mailbox}",
        "activar {flag} en todo {mailbox}",
        "setați {flag} pe tot {mailbox}";
    ClearFlagAll:
        "clear {flag} on all of {mailbox}",
        "retirer {flag} sur tout {mailbox}",
        "quitar {flag} en todo {mailbox}",
        "eliminați {flag} de pe tot {mailbox}";
    NoPreviewFor:
        "no preview for {type}",
        "pas d’aperçu pour {type}",
        "sin vista previa para {type}",
        "fără previzualizare pentru {type}";
    ImageNotInMessage:
        "the image is not in the message",
        "l’image n’est pas dans le message",
        "la imagen no está en el mensaje",
        "imaginea nu este în mesaj";
    NotAnImage:
        "{type} is not an image",
        "{type} n’est pas une image",
        "{type} no es una imagen",
        "{type} nu este o imagine";
    AttachmentNotStored:
        "the attachment is not in the local store",
        "la pièce jointe n’est pas dans le stockage local",
        "el adjunto no está en el almacenamiento local",
        "atașamentul nu este în stocarea locală";
    CouldNotOpenBrowser:
        "Could not open the browser: {error}",
        "Impossible d’ouvrir le navigateur : {error}",
        "No se pudo abrir el navegador: {error}",
        "Browserul nu a putut fi deschis: {error}";
    MessageGone:
        "The message is not in the local store any more.",
        "Le message n’est plus dans le stockage local.",
        "El mensaje ya no está en el almacenamiento local.",
        "Mesajul nu mai este în stocarea locală.";
    AttachmentGone:
        "The attachment is not in the local store any more.",
        "La pièce jointe n’est plus dans le stockage local.",
        "El adjunto ya no está en el almacenamiento local.",
        "Atașamentul nu mai este în stocarea locală.";
    CouldNotWriteOriginal:
        "Could not write the original: {error}",
        "Impossible d’écrire l’original : {error}",
        "No se pudo escribir el original: {error}",
        "Originalul nu a putut fi scris: {error}";
    CouldNotWriteAttachment:
        "Could not write the attachment: {error}",
        "Impossible d’écrire la pièce jointe : {error}",
        "No se pudo escribir el adjunto: {error}",
        "Atașamentul nu a putut fi scris: {error}";
    NoEditor:
        "no editor",
        "aucun éditeur",
        "ningún editor",
        "niciun editor";
    EditorExited:
        "{program} exited with {status}",
        "{program} s’est terminé avec {status}",
        "{program} terminó con {status}",
        "{program} s-a încheiat cu {status}";
    TerminalCantDraw:
        "the terminal can't be drawn: {error}",
        "impossible de dessiner le terminal : {error}",
        "no se puede dibujar el terminal: {error}",
        "terminalul nu poate fi desenat: {error}";
    InputClosed:
        "input closed",
        "entrée fermée",
        "entrada cerrada",
        "intrare închisă";
    TerminalWentAway:
        "the terminal went away",
        "le terminal a disparu",
        "el terminal desapareció",
        "terminalul a dispărut";
    ReceivedSignal:
        "received {name}",
        "{name} reçu",
        "se recibió {name}",
        "s-a primit {name}";
    ConsoleClose:
        "console close",
        "fermeture de la console",
        "cierre de la consola",
        "închiderea consolei";
    UnexpectedReceipt:
        "unexpected receipt: {error}",
        "accusé de réception inattendu : {error}",
        "acuse inesperado: {error}",
        "confirmare neașteptată: {error}";
    ToNameLower:
        "to {name}",
        "à {name}",
        "para {name}",
        "către {name}";
    SyncedSettings:
        "Settings on the server",
        "Paramètres sur le serveur",
        "Ajustes en el servidor",
        "Setări pe server";
    SettingsAccount:
        "Settings account",
        "Compte des paramètres",
        "Cuenta de los ajustes",
        "Contul setărilor";
    SettingsAccountHint:
        "Its hidden dail/Settings folder holds the settings shared by your devices",
        "Son dossier caché dail/Settings garde les paramètres partagés par vos appareils",
        "Su carpeta oculta dail/Settings guarda los ajustes que comparten tus dispositivos",
        "Dosarul ascuns dail/Settings al acestuia păstrează setările comune ale dispozitivelor dvs.";
    SettingsAccountFirst:
        "{email} (first account)",
        "{email} (premier compte)",
        "{email} (primera cuenta)",
        "{email} (primul cont)";
    SettingsAccountWhich:
        "Which account holds the settings?",
        "Quel compte garde les paramètres ?",
        "¿Qué cuenta guarda los ajustes?",
        "Ce cont păstrează setările?";
    SettingsReading:
        "reading from the server…",
        "lecture sur le serveur…",
        "leyendo del servidor…",
        "se citește de pe server…";
    SettingsWriting:
        "storing on the server…",
        "enregistrement sur le serveur…",
        "guardando en el servidor…",
        "se salvează pe server…";
    SettingsUpToDate:
        "up to date",
        "à jour",
        "al día",
        "la zi";
    SettingsNotRead:
        "not read yet",
        "pas encore lus",
        "aún sin leer",
        "necitite încă";
    SettingsSyncState:
        "Settings sync",
        "Synchronisation des paramètres",
        "Sincronización de los ajustes",
        "Sincronizarea setărilor";
    SettingsShared:
        "Follow this account on every device: the theme, language, list density, mark-read delay, undo-send window and what shows after triage. The rest stays on this device.",
        "Suivent ce compte sur chaque appareil : le thème, la langue, la densité de la liste, le délai avant lecture, le délai d’annulation d’envoi et ce qui s’affiche après le tri. Le reste reste sur cet appareil.",
        "Siguen a esta cuenta en todos los dispositivos: el tema, el idioma, la densidad de la lista, el retraso para marcar como leído, el plazo para deshacer el envío y lo que se muestra tras clasificar. El resto se queda en este dispositivo.",
        "Urmează acest cont pe fiecare dispozitiv: tema, limba, densitatea listei, întârzierea marcării ca citit, intervalul de anulare a trimiterii și ce se afișează după sortare. Restul rămâne pe acest dispozitiv.";
    RuleSenderContains:
        "Sort mail whose sender contains…",
        "Trier le courrier dont l’expéditeur contient…",
        "Clasificar el correo cuyo remitente contiene…",
        "Sortați e-mailurile al căror expeditor conține…";
    RuleSenderHint:
        "e.g. newsletter@example.org",
        "par ex. newsletter@example.org",
        "p. ej. newsletter@example.org",
        "de ex. newsletter@example.org";
    RuleMoveToWhichFolder:
        "Move it to which folder?",
        "Le déplacer dans quel dossier ?",
        "¿Moverlo a qué carpeta?",
        "Îl mutați în ce dosar?";
    RuleFromTo:
        "From {sender} to {to}",
        "De {sender} vers {to}",
        "De {sender} a {to}",
        "De la {sender} în {to}";
    DeleteWhichRule:
        "Delete which rule?",
        "Supprimer quelle règle ?",
        "¿Eliminar qué regla?",
        "Ștergeți ce regulă?";
    ToggleWhichRule:
        "Turn which rule on or off?",
        "Activer ou désactiver quelle règle ?",
        "¿Activar o desactivar qué regla?",
        "Activați sau dezactivați ce regulă?";
    ApplyRulesWhichMailbox:
        "Run the rules on which mailbox?",
        "Appliquer les règles à quel dossier ?",
        "¿Aplicar las reglas a qué carpeta?",
        "Aplicați regulile pe ce dosar?";
    RuleOn:
        "on",
        "active",
        "activa",
        "activă";
    RuleOff:
        "off",
        "inactive",
        "inactiva",
        "inactivă";
    RulesTab:
        "Rules",
        "Règles",
        "Reglas",
        "Reguli";
    RulesTitle:
        "Sorting rules",
        "Règles de tri",
        "Reglas de clasificación",
        "Reguli de sortare";
    RulesHint:
        "dail runs these rules on new mail while it is open, on every device. They are stored with the settings on the server.",
        "dail applique ces règles au nouveau courrier tant qu’il est ouvert, sur chaque appareil. Elles sont enregistrées avec les paramètres sur le serveur.",
        "dail aplica estas reglas al correo nuevo mientras está abierto, en todos los dispositivos. Se guardan con los ajustes en el servidor.",
        "dail aplică aceste reguli e-mailurilor noi cât timp este deschis, pe fiecare dispozitiv. Sunt salvate împreună cu setările pe server.";
    RulesEmpty:
        "No rules yet.",
        "Aucune règle pour l’instant.",
        "Aún no hay reglas.",
        "Încă nu există reguli.";
    RulesNotStored:
        "Changes not stored on the server yet",
        "Modifications pas encore enregistrées sur le serveur",
        "Cambios aún no guardados en el servidor",
        "Modificări nesalvate încă pe server";
    RulesStored:
        "Stored on the server",
        "Enregistrées sur le serveur",
        "Guardadas en el servidor",
        "Salvate pe server";
    RulesStore:
        "Store on the server",
        "Enregistrer sur le serveur",
        "Guardar en el servidor",
        "Salvați pe server";
    RulesApplyHere:
        "Apply to this mailbox",
        "Appliquer à ce dossier",
        "Aplicar a esta carpeta",
        "Aplicați pe acest dosar";
    RulesApplyHint:
        "Open a mailbox to run the rules on it.",
        "Ouvrez un dossier pour lui appliquer les règles.",
        "Abre una carpeta para aplicarle las reglas.",
        "Deschideți un dosar pentru a-i aplica regulile.";
    AddRule:
        "Add a rule…",
        "Ajouter une règle…",
        "Añadir una regla…",
        "Adăugați o regulă…";
    EditRule:
        "Edit…",
        "Modifier…",
        "Editar…",
        "Editați…";
    RuleEnabled:
        "Enabled",
        "Active",
        "Activa",
        "Activă";
    RuleSummary:
        "{when} → {then}",
        "{when} → {then}",
        "{when} → {then}",
        "{when} → {then}";
    RuleDoNothing:
        "do nothing",
        "ne rien faire",
        "no hacer nada",
        "nu face nimic";
    RuleEveryMessage:
        "every message",
        "tout message",
        "todos los mensajes",
        "orice mesaj";
    RuleNoMessage:
        "no message",
        "aucun message",
        "ningún mensaje",
        "niciun mesaj";
    RuleAnd:
        " and ",
        " et ",
        " y ",
        " și ";
    RuleOr:
        " or ",
        " ou ",
        " o ",
        " sau ";
    RuleNot:
        "not ({condition})",
        "non ({condition})",
        "no ({condition})",
        "nu ({condition})";
    RuleCondFrom:
        "sender contains “{text}”",
        "l’expéditeur contient « {text} »",
        "el remitente contiene «{text}»",
        "expeditorul conține „{text}”";
    RuleCondTo:
        "To contains “{text}”",
        "À contient « {text} »",
        "Para contiene «{text}»",
        "Către conține „{text}”";
    RuleCondCc:
        "Cc contains “{text}”",
        "Cc contient « {text} »",
        "CC contiene «{text}»",
        "Cc conține „{text}”";
    RuleCondSubject:
        "subject contains “{text}”",
        "l’objet contient « {text} »",
        "el asunto contiene «{text}»",
        "subiectul conține „{text}”";
    RuleCondList:
        "mailing list contains “{text}”",
        "la liste de diffusion contient « {text} »",
        "la lista de correo contiene «{text}»",
        "lista de corespondență conține „{text}”";
    RuleCondAttachment:
        "has an attachment",
        "a une pièce jointe",
        "tiene un adjunto",
        "are un atașament";
    RuleCondMailbox:
        "in {path}",
        "dans {path}",
        "en {path}",
        "în {path}";
    RuleActMove:
        "move to {to}",
        "déplacer vers {to}",
        "mover a {to}",
        "mutare în {to}";
    RuleActLabel:
        "label {label}",
        "étiqueter {label}",
        "etiquetar {label}",
        "etichetare {label}";
    RuleActRead:
        "mark read",
        "marquer comme lu",
        "marcar como leído",
        "marcare ca citit";
    RuleActStar:
        "star",
        "suivre",
        "destacar",
        "marcare cu stea";
    RuleActArchive:
        "archive",
        "archiver",
        "archivar",
        "arhivare";
    RuleWhichTest:
        "Match mail by what?",
        "Reconnaître le courrier à quoi ?",
        "¿Reconocer el correo por qué?",
        "Recunoașteți e-mailurile după ce?";
    RuleTestFrom:
        "Sender",
        "Expéditeur",
        "Remitente",
        "Expeditor";
    RuleTestTo:
        "Recipient (To)",
        "Destinataire (À)",
        "Destinatario (Para)",
        "Destinatar (Către)";
    RuleTestCc:
        "Copy (Cc)",
        "Copie (Cc)",
        "Copia (CC)",
        "Copie (Cc)";
    RuleTestSubject:
        "Subject",
        "Objet",
        "Asunto",
        "Subiect";
    RuleTestList:
        "Mailing list",
        "Liste de diffusion",
        "Lista de correo",
        "Listă de corespondență";
    RuleTestAttachment:
        "Has an attachment",
        "A une pièce jointe",
        "Tiene un adjunto",
        "Are un atașament";
    RuleContainsWhat:
        "Containing what?",
        "Contenant quoi ?",
        "¿Que contenga qué?",
        "Care conține ce?";
    RuleContainsHint:
        "Upper or lower case does not matter",
        "Majuscules et minuscules indifférentes",
        "Da igual mayúsculas o minúsculas",
        "Nu contează literele mari sau mici";
    RuleWhichAction:
        "Then do what?",
        "Puis faire quoi ?",
        "¿Y luego qué hacer?",
        "Apoi ce să facă?";
    RuleActionMove:
        "Move to a folder…",
        "Déplacer dans un dossier…",
        "Mover a una carpeta…",
        "Mutare într-un dosar…";
    RuleActionArchive:
        "Archive",
        "Archiver",
        "Archivar",
        "Arhivare";
    RuleActionRead:
        "Mark read",
        "Marquer comme lu",
        "Marcar como leído",
        "Marcare ca citit";
    RuleActionStar:
        "Star",
        "Suivre",
        "Destacar",
        "Marcare cu stea";
    RuleEditWhat:
        "Change what?",
        "Modifier quoi ?",
        "¿Cambiar qué?",
        "Ce schimbați?";
    RuleEditName:
        "Name",
        "Nom",
        "Nombre",
        "Nume";
    RuleEditCondition:
        "Condition",
        "Condition",
        "Condición",
        "Condiție";
    RuleEditAction:
        "Action",
        "Action",
        "Acción",
        "Acțiune";
    RuleDelete:
        "Delete",
        "Supprimer",
        "Eliminar",
        "Ștergeți";
    RuleNewName:
        "New name?",
        "Nouveau nom ?",
        "¿Nombre nuevo?",
        "Nume nou?";
    MakeRuleFromMessage:
        "Make a rule from this message",
        "Créer une règle à partir de ce message",
        "Crear una regla a partir de este mensaje",
        "Creați o regulă din acest mesaj";
    RuleFromMessageWhich:
        "Match mail like this one by…",
        "Reconnaître le courrier semblable par…",
        "Reconocer el correo como este por…",
        "Recunoașteți e-mailurile ca acesta după…";
    RulesFooter:
        "Tab section · j/k move · Enter edit · Space on/off · a add · x apply · p store · Esc close",
        "Tab section · j/k déplacer · Entrée modifier · Espace activer · a ajouter · x appliquer · p enregistrer · Échap fermer",
        "Tab sección · j/k mover · Intro editar · Espacio activar · a añadir · x aplicar · p guardar · Esc cerrar",
        "Tab secțiune · j/k deplasare · Enter editați · Spațiu activați · a adăugați · x aplicați · p salvați · Esc închideți";
    RuleEditComplex:
        "Change what? This rule's conditions or actions are too complex to edit here: use the CLI or an agent.",
        "Modifier quoi ? Les conditions ou actions de cette règle sont trop complexes pour être modifiées ici : utilisez la CLI ou un agent.",
        "¿Cambiar qué? Las condiciones o acciones de esta regla son demasiado complejas para editarlas aquí: usa la CLI o un agente.",
        "Ce schimbați? Condițiile sau acțiunile acestei reguli sunt prea complexe pentru a fi editate aici: folosiți CLI-ul sau un agent.";
    RuleEnable:
        "Turn on",
        "Activer",
        "Activar",
        "Activați";
    RuleDisable:
        "Turn off",
        "Désactiver",
        "Desactivar",
        "Dezactivați";
    MessageActionName:
        "{action}, message from {sender}",
        "{action}, message de {sender}",
        "{action}, mensaje de {sender}",
        "{action}, mesaj de la {sender}";
    MessageCursorHelp:
        "n p            the message cursor: the next / previous message of the conversation; its keys (r a f u s #) then act on that message alone, Enter edits a draft",
        "n p            le curseur de message : le message suivant / précédent de la discussion ; ses touches (r a f u s #) agissent alors sur ce seul message, Entrée modifie un brouillon",
        "n p            el cursor de mensaje: el mensaje siguiente / anterior de la conversación; sus teclas (r a f u s #) actúan entonces solo sobre ese mensaje, Intro edita un borrador",
        "n p            cursorul de mesaj: mesajul următor / anterior din conversație; tastele lui (r a f u s #) acționează apoi doar asupra acelui mesaj, Enter editează o ciornă";
    NoteDelete:
        "Delete",
        "Supprimer",
        "Eliminar",
        "Ștergeți";
    NewConversations:
        "{n} new conversations",
        "{n} nouvelles discussions",
        "{n} conversaciones nuevas",
        "{n} conversații noi";
    MakeRule:
        "Make a rule…",
        "Créer une règle…",
        "Crear una regla…",
        "Creați o regulă…";
    ApplyRules:
        "Apply rules",
        "Appliquer les règles",
        "Aplicar las reglas",
        "Aplicați regulile";
    OfflineWindow:
        "Offline window…",
        "Copie hors ligne…",
        "Copia sin conexión…",
        "Copie offline…";
    SendingAddresses:
        "Sending addresses…",
        "Adresses d’envoi…",
        "Direcciones de envío…",
        "Adrese de trimitere…";
    NotificationsBackground:
        "Notifications & background",
        "Notifications et arrière-plan",
        "Notificaciones y segundo plano",
        "Notificări și fundal";
    NoKeyboardAttached:
        "No keyboard attached",
        "Aucun clavier branché",
        "Ningún teclado conectado",
        "Nicio tastatură conectată";
    SettingsAccountCount:
        "{n} account(s)",
        "{n} compte(s)",
        "{n} cuenta(s)",
        "{n} cont(uri)";
    SettingsRuleCount:
        "{n} rule(s)",
        "{n} règle(s)",
        "{n} regla(s)",
        "{n} regulă(i)";
    BackgroundSyncHint:
        "Periodic checks wait for the phone's maintenance windows. Push keeps a silent notification while it watches the inboxes.",
        "Les vérifications périodiques attendent les fenêtres de maintenance du téléphone. Le mode push garde une notification silencieuse pendant qu’il surveille les boîtes de réception.",
        "Las comprobaciones periódicas esperan a las ventanas de mantenimiento del teléfono. Push mantiene una notificación silenciosa mientras vigila las bandejas de entrada.",
        "Verificările periodice așteaptă ferestrele de mentenanță ale telefonului. Modul push păstrează o notificare silențioasă cât timp urmărește căsuțele de intrare.";
    BatteryExemptHint:
        "Some phones stop background checks until dail is exempt from battery optimization.",
        "Certains téléphones arrêtent les vérifications en arrière-plan tant que dail n’est pas exempté de l’optimisation de la batterie.",
        "Algunos teléfonos detienen las comprobaciones en segundo plano hasta que dail queda exento de la optimización de batería.",
        "Unele telefoane opresc verificările din fundal până când dail este exceptat de la optimizarea bateriei.";
    SettingsLastPulled:
        "Last pulled",
        "Dernière lecture",
        "Última lectura",
        "Ultima citire";
    SettingsLastStored:
        "Last stored",
        "Dernier enregistrement",
        "Último guardado",
        "Ultima salvare";
    SettingsPullNow:
        "Pull now",
        "Lire maintenant",
        "Leer ahora",
        "Citiți acum";
    SettingsLastError:
        "Last error",
        "Dernière erreur",
        "Último error",
        "Ultima eroare";
}

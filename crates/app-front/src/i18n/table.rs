//! The strings of every language. A row is a key with its English, French, Spanish, Romanian,
//! Brazilian Portuguese, European Portuguese and Latin templates, so a key cannot miss a language;
//! the doc of a key is its English text.

/// Declares [`Key`], [`Key::ALL`] and a function per language from one table.
macro_rules! strings {
    ($($key:ident: $english:literal, $french:literal, $spanish:literal, $romanian:literal,
        $pt_br:literal, $pt_pt:literal, $latin:literal;)*) => {
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

        /// The Brazilian Portuguese table.
        pub const fn portuguese_brazil(key: Key) -> &'static str {
            match key {
                $(Key::$key => $pt_br,)*
            }
        }

        /// The European Portuguese table.
        pub const fn portuguese_portugal(key: Key) -> &'static str {
            match key {
                $(Key::$key => $pt_pt,)*
            }
        }

        /// The Latin table.
        pub const fn latin(key: Key) -> &'static str {
            match key {
                $(Key::$key => $latin,)*
            }
        }
    };
}

strings! {
    StoreFailed:
        "local store failed",
        "échec du stockage local",
        "error del almacenamiento local",
        "eroare la stocarea locală",
        "falha no armazenamento local",
        "falha no armazenamento local",
        "repositorium locale defecit";
    Quarantined:
        "{count} saved items could not be read",
        "{count} éléments enregistrés n’ont pas pu être lus",
        "{count} elementos guardados no se pudieron leer",
        "{count} elemente salvate nu au putut fi citite",
        "{count} itens salvos não puderam ser lidos",
        "{count} itens guardados não puderam ser lidos",
        "{count} res servatae legi non potuerunt";
    QuarantinedHint:
        "They are kept aside in the profile, not deleted: a later version of dail may read them. Report it with the version you updated from.",
        "Ils sont mis de côté dans le profil, pas supprimés : une version ultérieure de dail pourra peut-être les lire. Signalez-le en indiquant la version précédente.",
        "Se guardan aparte en el perfil, no se eliminan: una versión posterior de dail podría leerlos. Infórmalo indicando la versión desde la que actualizaste.",
        "Sunt păstrate deoparte în profil, nu șterse: o versiune ulterioară de dail le-ar putea citi. Raportați problema, menționând versiunea de la care ați actualizat.",
        "Eles ficam separados no perfil, não são excluídos: uma versão futura do dail pode lê-los. Informe o problema com a versão de onde você atualizou.",
        "Ficam à parte no perfil, não são eliminados: uma versão futura do dail poderá lê-los. Comunique o problema indicando a versão a partir da qual atualizou.",
        "Seorsum in profilo servantur, non delentur: versio posterior dail eas legere fortasse poterit. Rem nuntia cum versione unde renovavisti.";
    KeyringFailed:
        "keyring failed",
        "échec du trousseau",
        "error del llavero",
        "eroare la portofelul de chei",
        "falha no chaveiro",
        "falha no porta-chaves",
        "clavarium defecit";
    KeyringHint:
        "The system keyring (Secret Service on Linux, Credential Manager on Windows) is locked or missing: unlock it, or start a keyring service.",
        "Le trousseau du système (Secret Service sous Linux, Gestionnaire d’identifiants sous Windows) est verrouillé ou absent : déverrouillez-le ou démarrez un service de trousseau.",
        "El llavero del sistema (Secret Service en Linux, Administrador de credenciales en Windows) está bloqueado o no existe: desbloquéalo o inicia un servicio de llavero.",
        "Portofelul de chei al sistemului (Secret Service pe Linux, Credential Manager pe Windows) este blocat sau lipsește: deblocați-l sau porniți un serviciu de chei.",
        "O chaveiro do sistema (Secret Service no Linux, Gerenciador de Credenciais no Windows) está bloqueado ou ausente: desbloqueie-o ou inicie um serviço de chaveiro.",
        "O porta-chaves do sistema (Secret Service no Linux, Gestor de Credenciais no Windows) está bloqueado ou em falta: desbloqueie-o ou inicie um serviço de porta-chaves.",
        "Clavarium systematis (Secret Service in Linux, Credential Manager in Windows) clausum est aut deest: id resera aut ministerium clavarii incipe.";
    LoginFailed:
        "{email}: login failed",
        "{email} : échec de la connexion",
        "{email}: error al iniciar sesión",
        "{email}: autentificarea a eșuat",
        "{email}: falha no login",
        "{email}: falha no início de sessão",
        "{email}: ingressus defecit";
    SyncFailed:
        "{email}: sync failed",
        "{email} : échec de la synchronisation",
        "{email}: error de sincronización",
        "{email}: sincronizarea a eșuat",
        "{email}: falha na sincronização",
        "{email}: falha na sincronização",
        "{email}: synchronizatio defecit";
    FixPassword:
        "re-enter password",
        "saisir à nouveau le mot de passe",
        "volver a escribir la contraseña",
        "reintroduceți parola",
        "digitar a senha novamente",
        "introduzir a palavra-passe novamente",
        "tesseram iterum insere";
    FixRetry:
        "retry",
        "réessayer",
        "reintentar",
        "reîncercați",
        "tentar novamente",
        "tentar novamente",
        "iterum conare";
    FixTest:
        "test again",
        "tester à nouveau",
        "probar de nuevo",
        "testați din nou",
        "testar novamente",
        "testar novamente",
        "iterum proba";
    HintPasswordGmail:
        "Gmail refused the password. It needs an App Password (Google account → Security → 2-Step Verification → App passwords), not your Google password.",
        "Gmail a refusé le mot de passe. Il faut un mot de passe d’application (compte Google → Sécurité → Validation en deux étapes → Mots de passe des applications), pas votre mot de passe Google.",
        "Gmail rechazó la contraseña. Necesita una contraseña de aplicación (cuenta de Google → Seguridad → Verificación en dos pasos → Contraseñas de aplicaciones), no tu contraseña de Google.",
        "Gmail a refuzat parola. Este nevoie de o parolă pentru aplicații (Cont Google → Securitate → Verificare în doi pași → Parole pentru aplicații), nu de parola contului Google.",
        "O Gmail recusou a senha. É preciso uma senha de app (Conta do Google → Segurança → Verificação em duas etapas → Senhas de app), não a sua senha do Google.",
        "O Gmail recusou a palavra-passe. É necessária uma palavra-passe de aplicação (Conta Google → Segurança → Validação em dois passos → Palavras-passe de aplicações), não a sua palavra-passe Google.",
        "Gmail tesseram recusavit. Tessera applicationis requiritur (Ratio Google → Securitas → Verificatio duplex → Tesserae applicationum), non tessera tua Google.";
    HintPassword:
        "The server refused the password (or none is stored). Enter it again; if you changed it on the web, use the new one.",
        "Le serveur a refusé le mot de passe (ou aucun n’est enregistré). Saisissez-le à nouveau ; si vous l’avez changé sur le web, utilisez le nouveau.",
        "El servidor rechazó la contraseña (o no hay ninguna guardada). Escríbela de nuevo; si la cambiaste en la web, usa la nueva.",
        "Serverul a refuzat parola (sau nu este salvată niciuna). Introduceți-o din nou; dacă ați schimbat-o pe web, folosiți-o pe cea nouă.",
        "O servidor recusou a senha (ou nenhuma está salva). Digite-a novamente; se você a alterou na web, use a nova.",
        "O servidor recusou a palavra-passe (ou nenhuma está guardada). Introduza-a novamente; se a alterou na web, use a nova.",
        "Servitor tesseram recusavit (aut nulla servata est). Eam iterum insere; si in interreti mutavisti, novam adhibe.";
    HintQuota:
        "The mailbox is full. Delete or archive mail and empty the Trash, then retry.",
        "La boîte aux lettres est pleine. Supprimez ou archivez du courrier et videz la corbeille, puis réessayez.",
        "El buzón está lleno. Elimina o archiva correo y vacía la papelera; luego reintenta.",
        "Căsuța poștală este plină. Ștergeți sau arhivați mesaje și goliți coșul de gunoi, apoi reîncercați.",
        "A caixa de correio está cheia. Exclua ou arquive mensagens e esvazie a lixeira; depois tente novamente.",
        "A caixa de correio está cheia. Elimine ou arquive mensagens e esvazie o lixo; depois tente novamente.",
        "Capsa epistularum plena est. Epistulas dele aut in archivum conde et quisquilias exhauri, deinde iterum conare.";
    HintTlsCiphers:
        "The server offers only TLS versions or ciphers dail does not support (dail needs TLS 1.2 or 1.3 with ChaCha20-Poly1305 or AES-GCM). Ask the provider; nothing in dail's settings changes this.",
        "Le serveur ne propose que des versions ou des chiffrements TLS que dail ne prend pas en charge (dail exige TLS 1.2 ou 1.3 avec ChaCha20-Poly1305 ou AES-GCM). Contactez le fournisseur ; aucun réglage de dail n’y change rien.",
        "El servidor solo ofrece versiones o cifrados TLS que dail no admite (dail necesita TLS 1.2 o 1.3 con ChaCha20-Poly1305 o AES-GCM). Consulta al proveedor; ningún ajuste de dail lo cambia.",
        "Serverul oferă doar versiuni sau cifruri TLS pe care dail nu le acceptă (dail are nevoie de TLS 1.2 sau 1.3 cu ChaCha20-Poly1305 ori AES-GCM). Contactați furnizorul; nicio setare din dail nu schimbă asta.",
        "O servidor só oferece versões ou cifras TLS que o dail não suporta (o dail precisa de TLS 1.2 ou 1.3 com ChaCha20-Poly1305 ou AES-GCM). Fale com o provedor; nenhuma configuração do dail muda isso.",
        "O servidor só oferece versões ou cifras TLS que o dail não suporta (o dail precisa de TLS 1.2 ou 1.3 com ChaCha20-Poly1305 ou AES-GCM). Contacte o fornecedor; nenhuma definição do dail altera isto.",
        "Servitor solum versiones aut cifras TLS praebet quas dail non sustinet (dail requirit TLS 1.2 aut 1.3 cum ChaCha20-Poly1305 aut AES-GCM). Provisorem interroga; nulla optio dail hoc mutat.";
    HintCertificate:
        "The server's certificate is not trusted: expired, self-signed or for another name. Check the server name. If you run the server and it uses its own certificate, trust it with “trust…”.",
        "Le certificat du serveur n’est pas fiable : expiré, auto-signé ou émis pour un autre nom. Vérifiez le nom du serveur. Si le serveur est le vôtre et utilise son propre certificat, acceptez-le avec « faire confiance… ».",
        "El certificado del servidor no es de confianza: caducado, autofirmado o emitido para otro nombre. Comprueba el nombre del servidor. Si el servidor es tuyo y usa su propio certificado, puedes confiar en él con «confiar…».",
        "Certificatul serverului nu este de încredere: expirat, autosemnat sau emis pentru alt nume. Verificați numele serverului. Dacă serverul este al dvs. și folosește propriul certificat, îl puteți accepta cu „acceptați…”.",
        "O certificado do servidor não é confiável: expirado, autoassinado ou emitido para outro nome. Verifique o nome do servidor. Se o servidor é seu e usa um certificado próprio, confie nele com “confiar…”.",
        "O certificado do servidor não é fidedigno: expirado, autoassinado ou emitido para outro nome. Verifique o nome do servidor. Se o servidor é seu e usa um certificado próprio, confie nele com “confiar…”.",
        "Testimonium servitoris fide non dignum est: exspiravit, a se ipso signatum est aut alii nomini datum. Nomen servitoris inspice. Si servitor tuus est et proprio testimonio utitur, ei confide per «confide…».";
    HintNetwork:
        "The server could not be reached. Check the connection and the server name; syncing tries again by itself.",
        "Le serveur est injoignable. Vérifiez la connexion et le nom du serveur ; la synchronisation réessaie d’elle-même.",
        "No se pudo conectar con el servidor. Comprueba la conexión y el nombre del servidor; la sincronización vuelve a intentarlo sola.",
        "Serverul nu a putut fi contactat. Verificați conexiunea și numele serverului; sincronizarea reîncearcă singură.",
        "Não foi possível alcançar o servidor. Verifique a conexão e o nome do servidor; a sincronização tenta de novo sozinha.",
        "Não foi possível contactar o servidor. Verifique a ligação e o nome do servidor; a sincronização volta a tentar automaticamente.",
        "Servitor attingi non potuit. Conexionem et nomen servitoris inspice; synchronizatio sponte iterum conatur.";
    Syncing:
        "⟳ syncing…",
        "⟳ synchronisation…",
        "⟳ sincronizando…",
        "⟳ se sincronizează…",
        "⟳ sincronizando…",
        "⟳ a sincronizar…",
        "⟳ synchronizatur…";
    Synced:
        "synced {age}",
        "synchronisé {age}",
        "sincronizado {age}",
        "sincronizat {age}",
        "sincronizado {age}",
        "sincronizado {age}",
        "synchronizatum {age}";
    NoAccounts:
        "no accounts",
        "aucun compte",
        "sin cuentas",
        "niciun cont",
        "nenhuma conta",
        "nenhuma conta",
        "nullae rationes";
    NotSynced:
        "not synced",
        "non synchronisé",
        "sin sincronizar",
        "nesincronizat",
        "não sincronizado",
        "não sincronizado",
        "non synchronizatum";
    NextPoll:
        "next {when}",
        "prochaine {when}",
        "próxima {when}",
        "următoarea {when}",
        "próxima {when}",
        "próxima {when}",
        "proxima {when}";
    Pending:
        "{pending} pending",
        "{pending} en attente",
        "{pending} pendientes",
        "{pending} în așteptare",
        "{pending} pendentes",
        "{pending} pendentes",
        "{pending} exspectant";
    PendingFailed:
        "{pending} pending · {failed} failed",
        "{pending} en attente · {failed} en échec",
        "{pending} pendientes · {failed} con error",
        "{pending} în așteptare · {failed} eșuate",
        "{pending} pendentes · {failed} com falha",
        "{pending} pendentes · {failed} com falha",
        "{pending} exspectant · {failed} defecerunt";
    CollapseSidebar:
        "Collapse sidebar (Ctrl+B)",
        "Réduire la barre latérale (Ctrl+B)",
        "Contraer la barra lateral (Ctrl+B)",
        "Restrângeți bara laterală (Ctrl+B)",
        "Recolher a barra lateral (Ctrl+B)",
        "Recolher a barra lateral (Ctrl+B)",
        "Tabulam lateralem contrahe (Ctrl+B)";
    ExpandSidebar:
        "Expand sidebar (Ctrl+B)",
        "Développer la barre latérale (Ctrl+B)",
        "Expandir la barra lateral (Ctrl+B)",
        "Extindeți bara laterală (Ctrl+B)",
        "Expandir a barra lateral (Ctrl+B)",
        "Expandir a barra lateral (Ctrl+B)",
        "Tabulam lateralem expande (Ctrl+B)";
    Compose:
        "Compose",
        "Rédiger",
        "Redactar",
        "Scrieți",
        "Escrever",
        "Escrever",
        "Scribe";
    ComposeKey:
        "Compose (C)",
        "Rédiger (C)",
        "Redactar (C)",
        "Scrieți (C)",
        "Escrever (C)",
        "Escrever (C)",
        "Scribe (C)";
    Settings:
        "Settings",
        "Paramètres",
        "Ajustes",
        "Setări",
        "Configurações",
        "Definições",
        "Optiones";
    ShowAllFolders:
        "Show all folders",
        "Afficher tous les dossiers",
        "Mostrar todas las carpetas",
        "Afișați toate dosarele",
        "Mostrar todas as pastas",
        "Mostrar todas as pastas",
        "Omnes capsas monstra";
    AllInboxes:
        "All inboxes",
        "Dossiers unifiés",
        "Todas las bandejas de entrada",
        "Toate căsuțele de intrare",
        "Todas as caixas de entrada",
        "Todas as caixas de entrada",
        "Omnes capsae acceptorum";
    Favourites:
        "Favourites",
        "Dossiers préférés",
        "Favoritos",
        "Favorite",
        "Favoritos",
        "Favoritos",
        "Dilecta";
    HideHiddenFolders:
        "Hide hidden folders",
        "Masquer les dossiers cachés",
        "Ocultar las carpetas ocultas",
        "Ascundeți dosarele ascunse",
        "Ocultar as pastas ocultas",
        "Ocultar as pastas ocultas",
        "Capsas occultas cela";
    NoAccountsYet:
        "No accounts yet.",
        "Aucun compte pour l’instant.",
        "Aún no hay cuentas.",
        "Încă nu există conturi.",
        "Ainda não há contas.",
        "Ainda não há contas.",
        "Nullae rationes adhuc.";
    AddAccount:
        "Add account",
        "Ajouter un compte",
        "Añadir cuenta",
        "Adăugați un cont",
        "Adicionar conta",
        "Adicionar conta",
        "Rationem adde";
    AddAccountPlus:
        "+ Add account",
        "+ Ajouter un compte",
        "+ Añadir cuenta",
        "+ Adăugați un cont",
        "+ Adicionar conta",
        "+ Adicionar conta",
        "+ Rationem adde";
    SyncingNow:
        "Syncing",
        "Synchronisation",
        "Sincronizando",
        "Se sincronizează",
        "Sincronizando",
        "A sincronizar",
        "Synchronizatur";
    Offline:
        "offline",
        "hors ligne",
        "sin conexión",
        "offline",
        "offline",
        "offline",
        "sine conexione";
    NewSubfolder:
        "New subfolder…",
        "Nouveau sous-dossier…",
        "Nueva subcarpeta…",
        "Subdosar nou…",
        "Nova subpasta…",
        "Nova subpasta…",
        "Nova subcapsa…";
    Rename:
        "Rename…",
        "Renommer…",
        "Cambiar nombre…",
        "Redenumiți…",
        "Renomear…",
        "Mudar o nome…",
        "Renomina…";
    UnpinFavourites:
        "Unpin from Favourites",
        "Retirer des dossiers préférés",
        "Quitar de Favoritos",
        "Eliminați din Favorite",
        "Remover dos Favoritos",
        "Remover dos Favoritos",
        "E dilectis remove";
    PinFavourites:
        "Pin to Favourites",
        "Ajouter aux dossiers préférés",
        "Fijar en Favoritos",
        "Fixați în Favorite",
        "Fixar nos Favoritos",
        "Fixar nos Favoritos",
        "In dilectis fige";
    ShowFolder:
        "Show folder",
        "Afficher le dossier",
        "Mostrar la carpeta",
        "Afișați dosarul",
        "Mostrar a pasta",
        "Mostrar a pasta",
        "Capsam monstra";
    HideFolder:
        "Hide folder",
        "Masquer le dossier",
        "Ocultar la carpeta",
        "Ascundeți dosarul",
        "Ocultar a pasta",
        "Ocultar a pasta",
        "Capsam cela";
    FocusMailbox:
        "Focus this mailbox",
        "Se concentrer sur ce dossier",
        "Centrarse en esta carpeta",
        "Concentrați-vă pe acest dosar",
        "Focar nesta pasta",
        "Focar nesta pasta",
        "In hac capsa intende";
    MarkAllRead:
        "Mark all as read",
        "Tout marquer comme lu",
        "Marcar todo como leído",
        "Marcați tot ca citit",
        "Marcar tudo como lido",
        "Marcar tudo como lido",
        "Omnia ut lecta nota";
    Delete:
        "Delete…",
        "Supprimer…",
        "Eliminar…",
        "Ștergeți…",
        "Excluir…",
        "Eliminar…",
        "Dele…";
    SearchOrRun:
        "Search or run a command",
        "Rechercher ou lancer une commande",
        "Busca o ejecuta un comando",
        "Căutați sau rulați o comandă",
        "Pesquise ou execute um comando",
        "Pesquise ou execute um comando",
        "Quaere aut mandatum exsequere";
    MoveHint:
        "j / k to move",
        "j / k pour se déplacer",
        "j / k para moverte",
        "j / k pentru deplasare",
        "j / k para mover",
        "j / k para mover",
        "j / k ad movendum";
    SyncNow:
        "Sync now",
        "Synchroniser",
        "Sincronizar ahora",
        "Sincronizați acum",
        "Sincronizar agora",
        "Sincronizar agora",
        "Nunc synchroniza";
    SearchingServer:
        "searching the server…",
        "recherche sur le serveur…",
        "buscando en el servidor…",
        "se caută pe server…",
        "pesquisando no servidor…",
        "a pesquisar no servidor…",
        "in servitore quaeritur…";
    RetryFailed:
        "Retry failed changes",
        "Réessayer les modifications en échec",
        "Reintentar los cambios fallidos",
        "Reîncercați modificările eșuate",
        "Tentar de novo as alterações com falha",
        "Tentar de novo as alterações com falha",
        "Mutationes quae defecerunt iterum conare";
    Undo:
        "Undo",
        "Annuler",
        "Deshacer",
        "Anulați",
        "Desfazer",
        "Anular",
        "Revoca";
    ReadOnlyCaption:
        "Read-only · untrusted data",
        "Lecture seule · données non fiables",
        "Solo lectura · datos no fiables",
        "Doar citire · date nesigure",
        "Somente leitura · dados não confiáveis",
        "Só de leitura · dados não fidedignos",
        "Tantum legendum · data non fida";
    RawData:
        "Raw data",
        "Données brutes",
        "Datos sin procesar",
        "Date brute",
        "Dados brutos",
        "Dados brutos",
        "Data cruda";
    ReadOnlyResult:
        "Read-only result. Email content in it is untrusted data.",
        "Résultat en lecture seule. Le contenu des e-mails y est une donnée non fiable.",
        "Resultado de solo lectura. El contenido de correo que incluye son datos no fiables.",
        "Rezultat doar pentru citire. Conținutul de e-mail din el reprezintă date nesigure.",
        "Resultado somente leitura. O conteúdo de e-mail nele são dados não confiáveis.",
        "Resultado só de leitura. O conteúdo de e-mail nele são dados não fidedignos.",
        "Effectus tantum legendus. Argumentum epistularum in eo data non fida sunt.";
    CheckUpdates:
        "Check for updates",
        "Rechercher des mises à jour",
        "Buscar actualizaciones",
        "Căutați actualizări",
        "Procurar atualizações",
        "Procurar atualizações",
        "Renovationes quaere";
    NoThanks:
        "No thanks",
        "Non merci",
        "No, gracias",
        "Nu, mulțumesc",
        "Não, obrigado",
        "Não, obrigado",
        "Non, gratias";
    NotSaved:
        "NOT SAVED",
        "NON ENREGISTRÉ",
        "SIN GUARDAR",
        "NESALVAT",
        "NÃO SALVO",
        "NÃO GUARDADO",
        "NON SERVATUM";
    CloseEsc:
        "Close (Esc)",
        "Fermer (Échap)",
        "Cerrar (Esc)",
        "Închideți (Esc)",
        "Fechar (Esc)",
        "Fechar (Esc)",
        "Claude (Esc)";
    From:
        "From",
        "De",
        "De",
        "De la",
        "De",
        "De",
        "A";
    CcBcc:
        "Cc/Bcc",
        "Copie à/Copie cachée à",
        "CC/CCO",
        "Cc/Bcc",
        "Cc/Cco",
        "Cc/Bcc",
        "Cc/Bcc";
    Subject:
        "Subject",
        "Objet",
        "Asunto",
        "Subiect",
        "Assunto",
        "Assunto",
        "Res";
    WriteMessage:
        "Write your message…",
        "Écrivez votre message…",
        "Escribe tu mensaje…",
        "Scrieți mesajul…",
        "Escreva sua mensagem…",
        "Escreva a sua mensagem…",
        "Epistulam tuam scribe…";
    OriginalBelow:
        "The original is added below when sent",
        "L’original est ajouté dessous à l’envoi",
        "El original se añade debajo al enviar",
        "Originalul este adăugat dedesubt la trimitere",
        "O original é adicionado abaixo ao enviar",
        "O original é adicionado abaixo ao enviar",
        "Archetypum infra additur cum mittitur";
    CancelSend:
        "Cancel send",
        "Annuler l’envoi",
        "Cancelar el envío",
        "Anulați trimiterea",
        "Cancelar o envio",
        "Cancelar o envio",
        "Missionem rescinde";
    AttachFiles:
        "Attach files…",
        "Joindre des fichiers…",
        "Adjuntar archivos…",
        "Atașați fișiere…",
        "Anexar arquivos…",
        "Anexar ficheiros…",
        "Documenta adiunge…";
    Send:
        "Send",
        "Envoyer",
        "Enviar",
        "Trimiteți",
        "Enviar",
        "Enviar",
        "Mitte";
    Discard:
        "Discard",
        "Abandonner",
        "Descartar",
        "Renunțați",
        "Descartar",
        "Rejeitar",
        "Abice";
    NameExample:
        "name@example.com",
        "nom@example.com",
        "nombre@example.com",
        "nume@example.com",
        "nome@example.com",
        "nome@example.com",
        "nomen@example.com";
    AddRecipient:
        "Add this recipient",
        "Ajouter ce destinataire",
        "Añadir este destinatario",
        "Adăugați acest destinatar",
        "Adicionar este destinatário",
        "Adicionar este destinatário",
        "Hunc recipientem adde";
    Checking:
        "checking…",
        "vérification…",
        "comprobando…",
        "se verifică…",
        "verificando…",
        "a verificar…",
        "inspicitur…";
    AttachFilesTitle:
        "Attach files",
        "Joindre des fichiers",
        "Adjuntar archivos",
        "Atașați fișiere",
        "Anexar arquivos",
        "Anexar ficheiros",
        "Documenta adiunge";
    DropToAttach:
        "Drop the files to attach them",
        "Déposez les fichiers pour les joindre",
        "Suelta los archivos para adjuntarlos",
        "Plasați fișierele aici pentru a le atașa",
        "Solte os arquivos para anexá-los",
        "Largue os ficheiros para os anexar",
        "Documenta hic demitte ut adiungantur";
    NoMatches:
        "No matching conversations.",
        "Aucune discussion ne correspond.",
        "Ninguna conversación coincide.",
        "Nicio conversație nu se potrivește.",
        "Nenhuma conversa corresponde.",
        "Nenhuma conversa corresponde.",
        "Nullum colloquium congruit.";
    NoConversations:
        "No conversations here.",
        "Aucune discussion ici.",
        "No hay conversaciones aquí.",
        "Nu există conversații aici.",
        "Não há conversas aqui.",
        "Não há conversas aqui.",
        "Nulla colloquia hic.";
    PickMailbox:
        "Pick a mailbox.",
        "Choisissez un dossier.",
        "Elige una carpeta.",
        "Alegeți un dosar.",
        "Escolha uma pasta.",
        "Escolha uma pasta.",
        "Capsam elige.";
    Focus:
        "Focus",
        "Focus",
        "Enfoque",
        "Concentrare",
        "Foco",
        "Foco",
        "Intentio";
    Clear:
        "Clear",
        "Effacer",
        "Borrar",
        "Ștergeți",
        "Limpar",
        "Limpar",
        "Purga";
    SelectAll:
        "Select all",
        "Tout sélectionner",
        "Seleccionar todo",
        "Selectați tot",
        "Selecionar tudo",
        "Selecionar tudo",
        "Omnia selige";
    Unread:
        "Unread",
        "Non lus",
        "No leídos",
        "Necitite",
        "Não lidas",
        "Não lidas",
        "Non lecta";
    Starred:
        "Starred",
        "Suivis",
        "Destacados",
        "Cu stea",
        "Com estrela",
        "Com estrela",
        "Stellata";
    Attachments:
        "Attachments",
        "Pièces jointes",
        "Archivos adjuntos",
        "Atașamente",
        "Anexos",
        "Anexos",
        "Adiuncta";
    Refreshing:
        "Refreshing",
        "Actualisation",
        "Actualizando",
        "Se actualizează",
        "Atualizando",
        "A atualizar",
        "Renovatur";
    Refresh:
        "Refresh",
        "Actualiser",
        "Actualizar",
        "Actualizați",
        "Atualizar",
        "Atualizar",
        "Renova";
    Archive:
        "Archive",
        "Archiver",
        "Archivar",
        "Arhivați",
        "Arquivar",
        "Arquivar",
        "In archivum conde";
    Trash:
        "Trash",
        "Corbeille",
        "Papelera",
        "Coș de gunoi",
        "Lixeira",
        "Lixo",
        "Quisquiliae";
    MarkRead:
        "Mark read",
        "Marquer comme lu",
        "Marcar como leído",
        "Marcați ca citit",
        "Marcar como lida",
        "Marcar como lida",
        "Ut lectum nota";
    MarkUnread:
        "Mark unread",
        "Marquer comme non lu",
        "Marcar como no leído",
        "Marcați ca necitit",
        "Marcar como não lida",
        "Marcar como não lida",
        "Ut non lectum nota";
    Star:
        "Star",
        "Ajouter un suivi",
        "Destacar",
        "Marcați cu stea",
        "Adicionar estrela",
        "Adicionar estrela",
        "Stella nota";
    Unstar:
        "Unstar",
        "Retirer le suivi",
        "Quitar destacado",
        "Eliminați steaua",
        "Remover estrela",
        "Remover estrela",
        "Stellam remove";
    UncheckAll:
        "Uncheck all",
        "Tout décocher",
        "Desmarcar todo",
        "Debifați tot",
        "Desmarcar tudo",
        "Desmarcar tudo",
        "Omnia dissigna";
    Unpin:
        "Unpin",
        "Détacher",
        "Desfijar",
        "Anulați fixarea",
        "Desafixar",
        "Desafixar",
        "Refige";
    Pin:
        "Pin",
        "Épingler",
        "Fijar",
        "Fixați",
        "Fixar",
        "Fixar",
        "Fige";
    NoSender:
        "(no sender)",
        "(sans expéditeur)",
        "(sin remitente)",
        "(fără expeditor)",
        "(sem remetente)",
        "(sem remetente)",
        "(sine missore)";
    NoSubject:
        "(no subject)",
        "(pas de sujet)",
        "(sin asunto)",
        "(fără subiect)",
        "(sem assunto)",
        "(sem assunto)",
        "(sine re)";
    Pinned:
        "Pinned",
        "Épinglé",
        "Fijado",
        "Fixat",
        "Fixada",
        "Fixada",
        "Fixum";
    MoveTo:
        "Move to…",
        "Déplacer vers…",
        "Mover a…",
        "Mutați în…",
        "Mover para…",
        "Mover para…",
        "Move in…";
    QuotedText:
        "quoted text",
        "le texte cité",
        "el texto citado",
        "textul citat",
        "o texto citado",
        "o texto citado",
        "textum citatum";
    Signature:
        "signature",
        "la signature",
        "la firma",
        "semnătura",
        "a assinatura",
        "a assinatura",
        "subscriptionem";
    NoConversationOpen:
        "No conversation open",
        "Aucune discussion ouverte",
        "Ninguna conversación abierta",
        "Nicio conversație deschisă",
        "Nenhuma conversa aberta",
        "Nenhuma conversa aberta",
        "Nullum colloquium apertum";
    Reply:
        "Reply",
        "Répondre",
        "Responder",
        "Răspundeți",
        "Responder",
        "Responder",
        "Responde";
    ReplyAll:
        "Reply all",
        "Répondre à tous",
        "Responder a todos",
        "Răspundeți tuturor",
        "Responder a todos",
        "Responder a todos",
        "Omnibus responde";
    Forward:
        "Forward",
        "Transférer",
        "Reenviar",
        "Redirecționați",
        "Encaminhar",
        "Reencaminhar",
        "Transmitte";
    UnstarKey:
        "Unstar (S)",
        "Retirer le suivi (S)",
        "Quitar destacado (S)",
        "Eliminați steaua (S)",
        "Remover estrela (S)",
        "Remover estrela (S)",
        "Stellam remove (S)";
    StarKey:
        "Star (S)",
        "Ajouter un suivi (S)",
        "Destacar (S)",
        "Marcați cu stea (S)",
        "Adicionar estrela (S)",
        "Adicionar estrela (S)",
        "Stella nota (S)";
    MarkReadKey:
        "Mark read (U)",
        "Marquer comme lu (U)",
        "Marcar como leído (U)",
        "Marcați ca citit (U)",
        "Marcar como lida (U)",
        "Marcar como lida (U)",
        "Ut lectum nota (U)";
    MarkUnreadKey:
        "Mark unread (U)",
        "Marquer comme non lu (U)",
        "Marcar como no leído (U)",
        "Marcați ca necitit (U)",
        "Marcar como não lida (U)",
        "Marcar como não lida (U)",
        "Ut non lectum nota (U)";
    ArchiveKey:
        "Archive (E)",
        "Archiver (E)",
        "Archivar (E)",
        "Arhivați (E)",
        "Arquivar (E)",
        "Arquivar (E)",
        "In archivum conde (E)";
    TrashKey:
        "Trash (#)",
        "Corbeille (#)",
        "Papelera (#)",
        "Coș de gunoi (#)",
        "Lixeira (#)",
        "Lixo (#)",
        "Quisquiliae (#)";
    JunkKey:
        "Junk (!)",
        "Indésirable (!)",
        "Spam (!)",
        "Spam (!)",
        "Spam (!)",
        "Spam (!)",
        "Purgamentum (!)";
    NotJunkKey:
        "Not junk (!)",
        "Pas indésirable (!)",
        "No es spam (!)",
        "Nu este spam (!)",
        "Não é spam (!)",
        "Não é spam (!)",
        "Non purgamentum (!)";
    MarkJunk:
        "Junk",
        "Indésirable",
        "Spam",
        "Spam",
        "Spam",
        "Spam",
        "Purgamentum";
    NotJunk:
        "Not junk",
        "Pas indésirable",
        "No es spam",
        "Nu este spam",
        "Não é spam",
        "Não é spam",
        "Non purgamentum";
    MoveKey:
        "Move… (V)",
        "Déplacer… (V)",
        "Mover… (V)",
        "Mutați… (V)",
        "Mover… (V)",
        "Mover… (V)",
        "Move… (V)";
    NextConversation:
        "Next conversation (j)",
        "Discussion suivante (j)",
        "Conversación siguiente (j)",
        "Conversația următoare (j)",
        "Próxima conversa (j)",
        "Conversa seguinte (j)",
        "Colloquium sequens (j)";
    PreviousConversation:
        "Previous conversation (k)",
        "Discussion précédente (k)",
        "Conversación anterior (k)",
        "Conversația anterioară (k)",
        "Conversa anterior (k)",
        "Conversa anterior (k)",
        "Colloquium prius (k)";
    OneMessage:
        "1 MESSAGE",
        "1 MESSAGE",
        "1 MENSAJE",
        "1 MESAJ",
        "1 MENSAGEM",
        "1 MENSAGEM",
        "1 EPISTULA";
    OpenOriginal:
        "Open original",
        "Ouvrir l’original",
        "Abrir el original",
        "Deschideți originalul",
        "Abrir o original",
        "Abrir o original",
        "Archetypum aperi";
    OpenOriginalArrow:
        "Open original ↗",
        "Ouvrir l’original ↗",
        "Abrir el original ↗",
        "Deschideți originalul ↗",
        "Abrir o original ↗",
        "Abrir o original ↗",
        "Archetypum aperi ↗";
    OpenOriginalHint:
        "A sanitized copy (no scripts, no images, nothing remote) in your browser",
        "Une copie assainie (ni scripts, ni images, rien de distant) dans votre navigateur",
        "Una copia saneada (sin scripts, sin imágenes, nada remoto) en tu navegador",
        "O copie igienizată (fără scripturi, fără imagini, nimic de la distanță) în browser",
        "Uma cópia higienizada (sem scripts, sem imagens, nada remoto) no seu navegador",
        "Uma cópia limpa (sem scripts, sem imagens, nada remoto) no seu navegador",
        "Exemplar purgatum (sine scriptis, sine imaginibus, nihil remotum) in navigatro tuo";
    Loading:
        "Loading…",
        "Chargement…",
        "Cargando…",
        "Se încarcă…",
        "Carregando…",
        "A carregar…",
        "Oneratur…";
    PicturesShown:
        "Showing the pictures in this message.",
        "Les images de ce message sont affichées.",
        "Se muestran las imágenes de este mensaje.",
        "Imaginile din acest mesaj sunt afișate.",
        "As imagens desta mensagem estão sendo exibidas.",
        "As imagens desta mensagem estão a ser mostradas.",
        "Imagines huius epistulae monstrantur.";
    HideImages:
        "Hide images",
        "Masquer les images",
        "Ocultar imágenes",
        "Ascundeți imaginile",
        "Ocultar imagens",
        "Ocultar imagens",
        "Imagines cela";
    PicturesHidden:
        "Pictures in this message are hidden.",
        "Les images de ce message sont masquées.",
        "Las imágenes de este mensaje están ocultas.",
        "Imaginile din acest mesaj sunt ascunse.",
        "As imagens desta mensagem estão ocultas.",
        "As imagens desta mensagem estão ocultas.",
        "Imagines huius epistulae celantur.";
    ShowImages:
        "Show images",
        "Afficher les images",
        "Mostrar imágenes",
        "Afișați imaginile",
        "Mostrar imagens",
        "Mostrar imagens",
        "Imagines monstra";
    PicturesHint:
        "Only pictures carried in the message; nothing is downloaded",
        "Seulement les images contenues dans le message ; rien n’est téléchargé",
        "Solo las imágenes incluidas en el mensaje; no se descarga nada",
        "Doar imaginile incluse în mesaj; nu se descarcă nimic",
        "Só as imagens contidas na mensagem; nada é baixado",
        "Só as imagens contidas na mensagem; nada é transferido",
        "Solae imagines in epistula inclusae; nihil deprimitur";
    NoText:
        "(no text)",
        "(pas de texte)",
        "(sin texto)",
        "(fără text)",
        "(sem texto)",
        "(sem texto)",
        "(sine textu)";
    Appearance:
        "Appearance",
        "Apparence",
        "Apariencia",
        "Aspect",
        "Aparência",
        "Aparência",
        "Species";
    Keyboard:
        "Keyboard",
        "Clavier",
        "Teclado",
        "Tastatură",
        "Teclado",
        "Teclado",
        "Claviatura";
    General:
        "General",
        "Général",
        "General",
        "General",
        "Geral",
        "Geral",
        "Generalia";
    Accounts:
        "Accounts",
        "Comptes",
        "Cuentas",
        "Conturi",
        "Contas",
        "Contas",
        "Rationes";
    CloseSettings:
        "Close settings (Esc)",
        "Fermer les paramètres (Échap)",
        "Cerrar los ajustes (Esc)",
        "Închideți setările (Esc)",
        "Fechar as configurações (Esc)",
        "Fechar as definições (Esc)",
        "Optiones claude (Esc)";
    NotTestedYet:
        "not tested yet",
        "pas encore testé",
        "aún sin probar",
        "netestat încă",
        "ainda não testada",
        "ainda não testada",
        "nondum probata";
    Connected:
        "connected",
        "connecté",
        "conectado",
        "conectat",
        "conectada",
        "ligada",
        "conexa";
    Test:
        "Test",
        "Tester",
        "Probar",
        "Testați",
        "Testar",
        "Testar",
        "Proba";
    ReEnterPassword:
        "Re-enter password",
        "Saisir à nouveau le mot de passe",
        "Volver a escribir la contraseña",
        "Reintroduceți parola",
        "Digitar a senha novamente",
        "Introduzir a palavra-passe novamente",
        "Tesseram iterum insere";
    KeepIt:
        "Keep it",
        "Le garder",
        "Conservarla",
        "Păstrați",
        "Mantê-la",
        "Mantê-la",
        "Serva";
    RemoveDots:
        "Remove…",
        "Retirer…",
        "Quitar…",
        "Eliminați…",
        "Remover…",
        "Remover…",
        "Remove…";
    Theme:
        "Theme",
        "Thème",
        "Tema",
        "Temă",
        "Tema",
        "Tema",
        "Thema";
    ThemeHint:
        "Follow the desktop, or always light or dark",
        "Suivre le bureau, ou toujours clair ou sombre",
        "Seguir el escritorio, o siempre claro u oscuro",
        "Urmați desktopul sau mereu luminos ori întunecat",
        "Seguir a área de trabalho, ou sempre claro ou escuro",
        "Seguir o ambiente de trabalho, ou sempre claro ou escuro",
        "Mensam sequere, aut semper lucidum aut obscurum";
    System:
        "System",
        "Système",
        "Sistema",
        "Sistem",
        "Sistema",
        "Sistema",
        "Systema";
    Light:
        "Light",
        "Clair",
        "Claro",
        "Luminos",
        "Claro",
        "Claro",
        "Lucidum";
    Dark:
        "Dark",
        "Sombre",
        "Oscuro",
        "Întunecat",
        "Escuro",
        "Escuro",
        "Obscurum";
    LightTheme:
        "Light theme",
        "Thème clair",
        "Tema claro",
        "Temă luminoasă",
        "Tema claro",
        "Tema claro",
        "Thema lucidum";
    DarkTheme:
        "Dark theme",
        "Thème sombre",
        "Tema oscuro",
        "Temă întunecată",
        "Tema escuro",
        "Tema escuro",
        "Thema obscurum";
    Density:
        "Density",
        "Densité",
        "Densidad",
        "Densitate",
        "Densidade",
        "Densidade",
        "Densitas";
    DensityHint:
        "How tall the rows of the thread list are",
        "La hauteur des lignes de la liste",
        "La altura de las filas de la lista de conversaciones",
        "Înălțimea rândurilor din lista de conversații",
        "A altura das linhas da lista de conversas",
        "A altura das linhas da lista de conversas",
        "Altitudo versuum in indice colloquiorum";
    Cozy:
        "Cozy 48",
        "Détendue 48",
        "Holgada 48",
        "Aerisită 48",
        "Confortável 48",
        "Confortável 48",
        "Laxa 48";
    Compact:
        "Compact 40",
        "Compacte 40",
        "Compacta 40",
        "Compactă 40",
        "Compacta 40",
        "Compacta 40",
        "Densa 40";
    PreviewDensity:
        "Preview 72",
        "Aperçu 72",
        "Vista previa 72",
        "Previzualizare 72",
        "Prévia 72",
        "Pré-visualização 72",
        "Praevisio 72";
    AppearancePreview:
        "Preview",
        "Aperçu",
        "Vista previa",
        "Previzualizare",
        "Prévia",
        "Pré-visualização",
        "Praevisio";
    SampleSubject1:
        "Photos from Saturday",
        "Les photos de samedi",
        "Fotos del sábado",
        "Fotografii de sâmbătă",
        "Fotos de sábado",
        "Fotografias de sábado",
        "Imagines Saturni diei";
    SampleSnippet1:
        "Here are the best ones; the rest are in the shared album.",
        "Voici les meilleures ; les autres sont dans l’album partagé.",
        "Aquí van las mejores; el resto está en el álbum compartido.",
        "Iată cele mai bune; restul sunt în albumul comun.",
        "Aqui estão as melhores; o resto está no álbum compartilhado.",
        "Aqui estão as melhores; as restantes estão no álbum partilhado.",
        "Hae optimae sunt; ceterae in albo communi.";
    SampleSubject2:
        "Your trip to Lisbon is confirmed",
        "Votre voyage à Lisbonne est confirmé",
        "Tu viaje a Lisboa está confirmado",
        "Călătoria dvs. la Lisabona este confirmată",
        "Sua viagem a Lisboa está confirmada",
        "A sua viagem a Lisboa está confirmada",
        "Iter tuum Olisiponem confirmatum est";
    SampleSnippet2:
        "Booking reference and check-in times inside.",
        "Référence de réservation et horaires d’enregistrement à l’intérieur.",
        "Dentro, el código de reserva y los horarios de facturación.",
        "În interior: codul rezervării și orele de check-in.",
        "Código da reserva e horários de check-in no interior.",
        "Código da reserva e horários de check-in no interior.",
        "Signum reservationis et horae adventus intus.";
    SampleSubject3:
        "Notes from the planning meeting",
        "Notes de la réunion de planification",
        "Notas de la reunión de planificación",
        "Notițe de la ședința de planificare",
        "Notas da reunião de planejamento",
        "Notas da reunião de planeamento",
        "Notae conventus consiliorum";
    SampleSnippet3:
        "Thanks, I added the dates we agreed on.",
        "Merci, j’ai ajouté les dates convenues.",
        "Gracias, añadí las fechas que acordamos.",
        "Mulțumesc, am adăugat datele stabilite.",
        "Obrigado, adicionei as datas que combinamos.",
        "Obrigado, acrescentei as datas que combinámos.",
        "Gratias, dies constitutos addidi.";
    TextSize:
        "Text size",
        "Taille du texte",
        "Tamaño del texto",
        "Dimensiunea textului",
        "Tamanho do texto",
        "Tamanho do texto",
        "Magnitudo textus";
    TextSizeHint:
        "Ctrl + plus and minus change it for a while",
        "Ctrl + plus et moins la changent pour un moment",
        "Ctrl + más y menos lo cambian durante un rato",
        "Ctrl + plus și minus o schimbă temporar",
        "Ctrl + mais e menos o mudam por um tempo",
        "Ctrl + mais e menos alteram-no durante algum tempo",
        "Ctrl + plus et minus eam ad tempus mutant";
    Preset:
        "Preset",
        "Préréglage",
        "Preajuste",
        "Presetare",
        "Predefinição",
        "Predefinição",
        "Praeformatum";
    PresetHint:
        "The keys everything starts from",
        "Les touches de départ",
        "Las teclas de partida",
        "Tastele de pornire",
        "As teclas de partida",
        "As teclas de partida",
        "Claves initiales";
    FilterCommands:
        "Filter the commands",
        "Filtrer les commandes",
        "Filtrar los comandos",
        "Filtrați comenzile",
        "Filtrar os comandos",
        "Filtrar os comandos",
        "Mandata cola";
    Mail:
        "Mail",
        "Courrier",
        "Correo",
        "E-mail",
        "E-mail",
        "Correio",
        "Cursus";
    Navigation:
        "Navigation",
        "Navigation",
        "Navegación",
        "Navigare",
        "Navegação",
        "Navegação",
        "Navigatio";
    Account:
        "Account",
        "Compte",
        "Cuenta",
        "Cont",
        "Conta",
        "Conta",
        "Ratio";
    App:
        "App",
        "Application",
        "Aplicación",
        "Aplicație",
        "Aplicativo",
        "Aplicação",
        "Applicatio";
    Change:
        "Change…",
        "Changer…",
        "Cambiar…",
        "Schimbați…",
        "Alterar…",
        "Alterar…",
        "Muta…";
    UndoSend:
        "Undo send",
        "Annulation de l’envoi",
        "Deshacer el envío",
        "Anularea trimiterii",
        "Desfazer envio",
        "Anular envio",
        "Missionem revoca";
    UndoSendHint:
        "How long a sent message waits before it leaves",
        "Le temps qu’un message attend avant de partir",
        "Cuánto espera un mensaje enviado antes de salir",
        "Cât așteaptă un mesaj trimis înainte de a pleca",
        "Quanto tempo uma mensagem enviada espera antes de sair",
        "Quanto tempo uma mensagem enviada espera antes de sair",
        "Quamdiu epistula missa exspectat antequam abeat";
    Off:
        "Off",
        "Désactivé",
        "Desactivado",
        "Dezactivat",
        "Desativado",
        "Desativado",
        "Exstinctum";
    CheckMail:
        "Check for mail",
        "Relève du courrier",
        "Comprobar correo",
        "Verificarea e-mailului",
        "Verificar e-mail",
        "Verificar correio",
        "Cursum inspice";
    CheckMailHint:
        "How often the inbox is polled without IDLE",
        "La fréquence de relève sans IDLE",
        "Cada cuánto se consulta la bandeja de entrada sin IDLE",
        "Cât de des este verificată căsuța de intrare fără IDLE",
        "Com que frequência a caixa de entrada é consultada sem IDLE",
        "Com que frequência a caixa de entrada é consultada sem IDLE",
        "Quam saepe capsa acceptorum sine IDLE inspicitur";
    MarkAsRead:
        "Mark as read",
        "Marquer comme lu",
        "Marcar como leído",
        "Marcare ca citit",
        "Marcar como lida",
        "Marcar como lida",
        "Ut lectum nota";
    MarkAsReadHint:
        "When an open conversation is marked read (other values: set_setting in the palette)",
        "Quand une discussion ouverte est marquée comme lue (autres valeurs : set_setting dans la palette)",
        "Cuándo se marca como leída una conversación abierta (otros valores: set_setting en la paleta)",
        "Când este marcată ca citită o conversație deschisă (alte valori: set_setting în paletă)",
        "Quando uma conversa aberta é marcada como lida (outros valores: set_setting na paleta)",
        "Quando uma conversa aberta é marcada como lida (outros valores: set_setting na paleta)",
        "Quando colloquium apertum ut lectum notatur (alia: set_setting in tabula)";
    Never:
        "Never",
        "Jamais",
        "Nunca",
        "Niciodată",
        "Nunca",
        "Nunca",
        "Numquam";
    Immediately:
        "Immediately",
        "Immédiatement",
        "Inmediatamente",
        "Imediat",
        "Imediatamente",
        "Imediatamente",
        "Statim";
    After2s:
        "After 2 s",
        "Après 2 s",
        "A los 2 s",
        "După 2 s",
        "Após 2 s",
        "Após 2 s",
        "Post 2 s";
    After5s:
        "After 5 s",
        "Après 5 s",
        "A los 5 s",
        "După 5 s",
        "Após 5 s",
        "Após 5 s",
        "Post 5 s";
    Notifications:
        "Notifications",
        "Notifications",
        "Notificaciones",
        "Notificări",
        "Notificações",
        "Notificações",
        "Nuntii";
    NotificationsHint:
        "New mail, failed sends and agent requests while dail is in the background",
        "Nouveau courrier, envois en échec et demandes d’agents quand dail est en arrière-plan",
        "Correo nuevo, envíos fallidos y solicitudes de agentes mientras dail está en segundo plano",
        "E-mailuri noi, trimiteri eșuate și cereri ale agenților cât timp dail rulează în fundal",
        "E-mails novos, envios com falha e solicitações de agentes enquanto o dail está em segundo plano",
        "Correio novo, envios falhados e pedidos de agentes enquanto o dail está em segundo plano",
        "Epistulae novae, missiones irritae et petitiones agentium dum dail in recessu est";
    UndoHistory:
        "Undo history",
        "Historique d’annulation",
        "Historial de deshacer",
        "Istoricul anulărilor",
        "Histórico de desfazer",
        "Histórico de anulação",
        "Historia revocationum";
    UndoHistoryHint:
        "How many steps undo remembers",
        "Le nombre d’étapes gardées",
        "Cuántos pasos recuerda deshacer",
        "Câți pași rețin anulările",
        "Quantos passos o desfazer lembra",
        "Quantos passos a anulação recorda",
        "Quot gradus revocatio meminit";
    Updates:
        "Updates",
        "Mises à jour",
        "Actualizaciones",
        "Actualizări",
        "Atualizações",
        "Atualizações",
        "Renovationes";
    CheckAutomatically:
        "Check automatically",
        "Vérifier automatiquement",
        "Buscar automáticamente",
        "Verificați automat",
        "Verificar automaticamente",
        "Verificar automaticamente",
        "Sponte inspice";
    CheckAutomaticallyHint:
        "Asks GitHub for the signed release list; nothing about you is sent",
        "Demande à GitHub la liste signée des versions ; rien sur vous n’est envoyé",
        "Pide a GitHub la lista firmada de versiones; no se envía nada sobre ti",
        "Cere de la GitHub lista semnată de versiuni; nu se trimite nimic despre dvs.",
        "Pede ao GitHub a lista assinada de versões; nada sobre você é enviado",
        "Pede ao GitHub a lista assinada de versões; nada sobre si é enviado",
        "A GitHub indicem signatum versionum petit; nihil de te mittitur";
    HowOften:
        "How often",
        "Fréquence",
        "Frecuencia",
        "Frecvență",
        "Frequência",
        "Frequência",
        "Quam saepe";
    HowOftenHint:
        "Between automatic checks",
        "Entre deux vérifications automatiques",
        "Entre búsquedas automáticas",
        "Între verificările automate",
        "Entre verificações automáticas",
        "Entre verificações automáticas",
        "Inter inspectiones spontaneas";
    ChangePassword:
        "Change the password",
        "Changer le mot de passe",
        "Cambiar la contraseña",
        "Schimbați parola",
        "Alterar a senha",
        "Alterar a palavra-passe",
        "Tesseram muta";
    AddAnAccount:
        "Add an account",
        "Ajouter un compte",
        "Añadir una cuenta",
        "Adăugați un cont",
        "Adicionar uma conta",
        "Adicionar uma conta",
        "Rationem adde";
    Cancel:
        "Cancel",
        "Annuler",
        "Cancelar",
        "Anulați",
        "Cancelar",
        "Cancelar",
        "Rescinde";
    Next:
        "Next",
        "Suivant",
        "Siguiente",
        "Înainte",
        "Avançar",
        "Seguinte",
        "Porro";
    EnterEmail:
        "Enter an email address like you@example.com.",
        "Saisissez une adresse comme vous@example.com.",
        "Escribe una dirección como tu@example.com.",
        "Introduceți o adresă precum dvs@example.com.",
        "Digite um endereço como voce@example.com.",
        "Introduza um endereço como voce@example.com.",
        "Inscriptionem insere ut tu@example.com.";
    EmailAddress:
        "Email address",
        "Adresse e-mail",
        "Dirección de correo",
        "Adresă de e-mail",
        "Endereço de e-mail",
        "Endereço de e-mail",
        "Inscriptio cursus electronici";
    NameOptional:
        "Name (optional)",
        "Nom (facultatif)",
        "Nombre (opcional)",
        "Nume (opțional)",
        "Nome (opcional)",
        "Nome (opcional)",
        "Nomen (ad libitum)";
    ShownInSidebar:
        "shown in the sidebar",
        "affiché dans la barre latérale",
        "se muestra en la barra lateral",
        "afișat în bara laterală",
        "exibido na barra lateral",
        "mostrado na barra lateral",
        "in tabula laterali monstratur";
    ImapServer:
        "IMAP server",
        "Serveur IMAP",
        "Servidor IMAP",
        "Server IMAP",
        "Servidor IMAP",
        "Servidor IMAP",
        "Servitor IMAP";
    SmtpServer:
        "SMTP server",
        "Serveur SMTP",
        "Servidor SMTP",
        "Server SMTP",
        "Servidor SMTP",
        "Servidor SMTP",
        "Servitor SMTP";
    HostPortHint:
        "host:port, optionally :tls or :starttls (143 and 587 use STARTTLS)",
        "hôte:port, avec :tls ou :starttls en option (143 et 587 utilisent STARTTLS)",
        "host:puerto, opcionalmente :tls o :starttls (143 y 587 usan STARTTLS)",
        "gazdă:port, opțional :tls sau :starttls (143 și 587 folosesc STARTTLS)",
        "host:porta, opcionalmente :tls ou :starttls (143 e 587 usam STARTTLS)",
        "anfitrião:porta, opcionalmente :tls ou :starttls (143 e 587 usam STARTTLS)",
        "hospes:porta, ad libitum :tls aut :starttls (143 et 587 STARTTLS adhibent)";
    FindServers:
        "Find servers",
        "Trouver les serveurs",
        "Buscar servidores",
        "Găsiți serverele",
        "Encontrar servidores",
        "Encontrar servidores",
        "Servitores inveni";
    Looking:
        "Looking…",
        "Recherche…",
        "Buscando…",
        "Se caută…",
        "Procurando…",
        "A procurar…",
        "Quaeritur…";
    FindServersHint:
        "Asks the provider's web server, then Mozilla's list.",
        "Interroge le serveur web du fournisseur, puis la liste de Mozilla.",
        "Pregunta al servidor web del proveedor y luego a la lista de Mozilla.",
        "Întreabă serverul web al furnizorului, apoi lista Mozilla.",
        "Consulta o servidor web do provedor e depois a lista da Mozilla.",
        "Consulta o servidor web do fornecedor e depois a lista da Mozilla.",
        "Servitorem interretialem provisoris, deinde indicem Mozillae interrogat.";
    ServersNote:
        "host:port, optionally :tls or :starttls. Find servers asks the provider, then Mozilla's list.",
        "hôte:port, avec :tls ou :starttls en option. « Trouver les serveurs » interroge le fournisseur, puis la liste de Mozilla.",
        "host:puerto, opcionalmente :tls o :starttls. «Buscar servidores» pregunta al proveedor y luego a la lista de Mozilla.",
        "gazdă:port, opțional :tls sau :starttls. „Găsiți serverele” întreabă furnizorul, apoi lista Mozilla.",
        "host:porta, opcionalmente :tls ou :starttls. “Encontrar servidores” consulta o provedor e depois a lista da Mozilla.",
        "anfitrião:porta, opcionalmente :tls ou :starttls. “Encontrar servidores” consulta o fornecedor e depois a lista da Mozilla.",
        "hospes:porta, ad libitum :tls aut :starttls. «Servitores inveni» provisorem, deinde indicem Mozillae interrogat.";
    Back:
        "Back",
        "Retour",
        "Atrás",
        "Înapoi",
        "Voltar",
        "Retroceder",
        "Retro";
    OauthOnly:
        "{provider} addresses need a sign-in that dail does not support yet. Use another address for now.",
        "Les adresses {provider} demandent une connexion que dail ne prend pas encore en charge. Utilisez une autre adresse pour l’instant.",
        "Las direcciones de {provider} necesitan un inicio de sesión que dail aún no admite. Por ahora, usa otra dirección.",
        "Adresele {provider} necesită o autentificare pe care dail nu o acceptă încă. Deocamdată folosiți altă adresă.",
        "Endereços {provider} precisam de um login que o dail ainda não suporta. Por enquanto, use outro endereço.",
        "Os endereços {provider} precisam de um início de sessão que o dail ainda não suporta. Por agora, use outro endereço.",
        "Inscriptiones {provider} ingressum requirunt quem dail nondum sustinet. Interim alia inscriptione utere.";
    CreateAppPassword:
        "Create an App Password",
        "Créer un mot de passe d’application",
        "Crear una contraseña de aplicación",
        "Creați o parolă pentru aplicații",
        "Criar uma senha de app",
        "Criar uma palavra-passe de aplicação",
        "Tesseram applicationis crea";
    PrivacyPolicy:
        "Privacy policy",
        "Politique de confidentialité",
        "Política de privacidad",
        "Politica de confidențialitate",
        "Política de privacidade",
        "Política de privacidade",
        "Ratio secreti";
    AppPassword:
        "App Password",
        "Mot de passe d’application",
        "Contraseña de aplicación",
        "Parolă pentru aplicații",
        "Senha de app",
        "Palavra-passe de aplicação",
        "Tessera applicationis";
    Password:
        "Password",
        "Mot de passe",
        "Contraseña",
        "Parolă",
        "Senha",
        "Palavra-passe",
        "Tessera";
    KeyringOnly:
        "Stored in the system keyring only; dail never writes it anywhere else.",
        "Stocké uniquement dans le trousseau du système ; dail ne l’écrit nulle part ailleurs.",
        "Se guarda solo en el llavero del sistema; dail nunca la escribe en otro sitio.",
        "Este stocată doar în portofelul de chei al sistemului; dail nu o scrie niciodată în altă parte.",
        "Salva só no chaveiro do sistema; o dail nunca a grava em outro lugar.",
        "Guardada apenas no porta-chaves do sistema; o dail nunca a escreve noutro lugar.",
        "Tantum in clavario systematis servatur; dail eam numquam alibi scribit.";
    KeystoreOnly:
        "Stored in Keystore-protected storage only; dail never writes it anywhere else.",
        "Stocké uniquement dans un espace protégé par le Keystore ; dail ne l’écrit nulle part ailleurs.",
        "Se guarda solo en un almacenamiento protegido por Keystore; dail nunca la escribe en otro sitio.",
        "Este stocată doar într-un spațiu protejat de Keystore; dail nu o scrie niciodată în altă parte.",
        "Salva só em armazenamento protegido pelo Keystore; o dail nunca a grava em outro lugar.",
        "Guardada apenas em armazenamento protegido pelo Keystore; o dail nunca a escreve noutro lugar.",
        "Tantum in repositorio a Keystore tuto servatur; dail eam numquam alibi scribit.";
    SaveAndTest:
        "Save and test",
        "Enregistrer et tester",
        "Guardar y probar",
        "Salvați și testați",
        "Salvar e testar",
        "Guardar e testar",
        "Serva et proba";
    AddAndTest:
        "Add and test",
        "Ajouter et tester",
        "Añadir y probar",
        "Adăugați și testați",
        "Adicionar e testar",
        "Adicionar e testar",
        "Adde et proba";
    ConnectedLogin:
        "Connected. IMAP and SMTP both accepted the login.",
        "Connecté. IMAP et SMTP ont accepté tous deux la connexion.",
        "Conectado. IMAP y SMTP aceptaron el inicio de sesión.",
        "Conectat. Atât IMAP, cât și SMTP au acceptat autentificarea.",
        "Conectado. IMAP e SMTP aceitaram o login.",
        "Ligado. IMAP e SMTP aceitaram o início de sessão.",
        "Conexum. IMAP et SMTP ingressum ambo acceperunt.";
    Close:
        "Close",
        "Fermer",
        "Cerrar",
        "Închideți",
        "Fechar",
        "Fechar",
        "Claude";
    KeepAnyway:
        "Keep it anyway",
        "Le garder quand même",
        "Conservarla de todos modos",
        "Păstrați oricum",
        "Manter mesmo assim",
        "Manter mesmo assim",
        "Nihilominus serva";
    TryAnotherPassword:
        "Try another password",
        "Essayer un autre mot de passe",
        "Probar otra contraseña",
        "Încercați altă parolă",
        "Tentar outra senha",
        "Tentar outra palavra-passe",
        "Aliam tesseram conare";
    Provider1:
        "1 / 4  Provider",
        "1 / 4  Fournisseur",
        "1 / 4  Proveedor",
        "1 / 4  Furnizor",
        "1 / 4  Provedor",
        "1 / 4  Fornecedor",
        "1 / 4  Provisor";
    Address2:
        "2 / 4  Address",
        "2 / 4  Adresse",
        "2 / 4  Dirección",
        "2 / 4  Adresă",
        "2 / 4  Endereço",
        "2 / 4  Endereço",
        "2 / 4  Inscriptio";
    Password3:
        "3 / 4  Password",
        "3 / 4  Mot de passe",
        "3 / 4  Contraseña",
        "3 / 4  Parolă",
        "3 / 4  Senha",
        "3 / 4  Palavra-passe",
        "3 / 4  Tessera";
    Connection4:
        "4 / 4  Connection",
        "4 / 4  Connexion",
        "4 / 4  Conexión",
        "4 / 4  Conexiune",
        "4 / 4  Conexão",
        "4 / 4  Ligação",
        "4 / 4  Conexio";
    AMessage:
        "a message",
        "un message",
        "un mensaje",
        "un mesaj",
        "uma mensagem",
        "uma mensagem",
        "epistulam";
    ApproveSend:
        "Approve send",
        "Approuver l’envoi",
        "Aprobar el envío",
        "Aprobați trimiterea",
        "Aprovar envio",
        "Aprovar envio",
        "Missionem proba";
    Edit:
        "Edit",
        "Modifier",
        "Editar",
        "Editați",
        "Editar",
        "Editar",
        "Recense";
    Reject:
        "Reject",
        "Rejeter",
        "Rechazar",
        "Respingeți",
        "Rejeitar",
        "Rejeitar",
        "Reice";
    StillChecked:
        "attachments are still being checked",
        "les pièces jointes sont encore en cours de vérification",
        "aún se están comprobando los adjuntos",
        "atașamentele sunt încă în verificare",
        "os anexos ainda estão sendo verificados",
        "os anexos ainda estão a ser verificados",
        "adiuncta adhuc inspiciuntur";
    SaveAttachmentsInto:
        "Save attachments into",
        "Enregistrer les pièces jointes dans",
        "Guardar los adjuntos en",
        "Salvați atașamentele în",
        "Salvar os anexos em",
        "Guardar os anexos em",
        "Adiuncta serva in";
    OneFile:
        "1 file",
        "1 fichier",
        "1 archivo",
        "1 fișier",
        "1 arquivo",
        "1 ficheiro",
        "1 documentum";
    SaveAll:
        "Save all",
        "Tout enregistrer",
        "Guardar todo",
        "Salvați tot",
        "Salvar tudo",
        "Guardar tudo",
        "Omnia serva";
    SaveAllDots:
        "Save all…",
        "Tout enregistrer…",
        "Guardar todo…",
        "Salvați tot…",
        "Salvar tudo…",
        "Guardar tudo…",
        "Omnia serva…";
    OpenArrow:
        "Open ↗",
        "Ouvrir ↗",
        "Abrir ↗",
        "Deschideți ↗",
        "Abrir ↗",
        "Abrir ↗",
        "Aperi ↗";
    HidePreview:
        "Hide preview",
        "Masquer l’aperçu",
        "Ocultar la vista previa",
        "Ascundeți previzualizarea",
        "Ocultar a prévia",
        "Ocultar a pré-visualização",
        "Praevisionem cela";
    Preview:
        "Preview",
        "Aperçu",
        "Vista previa",
        "Previzualizare",
        "Prévia",
        "Pré-visualização",
        "Praevisio";
    SaveDots:
        "Save…",
        "Enregistrer…",
        "Guardar…",
        "Salvați…",
        "Salvar…",
        "Guardar…",
        "Serva…";
    LoadingPreview:
        "Loading the preview…",
        "Chargement de l’aperçu…",
        "Cargando la vista previa…",
        "Se încarcă previzualizarea…",
        "Carregando a prévia…",
        "A carregar a pré-visualização…",
        "Praevisio oneratur…";
    Dismiss:
        "Dismiss",
        "Ignorer",
        "Descartar",
        "Închideți",
        "Dispensar",
        "Ignorar",
        "Dimitte";
    NothingMatches:
        "Nothing matches.",
        "Aucun résultat.",
        "Nada coincide.",
        "Nimic nu se potrivește.",
        "Nada corresponde.",
        "Nada corresponde.",
        "Nihil congruit.";
    Error:
        "Error",
        "Erreur",
        "Error",
        "Eroare",
        "Erro",
        "Erro",
        "Error";
    Sending:
        "Sending…",
        "Envoi…",
        "Enviando…",
        "Se trimite…",
        "Enviando…",
        "A enviar…",
        "Mittitur…";
    Then:
        "then",
        "puis",
        "luego",
        "apoi",
        "depois",
        "depois",
        "deinde";
    NotSentNotice:
        "Not sent.",
        "Non envoyé.",
        "No enviado.",
        "Netrimis.",
        "Não enviada.",
        "Não enviada.",
        "Non missa.";
    AttachmentsChecked:
        "Its attachments are still being checked.",
        "Ses pièces jointes sont encore en cours de vérification.",
        "Aún se están comprobando sus adjuntos.",
        "Atașamentele sale sunt încă în verificare.",
        "Seus anexos ainda estão sendo verificados.",
        "Os seus anexos ainda estão a ser verificados.",
        "Adiuncta eius adhuc inspiciuntur.";
    SendRequests:
        " Send requests ",
        " Demandes d’envoi ",
        " Solicitudes de envío ",
        " Cereri de trimitere ",
        " Solicitações de envio ",
        " Pedidos de envio ",
        " Petitiones missionis ";
    NoSendRequests:
        "No send requests.",
        "Aucune demande d’envoi.",
        "No hay solicitudes de envío.",
        "Nu există cereri de trimitere.",
        "Nenhuma solicitação de envio.",
        "Nenhum pedido de envio.",
        "Nullae petitiones missionis.";
    ApprovalsCheckingFooter:
        "attachments are being checked · e edit · r reject · q close",
        "vérification des pièces jointes · e modifier · r rejeter · q fermer",
        "comprobando adjuntos · e editar · r rechazar · q cerrar",
        "se verifică atașamentele · e editați · r respingeți · q închideți",
        "verificando anexos · e editar · r rejeitar · q fechar",
        "a verificar anexos · e editar · r rejeitar · q fechar",
        "adiuncta inspiciuntur · e recense · r reice · q claude";
    ApprovalsFooter:
        "a approve · e edit · r reject · j/k next · q close",
        "a approuver · e modifier · r rejeter · j/k suivant · q fermer",
        "a aprobar · e editar · r rechazar · j/k siguiente · q cerrar",
        "a aprobați · e editați · r respingeți · j/k următoarea · q închideți",
        "a aprovar · e editar · r rejeitar · j/k próxima · q fechar",
        "a aprovar · e editar · r rejeitar · j/k seguinte · q fechar",
        "a proba · e recense · r reice · j/k sequens · q claude";
    QClose:
        "q close",
        "q fermer",
        "q cerrar",
        "q închideți",
        "q fechar",
        "q fechar",
        "q claude";
    OriginalBelowTui:
        "(the original is added below when sent)",
        "(l’original est ajouté dessous à l’envoi)",
        "(el original se añade debajo al enviar)",
        "(originalul este adăugat dedesubt la trimitere)",
        "(o original é adicionado abaixo ao enviar)",
        "(o original é adicionado abaixo ao enviar)",
        "(archetypum infra additur cum mittitur)";
    ReadOnlyWhileSent:
        "read-only while it is sent · z cancels",
        "lecture seule pendant l’envoi · z annule",
        "solo lectura mientras se envía · z cancela",
        "doar citire cât se trimite · z anulează",
        "somente leitura enquanto é enviada · z cancela",
        "só de leitura enquanto é enviada · z cancela",
        "tantum legendum dum mittitur · z rescindit";
    ComposerFooter:
        "Ctrl+S send · Ctrl+O $EDITOR · Ctrl+K “Attach file” · Esc close",
        "Ctrl+S envoyer · Ctrl+O $EDITOR · Ctrl+K « Attach file » · Échap fermer",
        "Ctrl+S enviar · Ctrl+O $EDITOR · Ctrl+K «Attach file» · Esc cerrar",
        "Ctrl+S trimiteți · Ctrl+O $EDITOR · Ctrl+K „Attach file” · Esc închideți",
        "Ctrl+S enviar · Ctrl+O $EDITOR · Ctrl+K “Attach file” · Esc fechar",
        "Ctrl+S enviar · Ctrl+O $EDITOR · Ctrl+K “Attach file” · Esc fechar",
        "Ctrl+S mitte · Ctrl+O $EDITOR · Ctrl+K «Attach file» · Esc claude";
    FromColon:
        "From: ",
        "De : ",
        "De:  ",
        "De la:  ",
        "De:  ",
        "De:  ",
        "A:  ";
    PaletteTitle:
        " Palette ",
        " Palette ",
        " Paleta ",
        " Paletă ",
        " Paleta ",
        " Paleta ",
        " Tabula ";
    NothingMatchesTui:
        "  Nothing matches.",
        "  Aucun résultat.",
        "  Nada coincide.",
        "  Nimic nu se potrivește.",
        "  Nada corresponde.",
        "  Nada corresponde.",
        "  Nihil congruit.";
    EnterUseEscBack:
        "Enter to use · Esc to go back",
        "Entrée pour utiliser · Échap pour revenir",
        "Intro para usar · Esc para volver",
        "Enter pentru a folosi · Esc pentru a reveni",
        "Enter para usar · Esc para voltar",
        "Enter para usar · Esc para voltar",
        "Enter ad utendum · Esc ad redeundum";
    PaletteFooter:
        "↑↓ move · Enter choose · Esc back",
        "↑↓ déplacer · Entrée choisir · Échap retour",
        "↑↓ mover · Intro elegir · Esc atrás",
        "↑↓ deplasare · Enter alegeți · Esc înapoi",
        "↑↓ mover · Enter escolher · Esc voltar",
        "↑↓ mover · Enter escolher · Esc voltar",
        "↑↓ move · Enter elige · Esc retro";
    ScrollClose:
        "↑↓ Space b scroll · q or Esc close",
        "↑↓ Espace b défiler · q ou Échap fermer",
        "↑↓ Espacio b desplazar · q o Esc cerrar",
        "↑↓ Spațiu b derulare · q sau Esc închideți",
        "↑↓ Espaço b rolar · q ou Esc fechar",
        "↑↓ Espaço b deslocar · q ou Esc fechar",
        "↑↓ Spatium b volve · q aut Esc claude";
    ScrollView:
        "↑↓ Space b scroll · r view · q or Esc close",
        "↑↓ Espace b défiler · r vue · q ou Échap fermer",
        "↑↓ Espacio b desplazar · r vista · q o Esc cerrar",
        "↑↓ Spațiu b derulare · r vizualizare · q sau Esc închideți",
        "↑↓ Espaço b rolar · r visualização · q ou Esc fechar",
        "↑↓ Espaço b deslocar · r vista · q ou Esc fechar",
        "↑↓ Spatium b volve · r conspectus · q aut Esc claude";
    ScrollOpen:
        "↑↓ scroll · Tab Enter open · r raw data · q or Esc close",
        "↑↓ défiler · Tab Entrée ouvrir · r données brutes · q ou Échap fermer",
        "↑↓ desplazar · Tab Intro abrir · r datos sin procesar · q o Esc cerrar",
        "↑↓ derulare · Tab Enter deschideți · r date brute · q sau Esc închideți",
        "↑↓ rolar · Tab Enter abrir · r dados brutos · q ou Esc fechar",
        "↑↓ deslocar · Tab Enter abrir · r dados brutos · q ou Esc fechar",
        "↑↓ volve · Tab Enter aperi · r data cruda · q aut Esc claude";
    NoConversationTui:
        "No conversation open. Enter opens the selected one.",
        "Aucune discussion ouverte. Entrée ouvre celle qui est sélectionnée.",
        "Ninguna conversación abierta. Intro abre la seleccionada.",
        "Nicio conversație deschisă. Enter o deschide pe cea selectată.",
        "Nenhuma conversa aberta. Enter abre a selecionada.",
        "Nenhuma conversa aberta. Enter abre a selecionada.",
        "Nullum colloquium apertum. Enter selectum aperit.";
    Keys:
        "Keys",
        "Touches",
        "Teclas",
        "Taste",
        "Teclas",
        "Teclas",
        "Claves";
    NotTested:
        "not tested",
        "non testé",
        "sin probar",
        "netestat",
        "não testada",
        "não testada",
        "non probata";
    SettingsTitle:
        " Settings ",
        " Paramètres ",
        " Ajustes ",
        " Setări ",
        " Configurações ",
        " Definições ",
        " Optiones ";
    SettingsFooter:
        "Tab section · j/k move · Enter change · Esc close",
        "Tab section · j/k déplacer · Entrée changer · Échap fermer",
        "Tab sección · j/k mover · Intro cambiar · Esc cerrar",
        "Tab secțiune · j/k deplasare · Enter schimbați · Esc închideți",
        "Tab seção · j/k mover · Enter alterar · Esc fechar",
        "Tab secção · j/k mover · Enter alterar · Esc fechar",
        "Tab sectio · j/k move · Enter muta · Esc claude";
    AddAccountTitle:
        " Add account ",
        " Ajouter un compte ",
        " Añadir cuenta ",
        " Adăugați un cont ",
        " Adicionar conta ",
        " Adicionar conta ",
        " Rationem adde ";
    Step1:
        "1/4 Provider",
        "1/4 Fournisseur",
        "1/4 Proveedor",
        "1/4 Furnizor",
        "1/4 Provedor",
        "1/4 Fornecedor",
        "1/4 Provisor";
    Step2:
        "2/4 Address",
        "2/4 Adresse",
        "2/4 Dirección",
        "2/4 Adresă",
        "2/4 Endereço",
        "2/4 Endereço",
        "2/4 Inscriptio";
    Step3:
        "3/4 Password",
        "3/4 Mot de passe",
        "3/4 Contraseña",
        "3/4 Parolă",
        "3/4 Senha",
        "3/4 Palavra-passe",
        "3/4 Tessera";
    Step4:
        "4/4 Connection",
        "4/4 Connexion",
        "4/4 Conexión",
        "4/4 Conexiune",
        "4/4 Conexão",
        "4/4 Ligação",
        "4/4 Conexio";
    ChooseFooter:
        "↑↓ choose · Enter next · Esc cancel",
        "↑↓ choisir · Entrée suivant · Échap annuler",
        "↑↓ elegir · Intro siguiente · Esc cancelar",
        "↑↓ alegeți · Enter înainte · Esc anulați",
        "↑↓ escolher · Enter avançar · Esc cancelar",
        "↑↓ escolher · Enter seguinte · Esc cancelar",
        "↑↓ elige · Enter porro · Esc rescinde";
    Email:
        "Email",
        "E-mail",
        "Correo",
        "E-mail",
        "E-mail",
        "E-mail",
        "Cursus electronicus";
    ImapHostPort:
        "IMAP host:port",
        "IMAP hôte:port",
        "IMAP host:puerto",
        "IMAP gazdă:port",
        "IMAP host:porta",
        "IMAP anfitrião:porta",
        "IMAP hospes:porta";
    SmtpHostPort:
        "SMTP host:port",
        "SMTP hôte:port",
        "SMTP host:puerto",
        "SMTP gazdă:port",
        "SMTP host:porta",
        "SMTP anfitrião:porta",
        "SMTP hospes:porta";
    PortsHint:
        "Ports 143 and 587 use STARTTLS, others TLS; add :tls or :starttls to choose.",
        "Les ports 143 et 587 utilisent STARTTLS, les autres TLS ; ajoutez :tls ou :starttls pour choisir.",
        "Los puertos 143 y 587 usan STARTTLS, los demás TLS; añade :tls o :starttls para elegir.",
        "Porturile 143 și 587 folosesc STARTTLS, celelalte TLS; adăugați :tls sau :starttls pentru a alege.",
        "As portas 143 e 587 usam STARTTLS, as demais TLS; adicione :tls ou :starttls para escolher.",
        "As portas 143 e 587 usam STARTTLS, as restantes TLS; acrescente :tls ou :starttls para escolher.",
        "Portae 143 et 587 STARTTLS adhibent, ceterae TLS; adde :tls aut :starttls ad eligendum.";
    CtrlFHint:
        "Ctrl+F finds the servers (asks the provider's web server, then Mozilla's list).",
        "Ctrl+F trouve les serveurs (interroge le serveur web du fournisseur, puis la liste de Mozilla).",
        "Ctrl+F busca los servidores (pregunta al servidor web del proveedor y luego a la lista de Mozilla).",
        "Ctrl+F găsește serverele (întreabă serverul web al furnizorului, apoi lista Mozilla).",
        "Ctrl+F encontra os servidores (consulta o servidor web do provedor e depois a lista da Mozilla).",
        "Ctrl+F encontra os servidores (consulta o servidor web do fornecedor e depois a lista da Mozilla).",
        "Ctrl+F servitores invenit (servitorem interretialem provisoris, deinde indicem Mozillae interrogat).";
    FieldFooter:
        "Tab field · Enter next · Esc back",
        "Tab champ · Entrée suivant · Échap retour",
        "Tab campo · Intro siguiente · Esc atrás",
        "Tab câmp · Enter înainte · Esc înapoi",
        "Tab campo · Enter avançar · Esc voltar",
        "Tab campo · Enter seguinte · Esc retroceder",
        "Tab campus · Enter porro · Esc retro";
    KeyringTui:
        "It goes to the system keyring, never into dail's files.",
        "Il va dans le trousseau du système, jamais dans les fichiers de dail.",
        "Va al llavero del sistema, nunca a los archivos de dail.",
        "Ajunge în portofelul de chei al sistemului, niciodată în fișierele dail.",
        "Vai para o chaveiro do sistema, nunca para os arquivos do dail.",
        "Vai para o porta-chaves do sistema, nunca para os ficheiros do dail.",
        "In clavarium systematis it, numquam in documenta dail.";
    SaveTestFooter:
        "Enter save and test · Esc cancel",
        "Entrée enregistrer et tester · Échap annuler",
        "Intro guardar y probar · Esc cancelar",
        "Enter salvați și testați · Esc anulați",
        "Enter salvar e testar · Esc cancelar",
        "Enter guardar e testar · Esc cancelar",
        "Enter serva et proba · Esc rescinde";
    AddTestFooter:
        "Enter add and test · Esc back",
        "Entrée ajouter et tester · Échap retour",
        "Intro añadir y probar · Esc atrás",
        "Enter adăugați și testați · Esc înapoi",
        "Enter adicionar e testar · Esc voltar",
        "Enter adicionar e testar · Esc retroceder",
        "Enter adde et proba · Esc retro";
    ConnectedCap:
        "Connected.",
        "Connecté.",
        "Conectado.",
        "Conectat.",
        "Conectado.",
        "Ligado.",
        "Conexum.";
    SyncCloseFooter:
        "Enter syncs now · Esc closes",
        "Entrée synchronise · Échap ferme",
        "Intro sincroniza ahora · Esc cierra",
        "Enter sincronizează acum · Esc închide",
        "Enter sincroniza agora · Esc fecha",
        "Enter sincroniza agora · Esc fecha",
        "Enter nunc synchronizat · Esc claudit";
    LoginFailedCap:
        "The login failed:",
        "La connexion a échoué :",
        "Error al iniciar sesión:",
        "Autentificarea a eșuat:",
        "O login falhou:",
        "O início de sessão falhou:",
        "Ingressus defecit:";
    PasswordClose:
        "p another password · k close",
        "p autre mot de passe · k fermer",
        "p otra contraseña · k cerrar",
        "p altă parolă · k închideți",
        "p outra senha · k fechar",
        "p outra palavra-passe · k fechar",
        "p alia tessera · k claude";
    PasswordKeep:
        "p another password · k keep it anyway",
        "p autre mot de passe · k le garder quand même",
        "p otra contraseña · k conservarla de todos modos",
        "p altă parolă · k păstrați oricum",
        "p outra senha · k manter mesmo assim",
        "p outra palavra-passe · k manter mesmo assim",
        "p alia tessera · k nihilominus serva";
    TestingLogin:
        "Testing the login…",
        "Test de la connexion…",
        "Probando el inicio de sesión…",
        "Se testează autentificarea…",
        "Testando o login…",
        "A testar o início de sessão…",
        "Ingressus probatur…";
    QuitConfirm:
        "Quitting cancels the send still in its undo window. q quits, any other key stays",
        "Quitter annule l’envoi encore dans son délai d’annulation. q quitte, toute autre touche reste",
        "Salir cancela el envío que sigue en su plazo para deshacer. q sale, cualquier otra tecla se queda",
        "Ieșirea anulează trimiterea aflată încă în intervalul de anulare. q iese, orice altă tastă rămâne",
        "Sair cancela o envio que ainda está no prazo para desfazer. q sai, qualquer outra tecla fica",
        "Sair cancela o envio que ainda está no prazo de anulação. q sai, qualquer outra tecla fica",
        "Exitus missionem rescindit quae adhuc in spatio revocandi est. q exit, quaevis alia clavis manet";
    PickMailboxTui:
        "Pick a mailbox: g i for the inbox.",
        "Choisissez un dossier : g i pour le courrier entrant.",
        "Elige una carpeta: g i para la bandeja de entrada.",
        "Alegeți un dosar: g i pentru căsuța de intrare.",
        "Escolha uma pasta: g i para a caixa de entrada.",
        "Escolha uma pasta: g i para a caixa de entrada.",
        "Capsam elige: g i pro capsa acceptorum.";
    PressEnterToAdd:
        "Press Enter to add one here, or run in another shell:",
        "Appuyez sur Entrée pour en ajouter un ici, ou lancez dans un autre terminal :",
        "Pulsa Intro para añadir una aquí, o ejecuta en otro terminal:",
        "Apăsați Enter pentru a adăuga unul aici sau rulați în alt terminal:",
        "Pressione Enter para adicionar uma aqui, ou execute em outro terminal:",
        "Prima Enter para adicionar uma aqui, ou execute noutro terminal:",
        "Preme Enter ut hic unam addas, aut in alio terminali exsequere:";
    AccountShowsUp:
        "The account shows up here once it is added.",
        "Le compte apparaît ici une fois ajouté.",
        "La cuenta aparece aquí en cuanto se añade.",
        "Contul apare aici după ce este adăugat.",
        "A conta aparece aqui quando for adicionada.",
        "A conta aparece aqui depois de adicionada.",
        "Ratio hic apparet postquam addita est.";
    SetEditor:
        "Set $VISUAL or $EDITOR to write the body in your editor",
        "Définissez $VISUAL ou $EDITOR pour écrire le message dans votre éditeur",
        "Define $VISUAL o $EDITOR para escribir el mensaje en tu editor",
        "Setați $VISUAL sau $EDITOR pentru a scrie mesajul în editorul dvs.",
        "Defina $VISUAL ou $EDITOR para escrever a mensagem no seu editor",
        "Defina $VISUAL ou $EDITOR para escrever a mensagem no seu editor",
        "$VISUAL aut $EDITOR constitue ut epistulam in editore tuo scribas";
    StillHere:
        "Still here; the send goes out when its window closes.",
        "Toujours là ; l’envoi part à la fin de son délai.",
        "Sigue aquí; el envío sale cuando termina su plazo.",
        "Încă aici; mesajul pleacă la încheierea intervalului.",
        "Ainda aqui; o envio sai quando o prazo terminar.",
        "Ainda aqui; o envio sai quando o prazo terminar.",
        "Adhuc hic; missio abit cum spatium finitur.";
    MouseOn:
        "Mouse on: tap to select, swipe to scroll",
        "Souris active : touchez pour sélectionner, glissez pour défiler",
        "Ratón activado: toca para seleccionar, desliza para desplazar",
        "Mouse activ: atingeți pentru a selecta, glisați pentru a derula",
        "Mouse ativado: toque para selecionar, deslize para rolar",
        "Rato ativado: toque para selecionar, deslize para deslocar",
        "Mus activus: tange ut seligas, verre ut volvas";
    MouseOff:
        "Mouse off: select text with the terminal; m turns it back on",
        "Souris inactive : sélectionnez le texte avec le terminal ; m la réactive",
        "Ratón desactivado: selecciona el texto con el terminal; m lo vuelve a activar",
        "Mouse inactiv: selectați textul cu terminalul; m îl reactivează",
        "Mouse desativado: selecione o texto com o terminal; m o reativa",
        "Rato desativado: selecione o texto com o terminal; m reativa-o",
        "Mus inactivus: textum terminali selige; m eum restituit";
    HelpNote:
        "Problems, the keys of the registry, the TUI's own. q, Esc or ? closes.",
        "Problèmes, touches du registre, touches du terminal. q, Échap ou ? ferme.",
        "Problemas, las teclas del registro y las propias del terminal. q, Esc o ? cierra.",
        "Probleme, tastele registrului și cele ale terminalului. q, Esc sau ? închide.",
        "Problemas, as teclas do registro e as do próprio terminal. q, Esc ou ? fecha.",
        "Problemas, as teclas do registo e as do próprio terminal. q, Esc ou ? fecha.",
        "Difficultates, claves indicis et propriae terminalis. q, Esc aut ? claudit.";
    TerminalTooSmall:
        "Terminal too small",
        "Terminal trop petit",
        "Terminal demasiado pequeño",
        "Terminal prea mic",
        "Terminal pequeno demais",
        "Terminal demasiado pequeno",
        "Terminale nimis parvum";
    Mailboxes:
        "Mailboxes",
        "Dossiers",
        "Carpetas",
        "Dosare",
        "Pastas",
        "Pastas",
        "Capsae";
    Message:
        "Message",
        "Message",
        "Mensaje",
        "Mesaj",
        "Mensagem",
        "Mensagem",
        "Epistula";
    Draft:
        "Draft",
        "Brouillon",
        "Borrador",
        "Ciornă",
        "Rascunho",
        "Rascunho",
        "Adumbratio";
    Conversations:
        "Conversations",
        "Discussions",
        "Conversaciones",
        "Conversații",
        "Conversas",
        "Conversas",
        "Colloquia";
    FocusFooter:
        "Focus · palette: Exit focus",
        "Focus · palette : Exit focus",
        "Enfoque · paleta: Exit focus",
        "Concentrare · paletă: Exit focus",
        "Foco · paleta: Exit focus",
        "Foco · paleta: Exit focus",
        "Intentio · tabula: Exit focus";
    OfflineSuffix:
        " offline",
        " hors ligne",
        " sin conexión",
        " offline",
        " offline",
        " offline",
        " sine conexione";
    FixPalettePassword:
        " (palette: Change account password)",
        " (palette : Change account password)",
        " (paleta: Change account password)",
        " (paletă: Change account password)",
        " (paleta: Change account password)",
        " (paleta: Change account password)",
        " (tabula: Change account password)";
    FixPaletteSync:
        " (palette: Sync now)",
        " (palette : Sync now)",
        " (paleta: Sync now)",
        " (paletă: Sync now)",
        " (paleta: Sync now)",
        " (paleta: Sync now)",
        " (tabula: Sync now)";
    FixPaletteTest:
        " (palette: Test account connection)",
        " (palette : Test account connection)",
        " (paleta: Test account connection)",
        " (paletă: Test account connection)",
        " (paleta: Test account connection)",
        " (paleta: Test account connection)",
        " (tabula: Test account connection)";
    UpdatesQuestion:
        "Check for updates? (settings: update_checks)",
        "Rechercher des mises à jour ? (paramètres : update_checks)",
        "¿Buscar actualizaciones? (ajustes: update_checks)",
        "Căutați actualizări? (setări: update_checks)",
        "Procurar atualizações? (configurações: update_checks)",
        "Procurar atualizações? (definições: update_checks)",
        "Renovationes quaerere? (optiones: update_checks)";
    HelpHint:
        "? help",
        "? aide",
        "? ayuda",
        "? ajutor",
        "? ajuda",
        "? ajuda",
        "? auxilium";
    Problems:
        "Problems\n",
        "Problèmes\n",
        "Problemas\n",
        "Probleme\n",
        "Problemas\n",
        "Problemas\n",
        "Difficultates\n";
    FolderList:
        "{name} folders",
        "Dossiers de {name}",
        "Carpetas de {name}",
        "Dosarele {name}",
        "Pastas de {name}",
        "Pastas de {name}",
        "Capsae {name}";
    SubfolderList:
        "{name} subfolders",
        "Sous-dossiers de {name}",
        "Subcarpetas de {name}",
        "Subdosarele {name}",
        "Subpastas de {name}",
        "Subpastas de {name}",
        "Subcapsae {name}";
    ShowHidden:
        "Show hidden ({n})",
        "Afficher les masqués ({n})",
        "Mostrar ocultas ({n})",
        "Afișați ascunsele ({n})",
        "Mostrar ocultas ({n})",
        "Mostrar ocultas ({n})",
        "Occultas monstra ({n})";
    ChangesWait:
        "{n} change(s) wait for the server",
        "{n} modification(s) en attente du serveur",
        "{n} cambio(s) esperan al servidor",
        "{n} modificare(i) așteaptă serverul",
        "{n} alteração(ões) aguardam o servidor",
        "{n} alteração(ões) aguardam o servidor",
        "{n} mutatio(nes) servitorem exspectant";
    CollapseWhat:
        "Collapse {what}",
        "Réduire {what}",
        "Contraer {what}",
        "Restrângeți {what}",
        "Recolher {what}",
        "Recolher {what}",
        "Contrahe {what}";
    ExpandWhat:
        "Expand {what}",
        "Développer {what}",
        "Expandir {what}",
        "Extindeți {what}",
        "Expandir {what}",
        "Expandir {what}",
        "Expande {what}";
    FetchingOlder:
        "Fetching older mail: {cached} of {wanted} cached",
        "Récupération du courrier ancien : {cached} sur {wanted} en cache",
        "Descargando correo anterior: {cached} de {wanted} en caché",
        "Se descarcă e-mailurile mai vechi: {cached} din {wanted} în cache",
        "Baixando e-mails antigos: {cached} de {wanted} em cache",
        "A transferir correio antigo: {cached} de {wanted} em cache",
        "Epistulae veteres deprimuntur: {cached} ex {wanted} in memoria";
    SyncNowKeys:
        "Sync now ({keys})",
        "Synchroniser ({keys})",
        "Sincronizar ahora ({keys})",
        "Sincronizați acum ({keys})",
        "Sincronizar agora ({keys})",
        "Sincronizar agora ({keys})",
        "Nunc synchroniza ({keys})";
    DraftSaveFailed:
        "Draft save failed: {error}",
        "Échec de l’enregistrement du brouillon : {error}",
        "Error al guardar el borrador: {error}",
        "Salvarea ciornei a eșuat: {error}",
        "Falha ao salvar o rascunho: {error}",
        "Falha ao guardar o rascunho: {error}",
        "Adumbratio servari non potuit: {error}";
    NotSent:
        "Not sent: {error}",
        "Non envoyé : {error}",
        "No enviado: {error}",
        "Netrimis: {error}",
        "Não enviada: {error}",
        "Não enviada: {error}",
        "Non missa: {error}";
    RemoveName:
        "Remove {name}",
        "Retirer {name}",
        "Quitar {name}",
        "Eliminați {name}",
        "Remover {name}",
        "Remover {name}",
        "Remove {name}";
    SearchTitle:
        "Search: {query}",
        "Recherche : {query}",
        "Búsqueda: {query}",
        "Căutare: {query}",
        "Pesquisa: {query}",
        "Pesquisa: {query}",
        "Quaestio: {query}";
    UnreadCount:
        "{n} unread",
        "{n} non lus",
        "{n} no leídos",
        "{n} necitite",
        "{n} não lidas",
        "{n} não lidas",
        "{n} non lecta";
    CheckedCount:
        "{n} checked",
        "{n} cochés",
        "{n} marcados",
        "{n} bifate",
        "{n} marcadas",
        "{n} marcadas",
        "{n} signata";
    LabelConversation:
        "{label} conversation",
        "{label} la discussion",
        "{label} la conversación",
        "{label} conversația",
        "{label} a conversa",
        "{label} a conversa",
        "{label} colloquium";
    HideWhat:
        "Hide {what}",
        "Masquer {what}",
        "Ocultar {what}",
        "Ascundeți {what}",
        "Ocultar {what}",
        "Ocultar {what}",
        "Cela {what}";
    ShowWhat:
        "Show {what}",
        "Afficher {what}",
        "Mostrar {what}",
        "Afișați {what}",
        "Mostrar {what}",
        "Mostrar {what}",
        "Monstra {what}";
    NOfM:
        "{n} of {m}",
        "{n} sur {m}",
        "{n} de {m}",
        "{n} din {m}",
        "{n} de {m}",
        "{n} de {m}",
        "{n} ex {m}";
    MessagesCount:
        "{n} MESSAGES",
        "{n} MESSAGES",
        "{n} MENSAJES",
        "{n} MESAJE",
        "{n} MENSAGENS",
        "{n} MENSAGENS",
        "{n} EPISTULAE";
    ExpandMessage:
        "Expand message from {from}",
        "Développer le message de {from}",
        "Expandir el mensaje de {from}",
        "Extindeți mesajul de la {from}",
        "Expandir a mensagem de {from}",
        "Expandir a mensagem de {from}",
        "Epistulam a {from} expande";
    CollapseMessage:
        "Collapse message from {from}",
        "Réduire le message de {from}",
        "Contraer el mensaje de {from}",
        "Restrângeți mesajul de la {from}",
        "Recolher a mensagem de {from}",
        "Recolher a mensagem de {from}",
        "Epistulam a {from} contrahe";
    ToList:
        "to {to}",
        "pour {to}",
        "para {to}",
        "către {to}",
        "para {to}",
        "para {to}",
        "ad {to}";
    BodyFailed:
        "The body could not be downloaded: {error}",
        "Le message n’a pas pu être téléchargé : {error}",
        "No se pudo descargar el mensaje: {error}",
        "Mesajul nu a putut fi descărcat: {error}",
        "Não foi possível baixar a mensagem: {error}",
        "Não foi possível transferir a mensagem: {error}",
        "Epistula deprimi non potuit: {error}";
    ActionFrom:
        "{action} from {sender}",
        "{action} de {sender}",
        "{action} de {sender}",
        "{action} de la {sender}",
        "{action} de {sender}",
        "{action} de {sender}",
        "{action} a {sender}";
    LinksCount:
        "Links ({n})",
        "Liens ({n})",
        "Enlaces ({n})",
        "Linkuri ({n})",
        "Links ({n})",
        "Ligações ({n})",
        "Nexus ({n})";
    OpenInBrowser:
        "Open in your browser:\n{url}",
        "Ouvrir dans votre navigateur :\n{url}",
        "Abrir en tu navegador:\n{url}",
        "Deschideți în browser:\n{url}",
        "Abrir no navegador:\n{url}",
        "Abrir no navegador:\n{url}",
        "In navigatro aperi:\n{url}";
    FailedError:
        "failed: {error}",
        "échec : {error}",
        "error: {error}",
        "eșuat: {error}",
        "falhou: {error}",
        "falhou: {error}",
        "defecit: {error}";
    RemoveAccountCache:
        "Remove {email} and its mail cache",
        "Supprimer {email} et son cache de courrier",
        "Quitar {email} y su caché de correo",
        "Eliminați {email} și memoria cache de e-mail",
        "Remover {email} e seu cache de e-mail",
        "Remover {email} e a sua cache de correio",
        "Remove {email} et memoriam eius epistularum";
    ChangeKeysOf:
        "Change the keys of {name}",
        "Changer les touches de {name}",
        "Cambiar las teclas de {name}",
        "Schimbați tastele pentru {name}",
        "Alterar as teclas de {name}",
        "Alterar as teclas de {name}",
        "Claves {name} muta";
    TestConnection:
        "{email}  Connection",
        "{email}  Connexion",
        "{email}  Conexión",
        "{email}  Conexiune",
        "{email}  Conexão",
        "{email}  Ligação",
        "{email}  Conexio";
    FoundAt:
        "Found at {source}",
        "Trouvé sur {source}",
        "Encontrado en {source}",
        "Găsit la {source}",
        "Encontrado em {source}",
        "Encontrado em {source}",
        "Inventum apud {source}";
    NotFound:
        "Not found ({error}); enter them yourself.",
        "Introuvable ({error}) ; saisissez-les vous-même.",
        "No encontrado ({error}); escríbelos tú.",
        "Negăsit ({error}); introduceți-le manual.",
        "Não encontrado ({error}); digite-os você mesmo.",
        "Não encontrado ({error}); introduza-os manualmente.",
        "Non inventum ({error}); ipse ea insere.";
    SigningIn:
        "Signing in to {email}…",
        "Connexion à {email}…",
        "Iniciando sesión en {email}…",
        "Se face autentificarea pentru {email}…",
        "Entrando em {email}…",
        "A iniciar sessão em {email}…",
        "Ingressus in {email}…";
    LoginFailedError:
        "The login failed: {error}",
        "La connexion a échoué : {error}",
        "Error al iniciar sesión: {error}",
        "Autentificarea a eșuat: {error}",
        "O login falhou: {error}",
        "O início de sessão falhou: {error}",
        "Ingressus defecit: {error}";
    NMessages:
        "{n} messages",
        "{n} messages",
        "{n} mensajes",
        "{n} mesaje",
        "{n} mensagens",
        "{n} mensagens",
        "{n} epistulae";
    FromName:
        "from {from}",
        "de {from}",
        "de {from}",
        "de la {from}",
        "de {from}",
        "de {from}",
        "a {from}";
    NFiles:
        "{n} files",
        "{n} fichiers",
        "{n} archivos",
        "{n} fișiere",
        "{n} arquivos",
        "{n} ficheiros",
        "{n} documenta";
    AttachmentsSummary:
        "Attachments ({files} · {size})",
        "Pièces jointes ({files} · {size})",
        "Adjuntos ({files} · {size})",
        "Atașamente ({files} · {size})",
        "Anexos ({files} · {size})",
        "Anexos ({files} · {size})",
        "Adiuncta ({files} · {size})";
    SavingInto:
        "Saving into {dir}…",
        "Enregistrement dans {dir}…",
        "Guardando en {dir}…",
        "Se salvează în {dir}…",
        "Salvando em {dir}…",
        "A guardar em {dir}…",
        "In {dir} servatur…";
    SaveFailed:
        "Could not save the attachments: {error}",
        "Impossible d’enregistrer les pièces jointes : {error}",
        "No se pudieron guardar los adjuntos: {error}",
        "Atașamentele nu au putut fi salvate: {error}",
        "Não foi possível salvar os anexos: {error}",
        "Não foi possível guardar os anexos: {error}",
        "Adiuncta servari non potuerunt: {error}";
    NoPreview:
        "No preview: {error}",
        "Pas d’aperçu : {error}",
        "Sin vista previa: {error}",
        "Fără previzualizare: {error}",
        "Sem prévia: {error}",
        "Sem pré-visualização: {error}",
        "Nulla praevisio: {error}";
    SavedAs:
        "Saved as {name}",
        "Enregistré sous {name}",
        "Guardado como {name}",
        "Salvat ca {name}",
        "Salvo como {name}",
        "Guardado como {name}",
        "Servatum ut {name}";
    SavedFiles:
        "Saved {n} files: {names}",
        "{n} fichiers enregistrés : {names}",
        "{n} archivos guardados: {names}",
        "{n} fișiere salvate: {names}",
        "{n} arquivos salvos: {names}",
        "{n} ficheiros guardados: {names}",
        "{n} documenta servata: {names}";
    AgentRequest:
        "Agent request: send {what} — check it before approving",
        "Demande d’agent : envoyer {what} — vérifiez avant d’approuver",
        "Solicitud de un agente: enviar {what}; revísalo antes de aprobarlo",
        "Cerere de la un agent: trimiterea {what}; verificați înainte de a aproba",
        "Solicitação de agente: enviar {what}; confira antes de aprovar",
        "Pedido de agente: enviar {what}; verifique antes de aprovar",
        "Petitio agentis: mittere {what}; inspice antequam probes";
    AgentWantsSend:
        "! An agent wants to send {what}",
        "! Un agent veut envoyer {what}",
        "! Un agente quiere enviar {what}",
        "! Un agent vrea să trimită {what}",
        "! Um agente quer enviar {what}",
        "! Um agente quer enviar {what}",
        "! Agens mittere vult {what}";
    ApprovalsHintTui:
        " — Ctrl+K, “Show send requests”",
        " — Ctrl+K, « Show send requests »",
        " — Ctrl+K, «Show send requests»",
        " — Ctrl+K, „Show send requests”",
        " — Ctrl+K, “Show send requests”",
        " — Ctrl+K, “Show send requests”",
        " — Ctrl+K, «Show send requests»";
    SendNowQuestion:
        "Send “{subject}” now? y sends (with the undo window), any other key doesn't",
        "Envoyer « {subject} » maintenant ? y envoie (avec le délai d’annulation), toute autre touche non",
        "¿Enviar «{subject}» ahora? y envía (con el plazo para deshacer), cualquier otra tecla no",
        "Trimiteți „{subject}” acum? y trimite (cu intervalul de anulare), orice altă tastă nu",
        "Enviar “{subject}” agora? y envia (com o prazo para desfazer), qualquer outra tecla não",
        "Enviar “{subject}” agora? y envia (com o prazo de anulação), qualquer outra tecla não",
        "Nunc mittere «{subject}»? y mittit (cum spatio revocandi), quaevis alia clavis non";
    FromLine:
        "  From: {from}",
        "  De : {from}",
        "  De: {from}",
        "  De la: {from}",
        "  De: {from}",
        "  De: {from}",
        "  A: {from}";
    SubjectLine:
        "Subject: {subject}",
        "Objet : {subject}",
        "Asunto: {subject}",
        "Subiect: {subject}",
        "Assunto: {subject}",
        "Assunto: {subject}",
        "Res: {subject}";
    FromHeader:
        "From: {from}",
        "De : {from}",
        "De: {from}",
        "De la: {from}",
        "De: {from}",
        "De: {from}",
        "A: {from}";
    DateLine:
        "In: {mailbox}",
        "Dans : {mailbox}",
        "En: {mailbox}",
        "În: {mailbox}",
        "Em: {mailbox}",
        "Em: {mailbox}",
        "In: {mailbox}";
    AttachmentsCount:
        "Attachments ({n})",
        "Pièces jointes ({n})",
        "Adjuntos ({n})",
        "Atașamente ({n})",
        "Anexos ({n})",
        "Anexos ({n})",
        "Adiuncta ({n})";
    ChangePasswordOf:
        "Change the password · {email}",
        "Changer le mot de passe · {email}",
        "Cambiar la contraseña · {email}",
        "Schimbați parola · {email}",
        "Alterar a senha · {email}",
        "Alterar a palavra-passe · {email}",
        "Tesseram muta · {email}";
    AddAccountStep:
        "Add account · {step}",
        "Ajouter un compte · {step}",
        "Añadir cuenta · {step}",
        "Adăugați un cont · {step}",
        "Adicionar conta · {step}",
        "Adicionar conta · {step}",
        "Rationem adde · {step}";
    FoundFill:
        "Found at {source}: {imap} and {smtp}. Ctrl+F fills them in.",
        "Trouvé sur {source} : {imap} et {smtp}. Ctrl+F les remplit.",
        "Encontrado en {source}: {imap} y {smtp}. Ctrl+F los rellena.",
        "Găsit la {source}: {imap} și {smtp}. Ctrl+F le completează.",
        "Encontrado em {source}: {imap} e {smtp}. Ctrl+F os preenche.",
        "Encontrado em {source}: {imap} e {smtp}. Ctrl+F preenche-os.",
        "Inventum apud {source}: {imap} et {smtp}. Ctrl+F ea implet.";
    EditorFailed:
        "The editor failed: {reason}",
        "L’éditeur a échoué : {reason}",
        "El editor falló: {reason}",
        "Editorul a eșuat: {reason}",
        "O editor falhou: {reason}",
        "O editor falhou: {reason}",
        "Editor defecit: {reason}";
    TitleFocus:
        "{title} · Focus",
        "{title} · Focus",
        "{title} · Enfoque",
        "{title} · Concentrare",
        "{title} · Foco",
        "{title} · Foco",
        "{title} · Intentio";
    TitleOnly:
        "{title} · only {only}",
        "{title} · seulement {only}",
        "{title} · solo {only}",
        "{title} · doar {only}",
        "{title} · só {only}",
        "{title} · só {only}",
        "{title} · tantum {only}";
    SendingCancel:
        "{sending} · z cancels",
        "{sending} · z annule",
        "{sending} · z cancela",
        "{sending} · z anulează",
        "{sending} · z cancela",
        "{sending} · z cancela",
        "{sending} · z rescindit";
    MoreDetails:
        " (+{n} more, ? for details)",
        " (+{n} autres, ? pour les détails)",
        " (+{n} más, ? para detalles)",
        " (+{n} în plus, ? pentru detalii)",
        " (+{n} outros, ? para detalhes)",
        " (+{n} outros, ? para detalhes)",
        " (+{n} alia, ? ad singula)";
    RetryPalette:
        "{text} (retry: palette)",
        "{text} (réessayer : palette)",
        "{text} (reintentar: paleta)",
        "{text} (reîncercați: paletă)",
        "{text} (tentar de novo: paleta)",
        "{text} (tentar de novo: paleta)",
        "{text} (iterum conare: tabula)";
    UndoLabel:
        "“{label}” z undo",
        "« {label} » z annuler",
        "«{label}» z deshacer",
        "„{label}” z anulați",
        "“{label}” z desfazer",
        "“{label}” z anular",
        "«{label}» z revoca";
    BadgeSettings:
        "{badge} (settings)",
        "{badge} (paramètres)",
        "{badge} (ajustes)",
        "{badge} (setări)",
        "{badge} (configurações)",
        "{badge} (definições)",
        "{badge} (optiones)";
    PresetGmailHint:
        "Needs an App Password (2-Step Verification on).",
        "Nécessite un mot de passe d’application (validation en deux étapes activée).",
        "Necesita una contraseña de aplicación (verificación en dos pasos activada).",
        "Necesită o parolă pentru aplicații (cu verificarea în doi pași activată).",
        "Precisa de uma senha de app (verificação em duas etapas ativada).",
        "Precisa de uma palavra-passe de aplicação (validação em dois passos ativada).",
        "Tesseram applicationis requirit (verificatione duplici activa).";
    PresetHostingerHint:
        "Your mailbox password.",
        "Le mot de passe de votre compte.",
        "La contraseña de tu buzón.",
        "Parola căsuței dvs. poștale.",
        "A senha da sua caixa de correio.",
        "A palavra-passe da sua caixa de correio.",
        "Tessera capsae tuae epistularum.";
    PresetCustomHint:
        "Enter the servers yourself.",
        "Saisissez les serveurs vous-même.",
        "Escribe los servidores tú.",
        "Introduceți serverele manual.",
        "Digite os servidores você mesmo.",
        "Introduza os servidores manualmente.",
        "Servitores ipse insere.";
    PresetGmail:
        "Gmail",
        "Gmail",
        "Gmail",
        "Gmail",
        "Gmail",
        "Gmail",
        "Gmail";
    PresetHostinger:
        "Hostinger Mail",
        "Hostinger Mail",
        "Hostinger Mail",
        "Hostinger Mail",
        "Hostinger Mail",
        "Hostinger Mail",
        "Hostinger Mail";
    PresetOther:
        "Other IMAP / SMTP",
        "Autre IMAP / SMTP",
        "Otro IMAP / SMTP",
        "Alt IMAP / SMTP",
        "Outro IMAP / SMTP",
        "Outro IMAP / SMTP",
        "Alius IMAP / SMTP";
    HintChoose:
        "choose",
        "choisir",
        "elegir",
        "alegeți",
        "escolher",
        "escolher",
        "elige";
    HintBack:
        "back",
        "retour",
        "atrás",
        "înapoi",
        "voltar",
        "retroceder",
        "retro";
    HintOpen:
        "open",
        "ouvrir",
        "abrir",
        "deschideți",
        "abrir",
        "abrir",
        "aperi";
    HintEverything:
        "everything",
        "tout",
        "todo",
        "tot",
        "tudo",
        "tudo",
        "omnia";
    HintNext:
        "next",
        "suivant",
        "siguiente",
        "următorul",
        "próximo",
        "seguinte",
        "sequens";
    HintPrevious:
        "previous",
        "précédent",
        "anterior",
        "anteriorul",
        "anterior",
        "anterior",
        "prius";
    HintSelect:
        "select",
        "sélectionner",
        "seleccionar",
        "selectați",
        "selecionar",
        "selecionar",
        "selige";
    ToLabel:
        "To",
        "Pour",
        "Para",
        "Către",
        "Para",
        "Para",
        "Ad";
    ReleaseNotes:
        "  release notes",
        "  notes de version",
        "  notas de la versión",
        "  note de lansare",
        "  notas da versão",
        "  notas da versão",
        "  notae versionis";
    DateLabel:
        "Date: {date} ({age})",
        "Date : {date} ({age})",
        "Fecha: {date} ({age})",
        "Data: {date} ({age})",
        "Data: {date} ({age})",
        "Data: {date} ({age})",
        "Dies: {date} ({age})";
    UnknownSender:
        "(unknown sender)",
        "(expéditeur inconnu)",
        "(remitente desconocido)",
        "(expeditor necunoscut)",
        "(remetente desconhecido)",
        "(remetente desconhecido)",
        "(missor ignotus)";
    SendingFailed:
        "Sending failed",
        "Échec de l’envoi",
        "Error al enviar",
        "Trimiterea a eșuat",
        "Falha no envio",
        "Falha no envio",
        "Missio defecit";
    AnAgentWantsToSendAMessage:
        "An agent wants to send a message",
        "Un agent veut envoyer un message",
        "Un agente quiere enviar un mensaje",
        "Un agent vrea să trimită un mesaj",
        "Um agente quer enviar uma mensagem",
        "Um agente quer enviar uma mensagem",
        "Agens epistulam mittere vult";
    To:
        "To:",
        "Pour :",
        "Para:",
        "Către:",
        "Para:",
        "Para:",
        "Ad:";
    Cc:
        "Cc:",
        "Copie à :",
        "CC:",
        "Cc:",
        "Cc:",
        "Cc:",
        "Cc:";
    Bcc:
        "Bcc:",
        "Copie cachée à :",
        "CCO:",
        "Bcc:",
        "Cco:",
        "Bcc:",
        "Bcc:";
    BccX:
        "Bcc",
        "Copie cachée à",
        "CCO",
        "Bcc",
        "Cco",
        "Bcc",
        "Bcc";
    SendingX:
        "sending…",
        "envoi…",
        "enviando…",
        "se trimite…",
        "enviando…",
        "a enviar…",
        "mittitur…";
    NotSavedToTheServer:
        "not saved to the server",
        "non enregistré sur le serveur",
        "no guardado en el servidor",
        "nesalvat pe server",
        "não salvo no servidor",
        "não guardado no servidor",
        "in servitore non servatum";
    Saving:
        "saving…",
        "enregistrement…",
        "guardando…",
        "se salvează…",
        "salvando…",
        "a guardar…",
        "servatur…";
    Saved:
        "saved",
        "enregistré",
        "guardado",
        "salvat",
        "salvo",
        "guardado",
        "servatum";
    DraftX:
        "draft",
        "brouillon",
        "borrador",
        "ciornă",
        "rascunho",
        "rascunho",
        "adumbratio";
    NewMessage:
        "New message",
        "Nouveau message",
        "Mensaje nuevo",
        "Mesaj nou",
        "Nova mensagem",
        "Nova mensagem",
        "Nova epistula";
    January:
        "January",
        "janvier",
        "enero",
        "ianuarie",
        "janeiro",
        "janeiro",
        "Ianuarius";
    February:
        "February",
        "février",
        "febrero",
        "februarie",
        "fevereiro",
        "fevereiro",
        "Februarius";
    March:
        "March",
        "mars",
        "marzo",
        "martie",
        "março",
        "março",
        "Martius";
    April:
        "April",
        "avril",
        "abril",
        "aprilie",
        "abril",
        "abril",
        "Aprilis";
    May:
        "May",
        "mai",
        "mayo",
        "mai",
        "maio",
        "maio",
        "Maius";
    June:
        "June",
        "juin",
        "junio",
        "iunie",
        "junho",
        "junho",
        "Iunius";
    July:
        "July",
        "juillet",
        "julio",
        "iulie",
        "julho",
        "julho",
        "Iulius";
    August:
        "August",
        "août",
        "agosto",
        "august",
        "agosto",
        "agosto",
        "Augustus";
    September:
        "September",
        "septembre",
        "septiembre",
        "septembrie",
        "setembro",
        "setembro",
        "September";
    October:
        "October",
        "octobre",
        "octubre",
        "octombrie",
        "outubro",
        "outubro",
        "October";
    November:
        "November",
        "novembre",
        "noviembre",
        "noiembrie",
        "novembro",
        "novembro",
        "November";
    December:
        "December",
        "décembre",
        "diciembre",
        "decembrie",
        "dezembro",
        "dezembro",
        "December";
    Jan:
        "Jan",
        "janv.",
        "ene.",
        "ian.",
        "jan.",
        "jan.",
        "Ian.";
    Feb:
        "Feb",
        "févr.",
        "feb.",
        "feb.",
        "fev.",
        "fev.",
        "Feb.";
    Mar:
        "Mar",
        "mars",
        "mar.",
        "mar.",
        "mar.",
        "mar.",
        "Mart.";
    Apr:
        "Apr",
        "avr.",
        "abr.",
        "apr.",
        "abr.",
        "abr.",
        "Apr.";
    Jun:
        "Jun",
        "juin",
        "jun.",
        "iun.",
        "jun.",
        "jun.",
        "Iun.";
    Jul:
        "Jul",
        "juil.",
        "jul.",
        "iul.",
        "jul.",
        "jul.",
        "Iul.";
    Aug:
        "Aug",
        "août",
        "ago.",
        "aug.",
        "ago.",
        "ago.",
        "Aug.";
    Sep:
        "Sep",
        "sept.",
        "sept.",
        "sept.",
        "set.",
        "set.",
        "Sept.";
    Oct:
        "Oct",
        "oct.",
        "oct.",
        "oct.",
        "out.",
        "out.",
        "Oct.";
    Nov:
        "Nov",
        "nov.",
        "nov.",
        "nov.",
        "nov.",
        "nov.",
        "Nov.";
    Dec:
        "Dec",
        "déc.",
        "dic.",
        "dec.",
        "dez.",
        "dez.",
        "Dec.";
    Later:
        "Later",
        "Plus tard",
        "Más tarde",
        "Mai târziu",
        "Mais tarde",
        "Mais tarde",
        "Postea";
    Today:
        "Today",
        "Aujourd’hui",
        "Hoy",
        "Azi",
        "Hoje",
        "Hoje",
        "Hodie";
    Yesterday:
        "Yesterday",
        "Hier",
        "Ayer",
        "Ieri",
        "Ontem",
        "Ontem",
        "Heri";
    ThisWeek:
        "This week",
        "Cette semaine",
        "Esta semana",
        "Săptămâna aceasta",
        "Esta semana",
        "Esta semana",
        "Hac hebdomade";
    EarlierThisMonth:
        "Earlier this month",
        "Plus tôt ce mois-ci",
        "Antes este mes",
        "Mai devreme luna aceasta",
        "Antes neste mês",
        "No início deste mês",
        "Prius hoc mense";
    Now:
        "now",
        "maintenant",
        "ahora",
        "acum",
        "agora",
        "agora",
        "nunc";
    JustNow:
        "just now",
        "à l’instant",
        "hace un momento",
        "chiar acum",
        "agora mesmo",
        "há pouco",
        "modo";
    Inbox:
        "Inbox",
        "Courrier entrant",
        "Bandeja de entrada",
        "Căsuță de intrare",
        "Caixa de entrada",
        "Caixa de entrada",
        "Accepta";
    Sent:
        "Sent",
        "Envoyés",
        "Enviados",
        "Trimise",
        "Enviados",
        "Enviados",
        "Missa";
    Drafts:
        "Drafts",
        "Brouillons",
        "Borradores",
        "Ciorne",
        "Rascunhos",
        "Rascunhos",
        "Adumbrationes";
    Junk:
        "Junk",
        "Indésirables",
        "Spam",
        "Spam",
        "Spam",
        "Spam",
        "Purgamentum";
    AllMail:
        "All Mail",
        "Tous les messages",
        "Todo el correo",
        "Toate e-mailurile",
        "Todos os e-mails",
        "Todo o correio",
        "Omnis cursus";
    WhichConversation:
        "Which conversation?",
        "Quelle discussion ?",
        "¿Qué conversación?",
        "Ce conversație?",
        "Qual conversa?",
        "Qual conversa?",
        "Quod colloquium?";
    WhichDraft:
        "Which draft?",
        "Quel brouillon ?",
        "¿Qué borrador?",
        "Ce ciornă?",
        "Qual rascunho?",
        "Qual rascunho?",
        "Quae adumbratio?";
    WhichAccount:
        "Which account?",
        "Quel compte ?",
        "¿Qué cuenta?",
        "Ce cont?",
        "Qual conta?",
        "Qual conta?",
        "Quae ratio?";
    RemoveThisAccountThisCannotBeUndone:
        "Remove this account? This cannot be undone.",
        "Supprimer ce compte ? C’est irréversible.",
        "¿Quitar esta cuenta? No se puede deshacer.",
        "Eliminați acest cont? Acțiunea nu poate fi anulată.",
        "Remover esta conta? Isso não pode ser desfeito.",
        "Remover esta conta? Esta ação não pode ser anulada.",
        "Hanc rationem removere? Hoc revocari non potest.";
    InstallTheUpdateDailAsksToRestart:
        "Install the update? dail asks to restart afterwards.",
        "Installer la mise à jour ? dail demandera de redémarrer ensuite.",
        "¿Instalar la actualización? dail pedirá reiniciarse después.",
        "Instalați actualizarea? dail va cere apoi repornirea.",
        "Instalar a atualização? O dail pedirá para reiniciar depois.",
        "Instalar a atualização? O dail pedirá para reiniciar depois.",
        "Renovationem instituere? dail postea iterum incipere rogabit.";
    ItsName:
        "its name",
        "son nom",
        "su nombre",
        "numele său",
        "o nome",
        "o nome",
        "nomen eius";
    NewNameFullPath:
        "New name (full path)?",
        "Nouveau nom (chemin complet) ?",
        "¿Nombre nuevo (ruta completa)?",
        "Nume nou (cale completă)?",
        "Novo nome (caminho completo)?",
        "Novo nome (caminho completo)?",
        "Novum nomen (via tota)?";
    DeleteThisFolderAndAllMailIn:
        "Delete this folder and all mail in it? This cannot be undone.",
        "Supprimer ce dossier et tout son courrier ? C’est irréversible.",
        "¿Eliminar esta carpeta y todo su correo? No se puede deshacer.",
        "Ștergeți acest dosar și toate mesajele din el? Acțiunea nu poate fi anulată.",
        "Excluir esta pasta e todos os e-mails nela? Isso não pode ser desfeito.",
        "Eliminar esta pasta e todo o correio nela? Esta ação não pode ser anulada.",
        "Hanc capsam et omnes eius epistulas delere? Hoc revocari non potest.";
    FindTheMailServersOfWhichAddress:
        "Find the mail servers of which address?",
        "Trouver les serveurs de quelle adresse ?",
        "¿Buscar los servidores de qué dirección?",
        "Găsiți serverele cărei adrese?",
        "Encontrar os servidores de qual endereço?",
        "Encontrar os servidores de qual endereço?",
        "Servitores cuius inscriptionis invenire?";
    YouExampleOrgAsksTheProviderThen:
        "you@example.org (asks the provider, then Mozilla)",
        "vous@example.org (interroge le fournisseur, puis Mozilla)",
        "tu@example.org (pregunta al proveedor y luego a Mozilla)",
        "dvs@example.org (întreabă furnizorul, apoi Mozilla)",
        "voce@example.org (consulta o provedor e depois a Mozilla)",
        "voce@example.org (consulta o fornecedor e depois a Mozilla)",
        "tu@example.org (provisorem, deinde Mozillam interrogat)";
    WhichTheme:
        "Which theme?",
        "Quel thème ?",
        "¿Qué tema?",
        "Ce temă?",
        "Qual tema?",
        "Qual tema?",
        "Quod thema?";
    SystemLightOrDarkThenATheme:
        "system, light or dark, then a theme for that mode: dark dail-dark-hc",
        "system, light ou dark, puis un thème pour ce mode : dark dail-dark-hc",
        "system, light o dark, y luego un tema para ese modo: dark dail-dark-hc",
        "system, light sau dark, apoi o temă pentru acel mod: dark dail-dark-hc",
        "system, light ou dark e depois um tema para esse modo: dark dail-dark-hc",
        "system, light ou dark e depois um tema para esse modo: dark dail-dark-hc",
        "system, light aut dark, deinde thema pro illo modo: dark dail-dark-hc";
    WhichCommand:
        "Which command?",
        "Quelle commande ?",
        "¿Qué comando?",
        "Ce comandă?",
        "Qual comando?",
        "Qual comando?",
        "Quod mandatum?";
    ItsNameEGArchiveThreads:
        "its name, e.g. archive_threads",
        "son nom, p. ex. archive_threads",
        "su nombre, p. ej. archive_threads",
        "numele ei, de ex. archive_threads",
        "o nome, p. ex. archive_threads",
        "o nome, p. ex. archive_threads",
        "nomen eius, e.g. archive_threads";
    WhichKeysCommaSeparatedEmptyThePreset:
        "Which keys? (comma-separated; empty: the preset's)",
        "Quelles touches ? (séparées par des virgules ; vide : celles du préréglage)",
        "¿Qué teclas? (separadas por comas; vacío: las del preajuste)",
        "Ce taste? (separate prin virgulă; gol: cele ale presetării)",
        "Quais teclas? (separadas por vírgula; vazio: as da predefinição)",
        "Quais teclas? (separadas por vírgula; vazio: as da predefinição)",
        "Quae claves? (commatibus distinctae; vacuum: praeformati)";
    WhichSetting:
        "Which setting?",
        "Quel paramètre ?",
        "¿Qué ajuste?",
        "Ce setare?",
        "Qual configuração?",
        "Qual definição?",
        "Quae optio?";
    NewDisplayName:
        "New display name",
        "Nouveau nom affiché",
        "Nombre visible nuevo",
        "Nume afișat nou",
        "Novo nome de exibição",
        "Novo nome a mostrar",
        "Novum nomen monstrandum";
    WhichMailbox:
        "Which mailbox?",
        "Quel dossier ?",
        "¿Qué carpeta?",
        "Ce dosar?",
        "Qual pasta?",
        "Qual pasta?",
        "Quae capsa?";
    KeepHowManyOfTheNewestMessages:
        "Keep how many of the newest messages offline?",
        "Garder combien de messages récents hors ligne ?",
        "¿Cuántos de los mensajes más recientes conservar sin conexión?",
        "Câte dintre cele mai noi mesaje păstrați offline?",
        "Manter offline quantas das mensagens mais recentes?",
        "Manter offline quantas das mensagens mais recentes?",
        "Quot recentissimas epistulas sine conexione servare?";
    ANumber0KeepsAllOfIt:
        "a number; 0 keeps all of it",
        "un nombre ; 0 garde tout",
        "un número; 0 lo conserva todo",
        "un număr; 0 le păstrează pe toate",
        "um número; 0 mantém tudo",
        "um número; 0 mantém tudo",
        "numerus; 0 omnia servat";
    GoToMailbox:
        "Go to mailbox",
        "Aller au dossier",
        "Ir a la carpeta",
        "Mergeți la dosar",
        "Ir para a pasta",
        "Ir para a pasta",
        "Ad capsam i";
    WhichAddress:
        "Which address?",
        "Quelle adresse ?",
        "¿Qué dirección?",
        "Ce adresă?",
        "Qual endereço?",
        "Qual endereço?",
        "Quae inscriptio?";
    ItsSignatureEmptyNone:
        "Its signature? (empty: none)",
        "Sa signature ? (vide : aucune)",
        "¿Su firma? (vacío: ninguna)",
        "Semnătura ei? (gol: niciuna)",
        "A assinatura? (vazio: nenhuma)",
        "A assinatura? (vazio: nenhuma)",
        "Subscriptio eius? (vacuum: nulla)";
    NameCompany:
        "Name · Company",
        "Nom · Société",
        "Nombre · Empresa",
        "Nume · Companie",
        "Nome · Empresa",
        "Nome · Empresa",
        "Nomen · Societas";
    RemoveWhichAddress:
        "Remove which address?",
        "Retirer quelle adresse ?",
        "¿Quitar qué dirección?",
        "Eliminați ce adresă?",
        "Remover qual endereço?",
        "Remover qual endereço?",
        "Quam inscriptionem removere?";
    TrustCertificateDots:
        "Trust certificate…",
        "Faire confiance au certificat…",
        "Confiar en el certificado…",
        "Acceptați certificatul…",
        "Confiar no certificado…",
        "Confiar no certificado…",
        "Testimonio confide…";
    ForgetCertificateDots:
        "Forget certificate…",
        "Oublier le certificat…",
        "Olvidar el certificado…",
        "Uitați certificatul…",
        "Esquecer o certificado…",
        "Esquecer o certificado…",
        "Testimonium obliviscere…";
    FixTrust:
        "trust…",
        "faire confiance…",
        "confiar…",
        "acceptați…",
        "confiar…",
        "confiar…",
        "confide…";
    FixPaletteTrust:
        " (palette: Trust a server's certificate)",
        " (palette : Trust a server's certificate)",
        " (paleta: Trust a server's certificate)",
        " (paletă: Trust a server's certificate)",
        " (paleta: Trust a server's certificate)",
        " (paleta: Trust a server's certificate)",
        " (tabula: Trust a server's certificate)";
    TrustWhichCertificate:
        "Trust which certificate?",
        "Faire confiance à quel certificat ?",
        "¿En qué certificado confiar?",
        "Ce certificat acceptați?",
        "Confiar em qual certificado?",
        "Confiar em qual certificado?",
        "Cui testimonio confidere?";
    ForgetWhichCertificate:
        "Forget which certificate?",
        "Oublier quel certificat ?",
        "¿Qué certificado olvidar?",
        "Ce certificat uitați?",
        "Esquecer qual certificado?",
        "Esquecer qual certificado?",
        "Quod testimonium oblivisci?";
    CertForAccount:
        "{subject} — for {email}",
        "{subject} — pour {email}",
        "{subject} — para {email}",
        "{subject} — pentru {email}",
        "{subject} — para {email}",
        "{subject} — para {email}",
        "{subject} — pro {email}";
    CertSelfSigned:
        "Self-signed",
        "Auto-signé",
        "Autofirmado",
        "Autosemnat",
        "Autoassinado",
        "Autoassinado",
        "A se ipso signatum";
    CertIssuer:
        "Issued by {issuer}",
        "Émis par {issuer}",
        "Emitido por {issuer}",
        "Emis de {issuer}",
        "Emitido por {issuer}",
        "Emitido por {issuer}",
        "Datum a {issuer}";
    CertValidity:
        "Valid from {from} to {until}",
        "Valide du {from} au {until}",
        "Válido del {from} al {until}",
        "Valabil de la {from} până la {until}",
        "Válido de {from} a {until}",
        "Válido de {from} a {until}",
        "Validum a {from} usque ad {until}";
    CertFingerprint:
        "SHA-256 {sha256}",
        "SHA-256 {sha256}",
        "SHA-256 {sha256}",
        "SHA-256 {sha256}",
        "SHA-256 {sha256}",
        "SHA-256 {sha256}",
        "SHA-256 {sha256}";
    SendFromWhichAddressEmptyTheAccount:
        "Send from which address? (empty: the account's own)",
        "Envoyer depuis quelle adresse ? (vide : celle du compte)",
        "¿Enviar desde qué dirección? (vacío: la de la cuenta)",
        "Trimiteți de la ce adresă? (gol: cea a contului)",
        "Enviar de qual endereço? (vazio: o da conta)",
        "Enviar de qual endereço? (vazio: o da conta)",
        "A qua inscriptione mittere? (vacuum: rationis ipsius)";
    SendFromWhichAddress:
        "Send from which address?",
        "Envoyer depuis quelle adresse ?",
        "¿Enviar desde qué dirección?",
        "Trimiteți de la ce adresă?",
        "Enviar de qual endereço?",
        "Enviar de qual endereço?",
        "A qua inscriptione mittere?";
    ListContactsMatching:
        "List contacts matching…",
        "Lister les contacts correspondant à…",
        "Listar los contactos que coinciden con…",
        "Listați contactele care se potrivesc cu…",
        "Listar os contatos que correspondem a…",
        "Listar os contactos que correspondem a…",
        "Necessarios enumera qui congruunt cum…";
    ANameOrAnAddress:
        "a name or an address",
        "un nom ou une adresse",
        "un nombre o una dirección",
        "un nume sau o adresă",
        "um nome ou um endereço",
        "um nome ou um endereço",
        "nomen aut inscriptio";
    SelectWhichConversation:
        "Select which conversation?",
        "Sélectionner quelle discussion ?",
        "¿Seleccionar qué conversación?",
        "Selectați ce conversație?",
        "Selecionar qual conversa?",
        "Selecionar qual conversa?",
        "Quod colloquium seligere?";
    OpenWhichConversation:
        "Open which conversation?",
        "Ouvrir quelle discussion ?",
        "¿Abrir qué conversación?",
        "Deschideți ce conversație?",
        "Abrir qual conversa?",
        "Abrir qual conversa?",
        "Quod colloquium aperire?";
    ShowImagesFromWhichSender:
        "Show images from which sender?",
        "Afficher les images de quel expéditeur ?",
        "¿Mostrar las imágenes de qué remitente?",
        "Afișați imaginile de la ce expeditor?",
        "Mostrar as imagens de qual remetente?",
        "Mostrar as imagens de qual remetente?",
        "Imagines cuius missoris monstrare?";
    HideImagesFromWhichSender:
        "Hide images from which sender?",
        "Masquer les images de quel expéditeur ?",
        "¿Ocultar las imágenes de qué remitente?",
        "Ascundeți imaginile de la ce expeditor?",
        "Ocultar as imagens de qual remetente?",
        "Ocultar as imagens de qual remetente?",
        "Imagines cuius missoris celare?";
    SaveTheAttachmentsIntoWhichFolder:
        "Save the attachments into which folder?",
        "Enregistrer les pièces jointes dans quel dossier ?",
        "¿Guardar los adjuntos en qué carpeta?",
        "Salvați atașamentele în ce dosar?",
        "Salvar os anexos em qual pasta?",
        "Guardar os anexos em que pasta?",
        "Adiuncta in quam capsam servare?";
    AnAbsolutePathEGHomeMe:
        "an absolute path, e.g. /home/me/Downloads",
        "un chemin absolu, p. ex. /home/moi/Téléchargements",
        "una ruta absoluta, p. ej. /home/yo/Descargas",
        "o cale absolută, de ex. /home/eu/Descărcări",
        "um caminho absoluto, p. ex. /home/eu/Downloads",
        "um caminho absoluto, p. ex. /home/eu/Transferências",
        "via absoluta, e.g. /home/ego/Depressa";
    MoveWhichConversation:
        "Move which conversation?",
        "Déplacer quelle discussion ?",
        "¿Mover qué conversación?",
        "Mutați ce conversație?",
        "Mover qual conversa?",
        "Mover qual conversa?",
        "Quod colloquium movere?";
    DiscardWhichChange:
        "Discard which change?",
        "Abandonner quelle modification ?",
        "¿Descartar qué cambio?",
        "Renunțați la ce modificare?",
        "Descartar qual alteração?",
        "Rejeitar que alteração?",
        "Quam mutationem abicere?";
    AttachWhichFile:
        "Attach which file?",
        "Joindre quel fichier ?",
        "¿Adjuntar qué archivo?",
        "Atașați ce fișier?",
        "Anexar qual arquivo?",
        "Anexar que ficheiro?",
        "Quod documentum adiungere?";
    AbsolutePathEGHomeMeReport:
        "absolute path, e.g. /home/me/report.pdf",
        "chemin absolu, p. ex. /home/moi/rapport.pdf",
        "ruta absoluta, p. ej. /home/yo/informe.pdf",
        "cale absolută, de ex. /home/eu/raport.pdf",
        "caminho absoluto, p. ex. /home/eu/relatorio.pdf",
        "caminho absoluto, p. ex. /home/eu/relatorio.pdf",
        "via absoluta, e.g. /home/ego/relatio.pdf";
    AttachToWhichDraft:
        "Attach to which draft?",
        "Joindre à quel brouillon ?",
        "¿Adjuntar a qué borrador?",
        "Atașați la ce ciornă?",
        "Anexar a qual rascunho?",
        "Anexar a qual rascunho?",
        "Cui adumbrationi adiungere?";
    RemoveWhichAttachment:
        "Remove which attachment?",
        "Supprimer quelle pièce jointe ?",
        "¿Quitar qué adjunto?",
        "Eliminați ce atașament?",
        "Remover qual anexo?",
        "Remover qual anexo?",
        "Quod adiunctum removere?";
    FromWhichDraft:
        "From which draft?",
        "De quel brouillon ?",
        "¿De qué borrador?",
        "Din ce ciornă?",
        "De qual rascunho?",
        "De qual rascunho?",
        "Ex qua adumbratione?";
    SearchMailResultsAsData:
        "Search mail (results as data)",
        "Rechercher dans le courrier (résultats en données)",
        "Buscar en el correo (resultados como datos)",
        "Căutați în e-mailuri (rezultate ca date)",
        "Pesquisar no e-mail (resultados como dados)",
        "Pesquisar no correio (resultados como dados)",
        "In cursu quaere (effectus ut data)";
    Search:
        "Search",
        "Rechercher",
        "Buscar",
        "Căutați",
        "Pesquisar",
        "Pesquisar",
        "Quaere";
    FocusWhichPane:
        "Focus which pane?",
        "Activer quel panneau ?",
        "¿Activar qué panel?",
        "Activați ce panou?",
        "Focar qual painel?",
        "Focar qual painel?",
        "Quod tabulatum intendere?";
    FocusOnWhichMailbox:
        "Focus on which mailbox?",
        "Focus sur quel dossier ?",
        "¿Centrarse en qué carpeta?",
        "Concentrare pe ce dosar?",
        "Focar em qual pasta?",
        "Focar em qual pasta?",
        "In qua capsa intendere?";
    NameOfTheNewFolder:
        "Name of the new folder?",
        "Nom du nouveau dossier ?",
        "¿Nombre de la carpeta nueva?",
        "Numele dosarului nou?",
        "Nome da nova pasta?",
        "Nome da nova pasta?",
        "Nomen novae capsae?";
    APathLikeWorkClients:
        "a path like Work/Clients",
        "un chemin comme Travail/Clients",
        "una ruta como Trabajo/Clientes",
        "o cale precum Serviciu/Clienți",
        "um caminho como Trabalho/Clientes",
        "um caminho como Trabalho/Clientes",
        "via ut Opus/Clientes";
    RenameWhichFolder:
        "Rename which folder?",
        "Renommer quel dossier ?",
        "¿Cambiar el nombre de qué carpeta?",
        "Redenumiți ce dosar?",
        "Renomear qual pasta?",
        "Mudar o nome de que pasta?",
        "Quam capsam renominare?";
    DeleteWhichFolder:
        "Delete which folder?",
        "Supprimer quel dossier ?",
        "¿Eliminar qué carpeta?",
        "Ștergeți ce dosar?",
        "Excluir qual pasta?",
        "Eliminar que pasta?",
        "Quam capsam delere?";
    PinWhichMailbox:
        "Pin which mailbox?",
        "Épingler quel dossier ?",
        "¿Fijar qué carpeta?",
        "Fixați ce dosar?",
        "Fixar qual pasta?",
        "Fixar qual pasta?",
        "Quam capsam figere?";
    UnpinWhichMailbox:
        "Unpin which mailbox?",
        "Détacher quel dossier ?",
        "¿Desfijar qué carpeta?",
        "Anulați fixarea cărui dosar?",
        "Desafixar qual pasta?",
        "Desafixar qual pasta?",
        "Quam capsam refigere?";
    HideWhichMailbox:
        "Hide which mailbox?",
        "Masquer quel dossier ?",
        "¿Ocultar qué carpeta?",
        "Ascundeți ce dosar?",
        "Ocultar qual pasta?",
        "Ocultar qual pasta?",
        "Quam capsam celare?";
    ShowWhichHiddenMailbox:
        "Show which hidden mailbox?",
        "Afficher quel dossier masqué ?",
        "¿Mostrar qué carpeta oculta?",
        "Afișați ce dosar ascuns?",
        "Mostrar qual pasta oculta?",
        "Mostrar qual pasta oculta?",
        "Quam capsam occultam monstrare?";
    MarkWhichMailboxRead:
        "Mark which mailbox read?",
        "Marquer quel dossier comme lu ?",
        "¿Marcar qué carpeta como leída?",
        "Marcați ce dosar ca citit?",
        "Marcar qual pasta como lida?",
        "Marcar qual pasta como lida?",
        "Quam capsam ut lectam notare?";
    ShowOnlyWhichThreads:
        "Show only which threads?",
        "Afficher seulement quelles discussions ?",
        "¿Mostrar solo qué conversaciones?",
        "Afișați doar ce conversații?",
        "Mostrar só quais conversas?",
        "Mostrar só quais conversas?",
        "Quae colloquia sola monstrare?";
    All:
        "all",
        "toutes",
        "todas",
        "toate",
        "todas",
        "todas",
        "omnia";
    CheckForUpdates:
        "Check for updates?",
        "Rechercher des mises à jour ?",
        "¿Buscar actualizaciones?",
        "Căutați actualizări?",
        "Procurar atualizações?",
        "Procurar atualizações?",
        "Renovationes quaerere?";
    OnOrOffThenHoursBetweenChecks:
        "on or off, then hours between checks: on 24",
        "on ou off, puis les heures entre deux vérifications : on 24",
        "on u off, y luego las horas entre búsquedas: on 24",
        "on sau off, apoi orele dintre verificări: on 24",
        "on ou off e depois as horas entre verificações: on 24",
        "on ou off e depois as horas entre verificações: on 24",
        "on aut off, deinde horae inter inspectiones: on 24";
    ApproveWhichSendRequest:
        "Approve which send request?",
        "Approuver quelle demande d’envoi ?",
        "¿Aprobar qué solicitud de envío?",
        "Aprobați ce cerere de trimitere?",
        "Aprovar qual solicitação de envio?",
        "Aprovar que pedido de envio?",
        "Quam petitionem missionis probare?";
    RejectWhichSendRequest:
        "Reject which send request?",
        "Rejeter quelle demande d’envoi ?",
        "¿Rechazar qué solicitud de envío?",
        "Respingeți ce cerere de trimitere?",
        "Rejeitar qual solicitação de envio?",
        "Rejeitar que pedido de envio?",
        "Quam petitionem missionis reicere?";
    TypeThenEnter:
        "Type, then Enter",
        "Saisissez, puis Entrée",
        "Escribe y pulsa Intro",
        "Tastați, apoi Enter",
        "Digite e pressione Enter",
        "Escreva e prima Enter",
        "Scribe, deinde Enter";
    CannotBeUndone:
        "cannot be undone",
        "irréversible",
        "no se puede deshacer",
        "ireversibil",
        "não pode ser desfeito",
        "não pode ser anulado",
        "revocari non potest";
    Filter:
        "Filter…",
        "Filtrer…",
        "Filtrar…",
        "Filtrați…",
        "Filtrar…",
        "Filtrar…",
        "Cola…";
    TypeACommandOrSearch:
        "Type a command or search…",
        "Saisissez une commande ou une recherche…",
        "Escribe un comando o una búsqueda…",
        "Tastați o comandă sau o căutare…",
        "Digite um comando ou uma pesquisa…",
        "Escreva um comando ou uma pesquisa…",
        "Mandatum aut quaestionem scribe…";
    Recent:
        "Recent",
        "Récents",
        "Recientes",
        "Recente",
        "Recentes",
        "Recentes",
        "Recentia";
    Threads:
        "Threads",
        "Discussions",
        "Conversaciones",
        "Conversații",
        "Conversas",
        "Conversas",
        "Colloquia";
    Date:
        "Date",
        "Date",
        "Fecha",
        "Data",
        "Data",
        "Data",
        "Dies";
    Mailbox:
        "Mailbox",
        "Dossier",
        "Carpeta",
        "Dosar",
        "Pasta",
        "Pasta",
        "Capsa";
    Size:
        "Size",
        "Taille",
        "Tamaño",
        "Dimensiune",
        "Tamanho",
        "Tamanho",
        "Magnitudo";
    Flags:
        "Flags",
        "Indicateurs",
        "Indicadores",
        "Indicatori",
        "Marcadores",
        "Sinalizadores",
        "Signa";
    Yes:
        "yes",
        "oui",
        "sí",
        "da",
        "sim",
        "sim",
        "ita";
    No:
        "no",
        "non",
        "no",
        "nu",
        "não",
        "não",
        "non";
    None:
        "none",
        "aucune",
        "ninguno",
        "niciunul",
        "nenhuma",
        "nenhuma",
        "nulla";
    OpenConversation:
        "Open conversation",
        "Ouvrir la discussion",
        "Abrir la conversación",
        "Deschideți conversația",
        "Abrir a conversa",
        "Abrir a conversa",
        "Colloquium aperi";
    LastMessage:
        "Last message",
        "Dernier message",
        "Último mensaje",
        "Ultimul mesaj",
        "Última mensagem",
        "Última mensagem",
        "Ultima epistula";
    Query:
        "Query",
        "Requête",
        "Consulta",
        "Interogare",
        "Consulta",
        "Consulta",
        "Quaestio";
    NoMailboxes:
        "No mailboxes",
        "Aucun dossier",
        "No hay carpetas",
        "Niciun dosar",
        "Nenhuma pasta",
        "Nenhuma pasta",
        "Nullae capsae";
    NoContacts:
        "No contacts",
        "Aucun contact",
        "No hay contactos",
        "Niciun contact",
        "Nenhum contato",
        "Nenhum contacto",
        "Nulli necessarii";
    NoPendingChanges:
        "No pending changes",
        "Aucune modification en attente",
        "No hay cambios pendientes",
        "Nicio modificare în așteptare",
        "Nenhuma alteração pendente",
        "Nenhuma alteração pendente",
        "Nullae mutationes exspectantes";
    NoSendRequestsX:
        "No send requests",
        "Aucune demande d’envoi",
        "No hay solicitudes de envío",
        "Nicio cerere de trimitere",
        "Nenhuma solicitação de envio",
        "Nenhum pedido de envio",
        "Nullae petitiones missionis";
    Testing:
        "Testing…",
        "Test…",
        "Probando…",
        "Se testează…",
        "Testando…",
        "A testar…",
        "Probatur…";
    Address:
        "Address",
        "Adresse",
        "Dirección",
        "Adresă",
        "Endereço",
        "Endereço",
        "Inscriptio";
    Domain:
        "Domain",
        "Domaine",
        "Dominio",
        "Domeniu",
        "Domínio",
        "Domínio",
        "Dominium";
    FoundIn:
        "Found in",
        "Trouvé dans",
        "Encontrado en",
        "Găsit în",
        "Encontrado em",
        "Encontrado em",
        "Inventum in";
    State:
        "State",
        "État",
        "Estado",
        "Stare",
        "Estado",
        "Estado",
        "Status";
    Value:
        "Value",
        "Valeur",
        "Valor",
        "Valoare",
        "Valor",
        "Valor",
        "Valor";
    Daily:
        "Daily",
        "Quotidien",
        "Diaria",
        "Zilnic",
        "Diária",
        "Diária",
        "Cottidie";
    Weekly:
        "Weekly",
        "Hebdomadaire",
        "Semanal",
        "Săptămânal",
        "Semanal",
        "Semanal",
        "Singulis hebdomadibus";
    Monthly:
        "Monthly",
        "Mensuel",
        "Mensual",
        "Lunar",
        "Mensal",
        "Mensal",
        "Singulis mensibus";
    CheckNow:
        "Check now",
        "Vérifier maintenant",
        "Buscar ahora",
        "Verificați acum",
        "Verificar agora",
        "Verificar agora",
        "Nunc inspice";
    Download:
        "Download",
        "Télécharger",
        "Descargar",
        "Descărcați",
        "Baixar",
        "Transferir",
        "Deprime";
    Install:
        "Install…",
        "Installer…",
        "Instalar…",
        "Instalați…",
        "Instalar…",
        "Instalar…",
        "Institue…";
    SkipThisVersion:
        "Skip this version",
        "Ignorer cette version",
        "Omitir esta versión",
        "Omiteți această versiune",
        "Pular esta versão",
        "Ignorar esta versão",
        "Hanc versionem praetermitte";
    ReleaseNotesX:
        "Release notes",
        "Notes de version",
        "Notas de la versión",
        "Note de lansare",
        "Notas da versão",
        "Notas da versão",
        "Notae versionis";
    ThisBuildDoesNotCheckForUpdates:
        "This build does not check for updates",
        "Cette version ne vérifie pas les mises à jour",
        "Esta compilación no busca actualizaciones",
        "Această versiune nu caută actualizări",
        "Esta versão não procura atualizações",
        "Esta versão não procura atualizações",
        "Haec versio renovationes non quaerit";
    DailIsUpToDate:
        "dail is up to date",
        "dail est à jour",
        "dail está actualizado",
        "dail este la zi",
        "O dail está atualizado",
        "O dail está atualizado",
        "dail recens est";
    NotCheckedYet:
        "Not checked yet",
        "Pas encore vérifié",
        "Aún no se ha comprobado",
        "Încă neverificat",
        "Ainda não verificado",
        "Ainda não verificado",
        "Nondum inspectum";
    CheckingForUpdates:
        "Checking for updates…",
        "Recherche de mises à jour…",
        "Buscando actualizaciones…",
        "Se caută actualizări…",
        "Procurando atualizações…",
        "A procurar atualizações…",
        "Renovationes quaeruntur…";
    RestartToUpdate:
        "Restart to update",
        "Redémarrer pour mettre à jour",
        "Reiniciar para actualizar",
        "Reporniți pentru actualizare",
        "Reiniciar para atualizar",
        "Reiniciar para atualizar",
        "Iterum incipe ut renoves";
    Checked:
        "checked",
        "cochée",
        "marcada",
        "bifată",
        "marcada",
        "marcada",
        "signatum";
    UnreadX:
        "unread",
        "non lu",
        "no leído",
        "necitit",
        "não lida",
        "não lida",
        "non lectum";
    StarredX:
        "starred",
        "suivi",
        "destacado",
        "cu stea",
        "com estrela",
        "com estrela",
        "stellatum";
    PinnedX:
        "pinned",
        "épinglé",
        "fijado",
        "fixat",
        "fixada",
        "fixada",
        "fixum";
    WithAttachments:
        "with attachments",
        "avec pièces jointes",
        "con adjuntos",
        "cu atașamente",
        "com anexos",
        "com anexos",
        "cum adiunctis";
    Unnamed:
        "(unnamed)",
        "(sans nom)",
        "(sin nombre)",
        "(fără nume)",
        "(sem nome)",
        "(sem nome)",
        "(sine nomine)";
    AccountX:
        "account",
        "compte",
        "cuenta",
        "cont",
        "conta",
        "conta",
        "ratio";
    AndMore:
        " and {n} more",
        " et {n} autres",
        " y {n} más",
        " și încă {n}",
        " e mais {n}",
        " e mais {n}",
        " et {n} alii";
    NewMessages:
        "{n} new messages",
        "{n} nouveaux messages",
        "{n} mensajes nuevos",
        "{n} mesaje noi",
        "{n} mensagens novas",
        "{n} mensagens novas",
        "{n} epistulae novae";
    MessagesCouldNotBeSent:
        "{n} messages could not be sent",
        "{n} messages n’ont pas pu être envoyés",
        "No se pudieron enviar {n} mensajes",
        "{n} mesaje nu au putut fi trimise",
        "{n} mensagens não puderam ser enviadas",
        "{n} mensagens não puderam ser enviadas",
        "{n} epistulae mitti non potuerunt";
    ToX:
        "To {to}: {subject}",
        "À {to} : {subject}",
        "Para {to}: {subject}",
        "Către {to}: {subject}",
        "Para {to}: {subject}",
        "Para {to}: {subject}",
        "Ad {to}: {subject}";
    AgentsWantToSendMessages:
        "Agents want to send {n} messages",
        "Des agents veulent envoyer {n} messages",
        "Unos agentes quieren enviar {n} mensajes",
        "Niște agenți vor să trimită {n} mesaje",
        "Agentes querem enviar {n} mensagens",
        "Agentes querem enviar {n} mensagens",
        "Agentes {n} epistulas mittere volunt";
    SendingInS:
        "sending in {n} s",
        "envoi dans {n} s",
        "enviando en {n} s",
        "se trimite în {n} s",
        "enviando em {n} s",
        "a enviar dentro de {n} s",
        "mittitur post {n} s";
    InS:
        "in {n}s",
        "dans {n} s",
        "en {n} s",
        "în {n} s",
        "em {n} s",
        "dentro de {n} s",
        "post {n} s";
    InM:
        "in {n}m",
        "dans {n} min",
        "en {n} min",
        "în {n} min",
        "em {n} min",
        "dentro de {n} min",
        "post {n} min";
    InH:
        "in {n}h",
        "dans {n} h",
        "en {n} h",
        "în {n} h",
        "em {n} h",
        "dentro de {n} h",
        "post {n} h";
    MAgo:
        "{n}m ago",
        "il y a {n} min",
        "hace {n} min",
        "acum {n} min",
        "há {n} min",
        "há {n} min",
        "ante {n} min";
    HAgo:
        "{n}h ago",
        "il y a {n} h",
        "hace {n} h",
        "acum {n} h",
        "há {n} h",
        "há {n} h",
        "ante {n} h";
    DAgo:
        "{n}d ago",
        "il y a {n} j",
        "hace {n} d",
        "acum {n} z",
        "há {n} d",
        "há {n} d",
        "ante {n} d";
    SendingInSX:
        "Sending in {n} s",
        "Envoi dans {n} s",
        "Enviando en {n} s",
        "Se trimite în {n} s",
        "Enviando em {n} s",
        "A enviar dentro de {n} s",
        "Mittitur post {n} s";
    RemoveAndItsPassword:
        "Remove {email} and its password",
        "Supprimer {email} et son mot de passe",
        "Quitar {email} y su contraseña",
        "Eliminați {email} și parola sa",
        "Remover {email} e sua senha",
        "Remover {email} e a sua palavra-passe",
        "Remove {email} et tesseram eius";
    InstallDail:
        "Install dail {version}",
        "Installer dail {version}",
        "Instalar dail {version}",
        "Instalați dail {version}",
        "Instalar o dail {version}",
        "Instalar o dail {version}",
        "Institue dail {version}";
    NewFolderIn:
        "New folder in {path}",
        "Nouveau dossier dans {path}",
        "Carpeta nueva en {path}",
        "Dosar nou în {path}",
        "Nova pasta em {path}",
        "Nova pasta em {path}",
        "Nova capsa in {path}";
    NowX:
        "now {now}",
        "actuellement {now}",
        "ahora {now}",
        "acum {now}",
        "agora {now}",
        "atualmente {now}",
        "nunc {now}";
    DeleteX:
        "Delete {path}",
        "Supprimer {path}",
        "Eliminar {path}",
        "Ștergeți {path}",
        "Excluir {path}",
        "Eliminar {path}",
        "Dele {path}";
    MarkAllUnreadMessagesReadUndoRestores:
        "Mark all {n} unread messages read? Undo restores only the cached ones.",
        "Marquer les {n} messages non lus comme lus ? L’annulation ne restaure que ceux en cache.",
        "¿Marcar los {n} mensajes no leídos como leídos? Deshacer solo restaura los que están en caché.",
        "Marcați toate cele {n} mesaje necitite ca citite? Anularea le restabilește doar pe cele din cache.",
        "Marcar as {n} mensagens não lidas como lidas? Desfazer só restaura as que estão em cache.",
        "Marcar as {n} mensagens não lidas como lidas? A anulação só restaura as que estão em cache.",
        "Omnes {n} epistulas non lectas ut lectas notare? Revocatio solas in memoria restituit.";
    EmptyTrashOfForever:
        "Delete the {n} messages in the Trash of {email} forever? This cannot be undone.",
        "Supprimer définitivement les {n} messages de la corbeille de {email} ? C’est irréversible.",
        "¿Eliminar para siempre los {n} mensajes de la papelera de {email}? No se puede deshacer.",
        "Ștergeți definitiv cele {n} mesaje din coșul de gunoi al {email}? Acțiunea nu poate fi anulată.",
        "Excluir para sempre as {n} mensagens da lixeira de {email}? Isso não pode ser desfeito.",
        "Eliminar definitivamente as {n} mensagens do lixo de {email}? Esta ação não pode ser anulada.",
        "{n} epistulas in quisquiliis {email} in perpetuum delere? Hoc revocari non potest.";
    EmptyTrash:
        "Empty Trash",
        "Vider la corbeille",
        "Vaciar la papelera",
        "Goliți coșul de gunoi",
        "Esvaziar a lixeira",
        "Esvaziar o lixo",
        "Quisquilias exhauri";
    EmptyTrashN:
        "Empty Trash ({n})",
        "Vider la corbeille ({n})",
        "Vaciar la papelera ({n})",
        "Goliți coșul de gunoi ({n})",
        "Esvaziar a lixeira ({n})",
        "Esvaziar o lixo ({n})",
        "Quisquilias exhauri ({n})";
    EmptyTrashOfWhichAccount:
        "Empty the Trash of which account?",
        "Vider la corbeille de quel compte ?",
        "¿Vaciar la papelera de qué cuenta?",
        "Goliți coșul de gunoi al cărui cont?",
        "Esvaziar a lixeira de qual conta?",
        "Esvaziar o lixo de que conta?",
        "Quisquilias cuius rationis exhaurire?";
    EmptyJunk:
        "Empty Junk",
        "Vider les indésirables",
        "Vaciar el spam",
        "Goliți dosarul spam",
        "Esvaziar o spam",
        "Esvaziar o spam",
        "Purgamentum exhauri";
    EmptyJunkN:
        "Empty Junk ({n})",
        "Vider les indésirables ({n})",
        "Vaciar el spam ({n})",
        "Goliți dosarul spam ({n})",
        "Esvaziar o spam ({n})",
        "Esvaziar o spam ({n})",
        "Purgamentum exhauri ({n})";
    EmptyJunkOfForever:
        "Delete the {n} messages in the Junk folder of {email} forever? This cannot be undone.",
        "Supprimer définitivement les {n} messages indésirables de {email} ? C’est irréversible.",
        "¿Eliminar para siempre los {n} mensajes de la carpeta de spam de {email}? No se puede deshacer.",
        "Ștergeți definitiv cele {n} mesaje din dosarul spam al {email}? Acțiunea nu poate fi anulată.",
        "Excluir para sempre as {n} mensagens da pasta de spam de {email}? Isso não pode ser desfeito.",
        "Eliminar definitivamente as {n} mensagens da pasta de spam de {email}? Esta ação não pode ser anulada.",
        "{n} epistulas in capsa purgamenti {email} in perpetuum delere? Hoc revocari non potest.";
    EmptyJunkOfWhichAccount:
        "Empty the Junk folder of which account?",
        "Vider les indésirables de quel compte ?",
        "¿Vaciar la carpeta de spam de qué cuenta?",
        "Goliți dosarul spam al cărui cont?",
        "Esvaziar a pasta de spam de qual conta?",
        "Esvaziar a pasta de spam de que conta?",
        "Capsam purgamenti cuius rationis exhaurire?";
    EmptyMailboxOp:
        "empty {mailbox}",
        "vider {mailbox}",
        "vaciar {mailbox}",
        "golire {mailbox}",
        "esvaziar {mailbox}",
        "esvaziar {mailbox}",
        "exhauri {mailbox}";
    MarkMessagesRead:
        "Mark {n} messages read",
        "Marquer {n} messages comme lus",
        "Marcar {n} mensajes como leídos",
        "Marcați {n} mesaje ca citite",
        "Marcar {n} mensagens como lidas",
        "Marcar {n} mensagens como lidas",
        "{n} epistulas ut lectas nota";
    NewValueFor:
        "New value for {key}",
        "Nouvelle valeur pour {key}",
        "Valor nuevo para {key}",
        "Valoare nouă pentru {key}",
        "Novo valor para {key}",
        "Novo valor para {key}",
        "Novus valor pro {key}";
    NowXX:
        "{allowed}, now {current}",
        "{allowed}, actuellement {current}",
        "{allowed}, ahora {current}",
        "{allowed}, acum {current}",
        "{allowed}, agora {current}",
        "{allowed}, atualmente {current}",
        "{allowed}, nunc {current}";
    GoTo:
        "Go to {name}",
        "Aller à {name}",
        "Ir a {name}",
        "Mergeți la {name}",
        "Ir para {name}",
        "Ir para {name}",
        "I ad {name}";
    Use:
        "Use “{query}”",
        "Utiliser « {query} »",
        "Usar «{query}»",
        "Folosiți „{query}”",
        "Usar “{query}”",
        "Usar “{query}”",
        "Utere «{query}»";
    SearchMailFor:
        "Search mail for “{query}”",
        "Rechercher « {query} » dans le courrier",
        "Buscar «{query}» en el correo",
        "Căutați „{query}” în e-mailuri",
        "Pesquisar “{query}” no e-mail",
        "Pesquisar “{query}” no correio",
        "Quaere «{query}» in cursu";
    UnreadXX:
        "{unread} unread · {total}",
        "{unread} non lus · {total}",
        "{unread} no leídos · {total}",
        "{unread} necitite · {total}",
        "{unread} não lidas · {total}",
        "{unread} não lidas · {total}",
        "{unread} non lecta · {total}";
    Attempts:
        "{state} · {n} attempts",
        "{state} · {n} tentatives",
        "{state} · {n} intentos",
        "{state} · {n} încercări",
        "{state} · {n} tentativas",
        "{state} · {n} tentativas",
        "{state} · {n} conatus";
    ToName:
        "To {to}",
        "Pour {to}",
        "Para {to}",
        "Către {to}",
        "Para {to}",
        "Para {to}",
        "Ad {to}";
    SignedIn:
        "Signed in {age}",
        "Connecté {age}",
        "Sesión iniciada {age}",
        "Autentificat {age}",
        "Login feito {age}",
        "Sessão iniciada {age}",
        "Ingressum {age}";
    DailIsAvailable:
        "dail {version} is available",
        "dail {version} est disponible",
        "dail {version} está disponible",
        "dail {version} este disponibil",
        "O dail {version} está disponível",
        "O dail {version} está disponível",
        "dail {version} praesto est";
    DownloadingDail:
        "Downloading dail {version}…",
        "Téléchargement de dail {version}…",
        "Descargando dail {version}…",
        "Se descarcă dail {version}…",
        "Baixando o dail {version}…",
        "A transferir o dail {version}…",
        "dail {version} deprimitur…";
    DailIsDownloadedAndVerified:
        "dail {version} is downloaded and verified",
        "dail {version} est téléchargé et vérifié",
        "dail {version} está descargado y verificado",
        "dail {version} este descărcat și verificat",
        "O dail {version} foi baixado e verificado",
        "O dail {version} foi transferido e verificado",
        "dail {version} depressum et inspectum est";
    InstallingDail:
        "Installing dail {version}…",
        "Installation de dail {version}…",
        "Instalando dail {version}…",
        "Se instalează dail {version}…",
        "Instalando o dail {version}…",
        "A instalar o dail {version}…",
        "dail {version} instituitur…";
    DailIsInstalledRestartDailToUse:
        "dail {version} is installed: restart dail to use it",
        "dail {version} est installé : redémarrez dail pour l’utiliser",
        "dail {version} está instalado: reinicia dail para usarlo",
        "dail {version} este instalat: reporniți dail pentru a-l folosi",
        "O dail {version} está instalado: reinicie o dail para usá-lo",
        "O dail {version} está instalado: reinicie o dail para o usar",
        "dail {version} institutum est: dail iterum incipe ut eo utaris";
    UpdateFailed:
        "Update failed: {reason}",
        "Échec de la mise à jour : {reason}",
        "Error al actualizar: {reason}",
        "Actualizarea a eșuat: {reason}",
        "Falha na atualização: {reason}",
        "Falha na atualização: {reason}",
        "Renovatio defecit: {reason}";
    CheckedX:
        "checked {age}",
        "vérifié {age}",
        "comprobado {age}",
        "verificat {age}",
        "verificado {age}",
        "verificado {age}",
        "inspectum {age}";
    Update:
        "Update {version}",
        "Mettre à jour vers {version}",
        "Actualizar a {version}",
        "Actualizați la {version}",
        "Atualizar para {version}",
        "Atualizar para {version}",
        "Renova ad {version}";
    EveryH:
        "every {n} h",
        "toutes les {n} h",
        "cada {n} h",
        "la fiecare {n} h",
        "a cada {n} h",
        "a cada {n} h",
        "singulis {n} h";
    UnreadXXX:
        "{label}, {n} unread",
        "{label}, {n} non lus",
        "{label}, {n} no leídos",
        "{label}, {n} necitite",
        "{label}, {n} não lidas",
        "{label}, {n} não lidas",
        "{label}, {n} non lecta";
    UpdateQuestion:
        "Check for new versions of dail?",
        "Rechercher les nouvelles versions de dail ?",
        "¿Buscar versiones nuevas de dail?",
        "Căutați versiuni noi de dail?",
        "Procurar novas versões do dail?",
        "Procurar novas versões do dail?",
        "Novas versiones dail quaerere?";
    UpdateQuestionDetail:
        "Once a day dail asks GitHub for the signed release list. Nothing about you or your mail is sent. You can change this in Settings.",
        "Une fois par jour, dail demande à GitHub la liste signée des versions. Rien sur vous ou votre courrier n’est envoyé. Vous pouvez changer cela dans les paramètres.",
        "Una vez al día, dail pide a GitHub la lista firmada de versiones. No se envía nada sobre ti ni sobre tu correo. Puedes cambiarlo en Ajustes.",
        "O dată pe zi, dail cere de la GitHub lista semnată de versiuni. Nu se trimite nimic despre dvs. sau despre e-mailurile dvs. Puteți schimba asta în Setări.",
        "Uma vez por dia, o dail pede ao GitHub a lista assinada de versões. Nada sobre você ou seus e-mails é enviado. Você pode mudar isso nas Configurações.",
        "Uma vez por dia, o dail pede ao GitHub a lista assinada de versões. Nada sobre si ou o seu correio é enviado. Pode alterar isto nas Definições.",
        "Semel in die dail a GitHub indicem signatum versionum petit. Nihil de te aut de epistulis tuis mittitur. Hoc in Optionibus mutare potes.";
    LanguageSetting:
        "Language",
        "Langue",
        "Idioma",
        "Limbă",
        "Idioma",
        "Idioma",
        "Lingua";
    LanguageHint:
        "The language of the app",
        "La langue de l’application",
        "El idioma de la aplicación",
        "Limba aplicației",
        "O idioma do aplicativo",
        "O idioma da aplicação",
        "Lingua applicationis";
    AddAnAccountInDailBeforeSharing:
        "Add an account in dail before sharing to it.",
        "Ajoutez un compte dans dail avant d’y partager.",
        "Añade una cuenta en dail antes de compartir con ella.",
        "Adăugați un cont în dail înainte de a partaja către el.",
        "Adicione uma conta no dail antes de compartilhar com ela.",
        "Adicione uma conta no dail antes de partilhar com ela.",
        "Rationem in dail adde antequam ad eam communices.";
    RestorePanes:
        "Restore panes",
        "Restaurer les panneaux",
        "Restaurar los paneles",
        "Restabiliți panourile",
        "Restaurar os painéis",
        "Restaurar os painéis",
        "Tabulata restitue";
    MaximizeMessage:
        "Maximize message",
        "Agrandir le message",
        "Maximizar el mensaje",
        "Maximizați mesajul",
        "Maximizar a mensagem",
        "Maximizar a mensagem",
        "Epistulam amplifica";
    About:
        "About",
        "À propos",
        "Acerca de",
        "Despre",
        "Sobre",
        "Acerca de",
        "De";
    ShowFolderNames:
        "Show folder names",
        "Afficher les noms des dossiers",
        "Mostrar los nombres de las carpetas",
        "Afișați numele dosarelor",
        "Mostrar os nomes das pastas",
        "Mostrar os nomes das pastas",
        "Nomina capsarum monstra";
    IconsOnly:
        "Icons only",
        "Icônes seules",
        "Solo iconos",
        "Doar pictograme",
        "Só ícones",
        "Só ícones",
        "Tantum icones";
    NoAccountsYetAddOneToRead:
        "No accounts yet. Add one to read your mail here.",
        "Aucun compte pour l’instant. Ajoutez-en un pour lire votre courrier ici.",
        "Aún no hay cuentas. Añade una para leer tu correo aquí.",
        "Încă nu există conturi. Adăugați unul pentru a vă citi e-mailurile aici.",
        "Ainda não há contas. Adicione uma para ler seus e-mails aqui.",
        "Ainda não há contas. Adicione uma para ler o seu correio aqui.",
        "Nullae rationes adhuc. Unam adde ut epistulas tuas hic legas.";
    PickAMailboxFromTheMenu:
        "Pick a mailbox from the menu.",
        "Choisissez un dossier dans le menu.",
        "Elige una carpeta en el menú.",
        "Alegeți un dosar din meniu.",
        "Escolha uma pasta no menu.",
        "Escolha uma pasta no menu.",
        "Capsam ex indice elige.";
    ForwardedMessage:
        "Forwarded message",
        "Message transféré",
        "Mensaje reenviado",
        "Mesaj redirecționat",
        "Mensagem encaminhada",
        "Mensagem reencaminhada",
        "Epistula transmissa";
    QuotedMessage:
        "Quoted message",
        "Message cité",
        "Mensaje citado",
        "Mesaj citat",
        "Mensagem citada",
        "Mensagem citada",
        "Epistula citata";
    Check:
        "Check",
        "Vérifier",
        "Comprobar",
        "Verificați",
        "Verificar",
        "Verificar",
        "Inspice";
    Open:
        "Open",
        "Ouvrir",
        "Abrir",
        "Deschideți",
        "Abrir",
        "Abrir",
        "Aperi";
    Skip:
        "Skip",
        "Ignorer",
        "Omitir",
        "Omiteți",
        "Pular",
        "Ignorar",
        "Praetermitte";
    NewMail:
        "New mail",
        "Nouveau courrier",
        "Correo nuevo",
        "E-mail nou",
        "Novos e-mails",
        "Correio novo",
        "Epistulae novae";
    AMessageWasNotSent:
        "A message was not sent",
        "Un message n’a pas été envoyé",
        "No se envió un mensaje",
        "Un mesaj nu a fost trimis",
        "Uma mensagem não foi enviada",
        "Uma mensagem não foi enviada",
        "Epistula non missa est";
    NewMailArrivesAsItComes:
        "New mail arrives as it comes",
        "Le courrier arrive au fil de l’eau",
        "El correo nuevo llega en cuanto se recibe",
        "E-mailurile noi sosesc pe măsură ce vin",
        "Os novos e-mails chegam assim que são recebidos",
        "O correio novo chega assim que é recebido",
        "Epistulae novae advenient ut accipiuntur";
    Stop:
        "Stop",
        "Arrêter",
        "Detener",
        "Opriți",
        "Parar",
        "Parar",
        "Siste";
    Battery:
        "Battery",
        "Batterie",
        "Batería",
        "Baterie",
        "Bateria",
        "Bateria",
        "Pila";
    ConnectingToYourInboxes:
        "Connecting to your inboxes",
        "Connexion à vos comptes",
        "Conectando con tus bandejas de entrada",
        "Conectare la căsuțele dvs. de intrare",
        "Conectando às suas caixas de entrada",
        "A ligar às suas caixas de entrada",
        "Conexio ad capsas acceptorum tuas";
    Watching1Inbox:
        "Watching 1 inbox",
        "Surveillance d’un dossier",
        "Vigilando 1 bandeja de entrada",
        "Se urmărește 1 căsuță de intrare",
        "Monitorando 1 caixa de entrada",
        "A monitorizar 1 caixa de entrada",
        "1 capsa acceptorum observatur";
    QuotedTextX:
        "Quoted text",
        "Texte cité",
        "Texto citado",
        "Text citat",
        "Texto citado",
        "Texto citado",
        "Textus citatus";
    SignatureX:
        "Signature",
        "Signature",
        "Firma",
        "Semnătură",
        "Assinatura",
        "Assinatura",
        "Subscriptio";
    Move:
        "Move…",
        "Déplacer…",
        "Mover…",
        "Mutați…",
        "Mover…",
        "Mover…",
        "Move…";
    CloseMessage:
        "Close message",
        "Fermer le message",
        "Cerrar el mensaje",
        "Închideți mesajul",
        "Fechar a mensagem",
        "Fechar a mensagem",
        "Epistulam claude";
    PreviousConversationX:
        "Previous conversation",
        "Discussion précédente",
        "Conversación anterior",
        "Conversația anterioară",
        "Conversa anterior",
        "Conversa anterior",
        "Colloquium prius";
    NextConversationX:
        "Next conversation",
        "Discussion suivante",
        "Conversación siguiente",
        "Conversația următoare",
        "Próxima conversa",
        "Conversa seguinte",
        "Colloquium sequens";
    Copy:
        "Copy",
        "Copier",
        "Copiar",
        "Copiați",
        "Copiar",
        "Copiar",
        "Exscribe";
    CopyAllText:
        "Copy all text",
        "Copier tout le texte",
        "Copiar todo el texto",
        "Copiați tot textul",
        "Copiar todo o texto",
        "Copiar todo o texto",
        "Totum textum exscribe";
    LinkCopied:
        "Link copied.",
        "Lien copié.",
        "Enlace copiado.",
        "Link copiat.",
        "Link copiado.",
        "Ligação copiada.",
        "Nexus exscriptus.";
    Save:
        "Save",
        "Enregistrer",
        "Guardar",
        "Salvați",
        "Salvar",
        "Guardar",
        "Serva";
    DailDoesNotShowThisKindOf:
        "dail does not show this kind of file. Save it to open it in another app.",
        "dail n’affiche pas ce type de fichier. Enregistrez-le pour l’ouvrir dans une autre application.",
        "dail no muestra este tipo de archivo. Guárdalo para abrirlo en otra aplicación.",
        "dail nu afișează acest tip de fișier. Salvați-l pentru a-l deschide în altă aplicație.",
        "O dail não exibe este tipo de arquivo. Salve-o para abri-lo em outro app.",
        "O dail não mostra este tipo de ficheiro. Guarde-o para o abrir noutra aplicação.",
        "dail hoc genus documenti non monstrat. Id serva ut in alia applicatione aperias.";
    ThisPictureCouldNotBeShown:
        "This picture could not be shown.",
        "Cette image n’a pas pu être affichée.",
        "No se pudo mostrar esta imagen.",
        "Această imagine nu a putut fi afișată.",
        "Não foi possível exibir esta imagem.",
        "Não foi possível mostrar esta imagem.",
        "Haec imago monstrari non potuit.";
    ThisPageCouldNotBeShown:
        "This page could not be shown.",
        "Cette page n’a pas pu être affichée.",
        "No se pudo mostrar esta página.",
        "Această pagină nu a putut fi afișată.",
        "Não foi possível exibir esta página.",
        "Não foi possível mostrar esta página.",
        "Haec pagina monstrari non potuit.";
    TheFileCouldNotBeRead:
        "The file could not be read.",
        "Le fichier n’a pas pu être lu.",
        "No se pudo leer el archivo.",
        "Fișierul nu a putut fi citit.",
        "Não foi possível ler o arquivo.",
        "Não foi possível ler o ficheiro.",
        "Documentum legi non potuit.";
    TheMessageIsNotDownloadedYet:
        "The message is not downloaded yet.",
        "Le message n’est pas encore téléchargé.",
        "El mensaje aún no se ha descargado.",
        "Mesajul nu este descărcat încă.",
        "A mensagem ainda não foi baixada.",
        "A mensagem ainda não foi transferida.",
        "Epistula nondum depressa est.";
    On:
        "On",
        "Activé",
        "Activado",
        "Activat",
        "Ativado",
        "Ativado",
        "Activum";
    CozyX:
        "Cozy",
        "Détendue",
        "Holgada",
        "Aerisită",
        "Confortável",
        "Confortável",
        "Laxa";
    CompactX:
        "Compact",
        "Compacte",
        "Compacta",
        "Compactă",
        "Compacta",
        "Compacta",
        "Densa";
    OnlyWhileOpen:
        "Only while open",
        "Seulement ouverte",
        "Solo mientras está abierta",
        "Doar cât este deschisă",
        "Só enquanto aberto",
        "Só enquanto aberto",
        "Tantum dum aperta est";
    Periodic:
        "Periodic",
        "Périodique",
        "Periódica",
        "Periodic",
        "Periódica",
        "Periódica",
        "Periodica";
    Push:
        "Push",
        "Push",
        "Push",
        "Push",
        "Push",
        "Push",
        "Push";
    CheckTheInboxEvery:
        "Check the inbox every",
        "Relever le dossier toutes les",
        "Comprobar la bandeja de entrada cada",
        "Verificați căsuța de intrare la fiecare",
        "Verificar a caixa de entrada a cada",
        "Verificar a caixa de entrada a cada",
        "Capsam acceptorum inspice singulis";
    MessagesFetchedFirst:
        "Messages fetched first",
        "Messages récupérés d’abord",
        "Mensajes descargados primero",
        "Mesaje descărcate inițial",
        "Mensagens baixadas primeiro",
        "Mensagens transferidas primeiro",
        "Epistulae primum depressae";
    ListDensity:
        "List density",
        "Densité de la liste",
        "Densidad de la lista",
        "Densitatea listei",
        "Densidade da lista",
        "Densidade da lista",
        "Densitas indicis";
    MailChecksInTheBackground:
        "Mail checks in the background",
        "Relève en arrière-plan",
        "Comprobaciones de correo en segundo plano",
        "Verificarea e-mailurilor în fundal",
        "Verificação de e-mails em segundo plano",
        "Verificação de correio em segundo plano",
        "Inspectio cursus in recessu";
    CheckEvery:
        "Check every",
        "Vérifier toutes les",
        "Comprobar cada",
        "Verificați la fiecare",
        "Verificar a cada",
        "Verificar a cada",
        "Inspice singulis";
    TerminalColours:
        "Terminal colours",
        "Couleurs du terminal",
        "Colores del terminal",
        "Culorile terminalului",
        "Cores do terminal",
        "Cores do terminal",
        "Colores terminalis";
    MarkAsReadAfter:
        "Mark as read after",
        "Marquer comme lu après",
        "Marcar como leído tras",
        "Marcați ca citit după",
        "Marcar como lida após",
        "Marcar como lida após",
        "Ut lectum nota post";
    RefreshAMailboxOnOpenAfter:
        "Refresh a mailbox on open after",
        "Actualiser un dossier à l’ouverture après",
        "Actualizar una carpeta al abrirla tras",
        "Actualizați un dosar la deschidere după",
        "Atualizar uma pasta ao abrir após",
        "Atualizar uma pasta ao abrir após",
        "Capsam aperiendo renova post";
    SyncEveryFolderEvery:
        "Sync every folder every",
        "Synchroniser tous les dossiers toutes les",
        "Sincronizar todas las carpetas cada",
        "Sincronizați toate dosarele la fiecare",
        "Sincronizar todas as pastas a cada",
        "Sincronizar todas as pastas a cada",
        "Omnes capsas synchroniza singulis";
    AfterArchiving:
        "After archiving",
        "Après l’archivage",
        "Tras archivar",
        "După arhivare",
        "Depois de arquivar",
        "Depois de arquivar",
        "Post archivationem";
    HowLongASendCanStillBe:
        "How long a send can still be cancelled.",
        "Le temps pendant lequel un envoi peut être annulé.",
        "Cuánto tiempo se puede cancelar aún un envío.",
        "Cât timp mai poate fi anulată o trimitere.",
        "Por quanto tempo um envio ainda pode ser cancelado.",
        "Durante quanto tempo um envio ainda pode ser cancelado.",
        "Quamdiu missio adhuc rescindi potest.";
    WhileTheAppIsOpen:
        "While the app is open.",
        "Tant que l’application est ouverte.",
        "Mientras la aplicación está abierta.",
        "Cât timp aplicația este deschisă.",
        "Enquanto o app está aberto.",
        "Enquanto a aplicação está aberta.",
        "Dum applicatio aperta est.";
    NewMailAndFailedSendsWhileDail:
        "New mail and failed sends, while dail is off screen.",
        "Nouveau courrier et envois en échec, quand dail n’est pas à l’écran.",
        "Correo nuevo y envíos fallidos, mientras dail no está en pantalla.",
        "E-mailuri noi și trimiteri eșuate, cât timp dail nu este pe ecran.",
        "Novos e-mails e envios com falha, enquanto o dail não está na tela.",
        "Correio novo e envios falhados, enquanto o dail não está no ecrã.",
        "Epistulae novae et missiones irritae, dum dail in velo non est.";
    PerMailboxOnItsFirstSync:
        "Per mailbox, on its first sync.",
        "Par dossier, à sa première synchronisation.",
        "Por carpeta, en su primera sincronización.",
        "Pe dosar, la prima sincronizare.",
        "Por pasta, na primeira sincronização.",
        "Por pasta, na primeira sincronização.",
        "Pro singulis capsis, prima synchronizatione.";
    Mode:
        "Mode",
        "Mode",
        "Modo",
        "Mod",
        "Modo",
        "Modo",
        "Modus";
    SystemFollowsAndroidSDarkTheme:
        "System follows Android's dark theme.",
        "Système suit le thème sombre d’Android.",
        "Sistema sigue el tema oscuro de Android.",
        "Sistem urmează tema întunecată din Android.",
        "Sistema segue o tema escuro do Android.",
        "Sistema segue o tema escuro do Android.",
        "Systema thema obscurum Androidis sequitur.";
    TextSizeFollowsAndroidSFontSize:
        "Text size follows Android's font size.",
        "La taille du texte suit celle d’Android.",
        "El tamaño del texto sigue el tamaño de fuente de Android.",
        "Dimensiunea textului urmează dimensiunea fontului din Android.",
        "O tamanho do texto segue o tamanho da fonte do Android.",
        "O tamanho do texto segue o tamanho do tipo de letra do Android.",
        "Magnitudo textus magnitudinem litterarum Androidis sequitur.";
    ChangeAKey:
        "Change a key…",
        "Changer une touche…",
        "Cambiar una tecla…",
        "Schimbați o tastă…",
        "Alterar uma tecla…",
        "Alterar uma tecla…",
        "Clavem muta…";
    HardwareKeyboard:
        "Hardware keyboard",
        "Clavier physique",
        "Teclado físico",
        "Tastatură fizică",
        "Teclado físico",
        "Teclado físico",
        "Claviatura corporea";
    ForAKeyboardOnATabletOr:
        "For a keyboard on a tablet or a Chromebook: the desktop's keys.",
        "Pour un clavier sur tablette ou Chromebook : les touches du bureau.",
        "Para un teclado en una tableta o un Chromebook: las teclas del escritorio.",
        "Pentru o tastatură pe tabletă sau Chromebook: tastele de pe desktop.",
        "Para um teclado em um tablet ou Chromebook: as teclas da área de trabalho.",
        "Para um teclado num tablet ou Chromebook: as teclas do ambiente de trabalho.",
        "Pro claviatura in tabella aut Chromebook: claves mensae.";
    Bindings:
        "Bindings",
        "Raccourcis",
        "Atajos",
        "Comenzi rapide",
        "Atalhos",
        "Atalhos",
        "Compendia";
    BatteryOptimization:
        "Battery optimization",
        "Optimisation de la batterie",
        "Optimización de batería",
        "Optimizarea bateriei",
        "Otimização de bateria",
        "Otimização de bateria",
        "Optimizatio pilae";
    NotificationSettings:
        "Notification settings",
        "Paramètres de notification",
        "Ajustes de notificaciones",
        "Setări pentru notificări",
        "Configurações de notificação",
        "Definições de notificações",
        "Optiones nuntiorum";
    SystemSettings:
        "System settings",
        "Paramètres système",
        "Ajustes del sistema",
        "Setări de sistem",
        "Configurações do sistema",
        "Definições do sistema",
        "Optiones systematis";
    BackgroundNotifications:
        "Background & notifications",
        "Arrière-plan et notifications",
        "Segundo plano y notificaciones",
        "Fundal și notificări",
        "Segundo plano e notificações",
        "Segundo plano e notificações",
        "Recessus et nuntii";
    NotTestedX:
        "Not tested",
        "Non testé",
        "Sin probar",
        "Netestat",
        "Não testada",
        "Não testada",
        "Non probata";
    ConnectedX:
        "Connected",
        "Connecté",
        "Conectado",
        "Conectat",
        "Conectada",
        "Ligada",
        "Conexa";
    Remove:
        "Remove",
        "Retirer",
        "Quitar",
        "Eliminați",
        "Remover",
        "Remover",
        "Remove";
    AnotherAccount:
        "Another account",
        "Un autre compte",
        "Otra cuenta",
        "Alt cont",
        "Outra conta",
        "Outra conta",
        "Alia ratio";
    GmailHostingerMailOrAnyIMAPAnd:
        "Gmail, Hostinger Mail or any IMAP and SMTP server.",
        "Gmail, Hostinger Mail ou tout serveur IMAP et SMTP.",
        "Gmail, Hostinger Mail o cualquier servidor IMAP y SMTP.",
        "Gmail, Hostinger Mail sau orice server IMAP și SMTP.",
        "Gmail, Hostinger Mail ou qualquer servidor IMAP e SMTP.",
        "Gmail, Hostinger Mail ou qualquer servidor IMAP e SMTP.",
        "Gmail, Hostinger Mail aut quilibet servitor IMAP et SMTP.";
    EnterTheServersYourselfOrFindThem:
        "Enter the servers yourself, or find them.",
        "Saisissez les serveurs vous-même, ou trouvez-les.",
        "Escribe los servidores tú o búscalos.",
        "Introduceți serverele manual sau găsiți-le.",
        "Digite os servidores você mesmo ou encontre-os.",
        "Introduza os servidores manualmente ou encontre-os.",
        "Servitores ipse insere aut inveni.";
    ShownInTheDrawer:
        "shown in the drawer",
        "affiché dans le tiroir",
        "se muestra en el panel lateral",
        "afișat în sertar",
        "exibido na gaveta",
        "mostrado na gaveta",
        "in loculo monstratur";
    View:
        "View",
        "Vue",
        "Vista",
        "Vizualizare",
        "Visualização",
        "Vista",
        "Conspectus";
    SearchingTheServer:
        "Searching the server…",
        "Recherche sur le serveur…",
        "Buscando en el servidor…",
        "Se caută pe server…",
        "Pesquisando no servidor…",
        "A pesquisar no servidor…",
        "In servitore quaeritur…";
    Selected:
        "{n} selected",
        "{n} sélectionnés",
        "{n} seleccionados",
        "{n} selectate",
        "{n} selecionadas",
        "{n} selecionadas",
        "{n} selecta";
    NotSentX:
        "Not sent: “{subject}”: {error}",
        "Non envoyé : « {subject} » : {error}",
        "No enviado: «{subject}»: {error}",
        "Netrimis: „{subject}”: {error}",
        "Não enviada: “{subject}”: {error}",
        "Não enviada: “{subject}”: {error}",
        "Non missa: «{subject}»: {error}";
    NewMailInConversations:
        "New mail in {n} conversations",
        "Nouveau courrier dans {n} discussions",
        "Correo nuevo en {n} conversaciones",
        "E-mailuri noi în {n} conversații",
        "Novos e-mails em {n} conversas",
        "Correio novo em {n} conversas",
        "Epistulae novae in {n} colloquiis";
    WatchingInboxes:
        "Watching {n} inboxes",
        "Surveillance de {n} dossiers",
        "Vigilando {n} bandejas de entrada",
        "Se urmăresc {n} căsuțe de intrare",
        "Monitorando {n} caixas de entrada",
        "A monitorizar {n} caixas de entrada",
        "{n} capsae acceptorum observantur";
    HideImagesFrom:
        "Hide images from {who}",
        "Masquer les images de {who}",
        "Ocultar las imágenes de {who}",
        "Ascundeți imaginile de la {who}",
        "Ocultar as imagens de {who}",
        "Ocultar as imagens de {who}",
        "Imagines a {who} cela";
    ShowImagesFrom:
        "Show images from {who}",
        "Afficher les images de {who}",
        "Mostrar las imágenes de {who}",
        "Afișați imaginile de la {who}",
        "Mostrar as imagens de {who}",
        "Mostrar as imagens de {who}",
        "Imagines a {who} monstra";
    SaveAllX:
        "Save all ({n})",
        "Tout enregistrer ({n})",
        "Guardar todo ({n})",
        "Salvați tot ({n})",
        "Salvar tudo ({n})",
        "Guardar tudo ({n})",
        "Omnia serva ({n})";
    Failed:
        "Failed: {error}",
        "Échec : {error}",
        "Error: {error}",
        "Eșuat: {error}",
        "Falhou: {error}",
        "Falhou: {error}",
        "Defecit: {error}";
    TheServerSearchFailed:
        "The server search failed: {error}",
        "La recherche sur le serveur a échoué : {error}",
        "Error en la búsqueda en el servidor: {error}",
        "Căutarea pe server a eșuat: {error}",
        "A pesquisa no servidor falhou: {error}",
        "A pesquisa no servidor falhou: {error}",
        "Quaestio in servitore defecit: {error}";
    NativeSaved:
        "Saved.",
        "Enregistré.",
        "Guardado.",
        "Salvat.",
        "Salvo.",
        "Guardado.",
        "Servatum.";
    NativeSavedN:
        "Saved {n} files.",
        "{n} fichiers enregistrés.",
        "{n} archivos guardados.",
        "{n} fișiere salvate.",
        "{n} arquivos salvos.",
        "{n} ficheiros guardados.",
        "{n} documenta servata.";
    NativeSavedPart:
        "Saved {n} of {m} files.",
        "{n} fichiers sur {m} enregistrés.",
        "{n} de {m} archivos guardados.",
        "{n} din {m} fișiere salvate.",
        "{n} de {m} arquivos salvos.",
        "{n} de {m} ficheiros guardados.",
        "{n} ex {m} documentis servata.";
    NativeNoLinkApp:
        "No app can open this link.",
        "Aucune application ne peut ouvrir ce lien.",
        "Ninguna aplicación puede abrir este enlace.",
        "Nicio aplicație nu poate deschide acest link.",
        "Nenhum app consegue abrir este link.",
        "Nenhuma aplicação consegue abrir esta ligação.",
        "Nulla applicatio hunc nexum aperire potest.";
    NativeNoSettings:
        "This phone has no such settings screen.",
        "Ce téléphone n’a pas cet écran de paramètres.",
        "Este teléfono no tiene esa pantalla de ajustes.",
        "Acest telefon nu are un astfel de ecran de setări.",
        "Este telefone não tem essa tela de configurações.",
        "Este telemóvel não tem esse ecrã de definições.",
        "Hoc telephonum talem paginam optionum non habet.";
    NativeRelease:
        "Release to open",
        "Relâchez pour ouvrir",
        "Suelta para abrir",
        "Eliberați pentru a deschide",
        "Solte para abrir",
        "Largue para abrir",
        "Dimitte ut aperias";
    NativeSave:
        "Save {name}",
        "Enregistrer {name}",
        "Guardar {name}",
        "Salvați {name}",
        "Salvar {name}",
        "Guardar {name}",
        "Serva {name}";
    NativeOpenLink:
        "Open this link?",
        "Ouvrir ce lien ?",
        "¿Abrir este enlace?",
        "Deschideți acest link?",
        "Abrir este link?",
        "Abrir esta ligação?",
        "Hunc nexum aperire?";
    NativePage:
        "Page {n} of {m}",
        "Page {n} sur {m}",
        "Página {n} de {m}",
        "Pagina {n} din {m}",
        "Página {n} de {m}",
        "Página {n} de {m}",
        "Pagina {n} ex {m}";
    NativeSelect:
        "Select",
        "Sélectionner",
        "Seleccionar",
        "Selectați",
        "Selecionar",
        "Selecionar",
        "Selige";
    NativeFailed:
        "Failed",
        "Échec",
        "Error",
        "Eșuat",
        "Falhou",
        "Falhou",
        "Defecit";
    NativePassed:
        "Passed",
        "Réussi",
        "Correcto",
        "Reușit",
        "Aprovado",
        "Passou",
        "Probatum";
    NativePending:
        "Pending",
        "En attente",
        "Pendiente",
        "În așteptare",
        "Pendente",
        "Pendente",
        "Exspectat";
    NativeMoreActions:
        "More actions",
        "Plus d’actions",
        "Más acciones",
        "Mai multe acțiuni",
        "Mais ações",
        "Mais ações",
        "Plures actiones";
    NativeFolderActions:
        "Folder actions",
        "Actions sur le dossier",
        "Acciones de la carpeta",
        "Acțiuni pentru dosar",
        "Ações da pasta",
        "Ações da pasta",
        "Actiones capsae";
    AboutDail:
        "About dail",
        "À propos de dail",
        "Acerca de dail",
        "Despre dail",
        "Sobre o dail",
        "Acerca do dail",
        "De dail";
    AboutDailVersion:
        "About dail ({version})",
        "À propos de dail ({version})",
        "Acerca de dail ({version})",
        "Despre dail ({version})",
        "Sobre o dail ({version})",
        "Acerca do dail ({version})",
        "De dail ({version})";
    AboutDesktopText:
        "Keyboard-first email client that agents can drive. Licence: {licence}.",
        "Client de messagerie piloté au clavier, que des agents peuvent commander. Licence : {licence}.",
        "Cliente de correo pensado para el teclado que los agentes pueden manejar. Licencia: {licence}.",
        "Client de e-mail gândit pentru tastatură, pe care îl pot comanda agenții. Licență: {licence}.",
        "Cliente de e-mail feito para o teclado, que agentes podem controlar. Licença: {licence}.",
        "Cliente de correio pensado para o teclado, que agentes podem controlar. Licença: {licence}.",
        "Cliens cursus electronici claviaturae aptus, quem agentes regere possunt. Licentia: {licence}.";
    AboutMobileText:
        "Email client for IMAP and SMTP accounts ({licence}). The app bundles these typefaces, icons, PDF fonts, Rust crates and Android libraries under their licences (SPDX identifiers: name, version, licence).",
        "Client de messagerie pour comptes IMAP et SMTP ({licence}). L’application intègre ces polices, icônes, polices PDF, crates Rust et bibliothèques Android sous leurs licences (identifiants SPDX : nom, version, licence).",
        "Cliente de correo para cuentas IMAP y SMTP ({licence}). La aplicación incluye estas tipografías, iconos, fuentes PDF, crates de Rust y bibliotecas de Android bajo sus licencias (identificadores SPDX: nombre, versión, licencia).",
        "Client de e-mail pentru conturi IMAP și SMTP ({licence}). Aplicația include aceste fonturi, pictograme, fonturi PDF, crate-uri Rust și biblioteci Android, sub licențele lor (identificatori SPDX: nume, versiune, licență).",
        "Cliente de e-mail para contas IMAP e SMTP ({licence}). O app inclui estas fontes, ícones, fontes de PDF, crates Rust e bibliotecas Android sob suas licenças (identificadores SPDX: nome, versão, licença).",
        "Cliente de correio para contas IMAP e SMTP ({licence}). A aplicação inclui estes tipos de letra, ícones, tipos de letra PDF, crates Rust e bibliotecas Android sob as respetivas licenças (identificadores SPDX: nome, versão, licença).",
        "Cliens cursus electronici pro rationibus IMAP et SMTP ({licence}). Applicatio has litteras, icones, litteras PDF, crates Rust et bibliothecas Android sub suis licentiis continet (signa SPDX: nomen, versio, licentia).";
    SourceCode:
        "Source code",
        "Code source",
        "Código fuente",
        "Cod sursă",
        "Código-fonte",
        "Código-fonte",
        "Codex fontis";
    Releases:
        "Releases",
        "Versions",
        "Versiones",
        "Versiuni",
        "Versões",
        "Versões",
        "Versiones";
    RustCrates:
        "Rust crates",
        "Crates Rust",
        "Crates de Rust",
        "Crate-uri Rust",
        "Crates Rust",
        "Crates Rust",
        "Crates Rust";
    AndroidLibraries:
        "Android libraries",
        "Bibliothèques Android",
        "Bibliotecas de Android",
        "Biblioteci Android",
        "Bibliotecas Android",
        "Bibliotecas Android",
        "Bibliothecae Android";
    PdfFontsAndCmaps:
        "PDF fonts and CMaps",
        "Polices PDF et CMaps",
        "Fuentes PDF y CMaps",
        "Fonturi PDF și CMap-uri",
        "Fontes PDF e CMaps",
        "Tipos de letra PDF e CMaps",
        "Litterae PDF et CMaps";
    PdfCraftViewer:
        "PdfCraft for Android (adapted viewer)",
        "PdfCraft pour Android (visionneuse adaptée)",
        "PdfCraft para Android (visor adaptado)",
        "PdfCraft pentru Android (vizualizator adaptat)",
        "PdfCraft para Android (visualizador adaptado)",
        "PdfCraft para Android (visualizador adaptado)",
        "PdfCraft pro Android (inspector accommodatus)";
    SharedRefused:
        "{n} shared files could not be attached: only files shared by other apps can.",
        "{n} fichiers partagés n’ont pas pu être joints : seuls les fichiers partagés par d’autres applications peuvent l’être.",
        "No se pudieron adjuntar {n} archivos compartidos: solo se pueden los que comparten otras aplicaciones.",
        "{n} fișiere partajate nu au putut fi atașate: se pot atașa doar fișierele partajate de alte aplicații.",
        "{n} arquivos compartilhados não puderam ser anexados: só podem os compartilhados por outros apps.",
        "{n} ficheiros partilhados não puderam ser anexados: só podem os partilhados por outras aplicações.",
        "{n} documenta communicata adiungi non potuerunt: sola ab aliis applicationibus communicata possunt.";
    SharedRefusedOne:
        "1 shared file could not be attached: only files shared by other apps can.",
        "1 fichier partagé n’a pas pu être joint : seuls les fichiers partagés par d’autres applications peuvent l’être.",
        "No se pudo adjuntar 1 archivo compartido: solo se pueden los que comparten otras aplicaciones.",
        "1 fișier partajat nu a putut fi atașat: se pot atașa doar fișierele partajate de alte aplicații.",
        "1 arquivo compartilhado não pôde ser anexado: só podem os compartilhados por outros apps.",
        "1 ficheiro partilhado não pôde ser anexado: só podem os partilhados por outras aplicações.",
        "1 documentum communicatum adiungi non potuit: sola ab aliis applicationibus communicata possunt.";
    LoginFailedColon:
        "The login failed: {error}",
        "La connexion a échoué : {error}",
        "Error al iniciar sesión: {error}",
        "Autentificarea a eșuat: {error}",
        "O login falhou: {error}",
        "O início de sessão falhou: {error}",
        "Ingressus defecit: {error}";
    HintUp:
        "up",
        "haut",
        "arriba",
        "sus",
        "acima",
        "cima",
        "sursum";
    HintDown:
        "down",
        "bas",
        "abajo",
        "jos",
        "abaixo",
        "baixo",
        "deorsum";
    WhichKeys:
        "Which keys?",
        "Quels raccourcis ?",
        "¿Qué atajos?",
        "Ce comenzi rapide?",
        "Quais atalhos?",
        "Quais atalhos?",
        "Quae compendia?";
    GmailOrVim:
        "gmail or vim",
        "gmail ou vim",
        "gmail o vim",
        "gmail sau vim",
        "gmail ou vim",
        "gmail ou vim",
        "gmail aut vim";
    FilterHint:
        "unread, starred, attachments (empty: all); now: {now}",
        "non lu, suivi, pièces jointes (vide : tout) ; actuellement : {now}",
        "no leído, destacado, adjuntos (vacío: todo); ahora: {now}",
        "necitit, cu stea, atașamente (gol: tot); acum: {now}",
        "não lida, com estrela, anexos (vazio: tudo); agora: {now}",
        "não lida, com estrela, anexos (vazio: tudo); atualmente: {now}",
        "non lectum, stellatum, adiuncta (vacuum: omnia); nunc: {now}";
    AttachmentsX:
        "attachments",
        "pièces jointes",
        "adjuntos",
        "atașamente",
        "anexos",
        "anexos",
        "adiuncta";
    ReadFlag:
        "read",
        "lu",
        "leído",
        "citit",
        "lida",
        "lida",
        "lectum";
    AnsweredFlag:
        "answered",
        "répondu",
        "respondido",
        "răspuns",
        "respondida",
        "respondida",
        "responsum";
    WindowN:
        " · window {n}",
        " · fenêtre {n}",
        " · ventana {n}",
        " · fereastră {n}",
        " · janela {n}",
        " · janela {n}",
        " · fenestra {n}";
    SetFlagOn:
        "set {flag} on {n} message(s)",
        "activer {flag} sur {n} message(s)",
        "activar {flag} en {n} mensaje(s)",
        "setați {flag} pe {n} mesaj(e)",
        "ativar {flag} em {n} mensagem(ns)",
        "ativar {flag} em {n} mensagem(ns)",
        "{flag} pone in {n} epistula(s)";
    ClearFlagOn:
        "clear {flag} on {n} message(s)",
        "retirer {flag} sur {n} message(s)",
        "quitar {flag} en {n} mensaje(s)",
        "eliminați {flag} de pe {n} mesaj(e)",
        "remover {flag} de {n} mensagem(ns)",
        "remover {flag} de {n} mensagem(ns)",
        "{flag} tolle ex {n} epistula(s)";
    SetFlagAll:
        "set {flag} on all of {mailbox}",
        "activer {flag} sur tout {mailbox}",
        "activar {flag} en todo {mailbox}",
        "setați {flag} pe tot {mailbox}",
        "ativar {flag} em toda a {mailbox}",
        "ativar {flag} em toda a {mailbox}",
        "{flag} pone in tota {mailbox}";
    ClearFlagAll:
        "clear {flag} on all of {mailbox}",
        "retirer {flag} sur tout {mailbox}",
        "quitar {flag} en todo {mailbox}",
        "eliminați {flag} de pe tot {mailbox}",
        "remover {flag} de toda a {mailbox}",
        "remover {flag} de toda a {mailbox}",
        "{flag} tolle ex tota {mailbox}";
    NoPreviewFor:
        "no preview for {type}",
        "pas d’aperçu pour {type}",
        "sin vista previa para {type}",
        "fără previzualizare pentru {type}",
        "sem prévia para {type}",
        "sem pré-visualização para {type}",
        "nulla praevisio pro {type}";
    ImageNotInMessage:
        "the image is not in the message",
        "l’image n’est pas dans le message",
        "la imagen no está en el mensaje",
        "imaginea nu este în mesaj",
        "a imagem não está na mensagem",
        "a imagem não está na mensagem",
        "imago in epistula non est";
    NotAnImage:
        "{type} is not an image",
        "{type} n’est pas une image",
        "{type} no es una imagen",
        "{type} nu este o imagine",
        "{type} não é uma imagem",
        "{type} não é uma imagem",
        "{type} imago non est";
    AttachmentNotStored:
        "the attachment is not in the local store",
        "la pièce jointe n’est pas dans le stockage local",
        "el adjunto no está en el almacenamiento local",
        "atașamentul nu este în stocarea locală",
        "o anexo não está no armazenamento local",
        "o anexo não está no armazenamento local",
        "adiunctum in repositorio locali non est";
    CouldNotOpenBrowser:
        "Could not open the browser: {error}",
        "Impossible d’ouvrir le navigateur : {error}",
        "No se pudo abrir el navegador: {error}",
        "Browserul nu a putut fi deschis: {error}",
        "Não foi possível abrir o navegador: {error}",
        "Não foi possível abrir o navegador: {error}",
        "Navigatrum aperiri non potuit: {error}";
    MessageGone:
        "The message is not in the local store any more.",
        "Le message n’est plus dans le stockage local.",
        "El mensaje ya no está en el almacenamiento local.",
        "Mesajul nu mai este în stocarea locală.",
        "A mensagem não está mais no armazenamento local.",
        "A mensagem já não está no armazenamento local.",
        "Epistula non iam in repositorio locali est.";
    AttachmentGone:
        "The attachment is not in the local store any more.",
        "La pièce jointe n’est plus dans le stockage local.",
        "El adjunto ya no está en el almacenamiento local.",
        "Atașamentul nu mai este în stocarea locală.",
        "O anexo não está mais no armazenamento local.",
        "O anexo já não está no armazenamento local.",
        "Adiunctum non iam in repositorio locali est.";
    CouldNotWriteOriginal:
        "Could not write the original: {error}",
        "Impossible d’écrire l’original : {error}",
        "No se pudo escribir el original: {error}",
        "Originalul nu a putut fi scris: {error}",
        "Não foi possível gravar o original: {error}",
        "Não foi possível escrever o original: {error}",
        "Archetypum scribi non potuit: {error}";
    CouldNotWriteAttachment:
        "Could not write the attachment: {error}",
        "Impossible d’écrire la pièce jointe : {error}",
        "No se pudo escribir el adjunto: {error}",
        "Atașamentul nu a putut fi scris: {error}",
        "Não foi possível gravar o anexo: {error}",
        "Não foi possível escrever o anexo: {error}",
        "Adiunctum scribi non potuit: {error}";
    NoEditor:
        "no editor",
        "aucun éditeur",
        "ningún editor",
        "niciun editor",
        "nenhum editor",
        "nenhum editor",
        "nullus editor";
    EditorExited:
        "{program} exited with {status}",
        "{program} s’est terminé avec {status}",
        "{program} terminó con {status}",
        "{program} s-a încheiat cu {status}",
        "{program} terminou com {status}",
        "{program} terminou com {status}",
        "{program} finivit cum {status}";
    TerminalCantDraw:
        "the terminal can't be drawn: {error}",
        "impossible de dessiner le terminal : {error}",
        "no se puede dibujar el terminal: {error}",
        "terminalul nu poate fi desenat: {error}",
        "não é possível desenhar o terminal: {error}",
        "não é possível desenhar o terminal: {error}",
        "terminale pingi non potest: {error}";
    InputClosed:
        "input closed",
        "entrée fermée",
        "entrada cerrada",
        "intrare închisă",
        "entrada fechada",
        "entrada fechada",
        "introitus clausus";
    TerminalWentAway:
        "the terminal went away",
        "le terminal a disparu",
        "el terminal desapareció",
        "terminalul a dispărut",
        "o terminal desapareceu",
        "o terminal desapareceu",
        "terminale evanuit";
    ReceivedSignal:
        "received {name}",
        "{name} reçu",
        "se recibió {name}",
        "s-a primit {name}",
        "{name} recebido",
        "{name} recebido",
        "{name} acceptum";
    ConsoleClose:
        "console close",
        "fermeture de la console",
        "cierre de la consola",
        "închiderea consolei",
        "fechamento do console",
        "fecho da consola",
        "clausura consolae";
    UnexpectedReceipt:
        "unexpected receipt: {error}",
        "accusé de réception inattendu : {error}",
        "acuse inesperado: {error}",
        "confirmare neașteptată: {error}",
        "recibo inesperado: {error}",
        "recibo inesperado: {error}",
        "acceptio inopinata: {error}";
    ToNameLower:
        "to {name}",
        "à {name}",
        "para {name}",
        "către {name}",
        "para {name}",
        "para {name}",
        "ad {name}";
    SyncedSettings:
        "Settings on the server",
        "Paramètres sur le serveur",
        "Ajustes en el servidor",
        "Setări pe server",
        "Configurações no servidor",
        "Definições no servidor",
        "Optiones in servitore";
    SettingsAccount:
        "Settings account",
        "Compte des paramètres",
        "Cuenta de los ajustes",
        "Contul setărilor",
        "Conta das configurações",
        "Conta das definições",
        "Ratio optionum";
    SettingsAccountHint:
        "Its hidden dail/Settings folder holds the settings shared by your devices",
        "Son dossier caché dail/Settings garde les paramètres partagés par vos appareils",
        "Su carpeta oculta dail/Settings guarda los ajustes que comparten tus dispositivos",
        "Dosarul ascuns dail/Settings al acestuia păstrează setările comune ale dispozitivelor dvs.",
        "A pasta oculta dail/Settings dela guarda as configurações compartilhadas pelos seus dispositivos",
        "A pasta oculta dail/Settings dela guarda as definições partilhadas pelos seus dispositivos",
        "Capsa eius occulta dail/Settings optiones machinis tuis communes servat";
    SettingsAccountFirst:
        "{email} (first account)",
        "{email} (premier compte)",
        "{email} (primera cuenta)",
        "{email} (primul cont)",
        "{email} (primeira conta)",
        "{email} (primeira conta)",
        "{email} (prima ratio)";
    SettingsAccountWhich:
        "Which account holds the settings?",
        "Quel compte garde les paramètres ?",
        "¿Qué cuenta guarda los ajustes?",
        "Ce cont păstrează setările?",
        "Qual conta guarda as configurações?",
        "Que conta guarda as definições?",
        "Quae ratio optiones servat?";
    SettingsReading:
        "reading from the server…",
        "lecture sur le serveur…",
        "leyendo del servidor…",
        "se citește de pe server…",
        "lendo do servidor…",
        "a ler do servidor…",
        "e servitore legitur…";
    SettingsWriting:
        "storing on the server…",
        "enregistrement sur le serveur…",
        "guardando en el servidor…",
        "se salvează pe server…",
        "salvando no servidor…",
        "a guardar no servidor…",
        "in servitore servatur…";
    SettingsUpToDate:
        "up to date",
        "à jour",
        "al día",
        "la zi",
        "atualizado",
        "atualizado",
        "recens";
    SettingsNotRead:
        "not read yet",
        "pas encore lus",
        "aún sin leer",
        "necitite încă",
        "ainda não lidas",
        "ainda não lidas",
        "nondum lecta";
    SettingsSyncState:
        "Settings sync",
        "Synchronisation des paramètres",
        "Sincronización de los ajustes",
        "Sincronizarea setărilor",
        "Sincronização das configurações",
        "Sincronização das definições",
        "Synchronizatio optionum";
    SettingsShared:
        "Follow this account on every device: the theme, language, list density, mark-read delay, undo-send window and what shows after triage. The rest stays on this device.",
        "Suivent ce compte sur chaque appareil : le thème, la langue, la densité de la liste, le délai avant lecture, le délai d’annulation d’envoi et ce qui s’affiche après le tri. Le reste reste sur cet appareil.",
        "Siguen a esta cuenta en todos los dispositivos: el tema, el idioma, la densidad de la lista, el retraso para marcar como leído, el plazo para deshacer el envío y lo que se muestra tras clasificar. El resto se queda en este dispositivo.",
        "Urmează acest cont pe fiecare dispozitiv: tema, limba, densitatea listei, întârzierea marcării ca citit, intervalul de anulare a trimiterii și ce se afișează după sortare. Restul rămâne pe acest dispozitiv.",
        "Acompanham esta conta em todos os dispositivos: o tema, o idioma, a densidade da lista, o atraso para marcar como lida, o prazo para desfazer o envio e o que aparece após a triagem. O resto fica neste dispositivo.",
        "Acompanham esta conta em todos os dispositivos: o tema, o idioma, a densidade da lista, o atraso para marcar como lida, o prazo de anulação do envio e o que aparece após a triagem. O resto fica neste dispositivo.",
        "Hanc rationem in omni machina sequuntur: thema, lingua, densitas indicis, mora ante lectionem, spatium revocandae missionis et quod post digestionem apparet. Cetera in hac machina manent.";
    RuleSenderContains:
        "Sort mail whose sender contains…",
        "Trier le courrier dont l’expéditeur contient…",
        "Clasificar el correo cuyo remitente contiene…",
        "Sortați e-mailurile al căror expeditor conține…",
        "Organizar os e-mails cujo remetente contém…",
        "Organizar o correio cujo remetente contém…",
        "Epistulas digere quarum missor continet…";
    RuleSenderHint:
        "e.g. newsletter@example.org",
        "par ex. newsletter@example.org",
        "p. ej. newsletter@example.org",
        "de ex. newsletter@example.org",
        "p. ex. newsletter@example.org",
        "p. ex. newsletter@example.org",
        "e.g. newsletter@example.org";
    RuleMoveToWhichFolder:
        "Move it to which folder?",
        "Le déplacer dans quel dossier ?",
        "¿Moverlo a qué carpeta?",
        "Îl mutați în ce dosar?",
        "Mover para qual pasta?",
        "Mover para que pasta?",
        "In quam capsam movere?";
    RuleFromTo:
        "From {sender} to {to}",
        "De {sender} vers {to}",
        "De {sender} a {to}",
        "De la {sender} în {to}",
        "De {sender} para {to}",
        "De {sender} para {to}",
        "A {sender} in {to}";
    DeleteWhichRule:
        "Delete which rule?",
        "Supprimer quelle règle ?",
        "¿Eliminar qué regla?",
        "Ștergeți ce regulă?",
        "Excluir qual regra?",
        "Eliminar que regra?",
        "Quam regulam delere?";
    ToggleWhichRule:
        "Turn which rule on or off?",
        "Activer ou désactiver quelle règle ?",
        "¿Activar o desactivar qué regla?",
        "Activați sau dezactivați ce regulă?",
        "Ativar ou desativar qual regra?",
        "Ativar ou desativar que regra?",
        "Quam regulam accendere aut exstinguere?";
    ApplyRulesWhichMailbox:
        "Run the rules on which mailbox?",
        "Appliquer les règles à quel dossier ?",
        "¿Aplicar las reglas a qué carpeta?",
        "Aplicați regulile pe ce dosar?",
        "Aplicar as regras a qual pasta?",
        "Aplicar as regras a que pasta?",
        "Regulas in quam capsam adhibere?";
    RuleOn:
        "on",
        "active",
        "activa",
        "activă",
        "ativa",
        "ativa",
        "activa";
    RuleOff:
        "off",
        "inactive",
        "inactiva",
        "inactivă",
        "inativa",
        "inativa",
        "inactiva";
    RulesTab:
        "Rules",
        "Règles",
        "Reglas",
        "Reguli",
        "Regras",
        "Regras",
        "Regulae";
    RulesTitle:
        "Sorting rules",
        "Règles de tri",
        "Reglas de clasificación",
        "Reguli de sortare",
        "Regras de organização",
        "Regras de organização",
        "Regulae digerendi";
    RulesHint:
        "dail runs these rules on new mail while it is open, on every device. They are stored with the settings on the server.",
        "dail applique ces règles au nouveau courrier tant qu’il est ouvert, sur chaque appareil. Elles sont enregistrées avec les paramètres sur le serveur.",
        "dail aplica estas reglas al correo nuevo mientras está abierto, en todos los dispositivos. Se guardan con los ajustes en el servidor.",
        "dail aplică aceste reguli e-mailurilor noi cât timp este deschis, pe fiecare dispozitiv. Sunt salvate împreună cu setările pe server.",
        "O dail aplica essas regras aos novos e-mails enquanto está aberto, em todos os dispositivos. Elas ficam salvas com as configurações no servidor.",
        "O dail aplica estas regras ao correio novo enquanto está aberto, em todos os dispositivos. Ficam guardadas com as definições no servidor.",
        "dail has regulas epistulis novis adhibet dum apertus est, in omni machina. Cum optionibus in servitore servantur.";
    RulesEmpty:
        "No rules yet.",
        "Aucune règle pour l’instant.",
        "Aún no hay reglas.",
        "Încă nu există reguli.",
        "Ainda não há regras.",
        "Ainda não há regras.",
        "Nullae regulae adhuc.";
    RulesNotStored:
        "Changes not stored on the server yet",
        "Modifications pas encore enregistrées sur le serveur",
        "Cambios aún no guardados en el servidor",
        "Modificări nesalvate încă pe server",
        "Alterações ainda não salvas no servidor",
        "Alterações ainda não guardadas no servidor",
        "Mutationes nondum in servitore servatae";
    RulesStored:
        "Stored on the server",
        "Enregistrées sur le serveur",
        "Guardadas en el servidor",
        "Salvate pe server",
        "Salvas no servidor",
        "Guardadas no servidor",
        "In servitore servatae";
    RulesStore:
        "Store on the server",
        "Enregistrer sur le serveur",
        "Guardar en el servidor",
        "Salvați pe server",
        "Salvar no servidor",
        "Guardar no servidor",
        "In servitore serva";
    RulesApplyHere:
        "Apply to this mailbox",
        "Appliquer à ce dossier",
        "Aplicar a esta carpeta",
        "Aplicați pe acest dosar",
        "Aplicar a esta pasta",
        "Aplicar a esta pasta",
        "Huic capsae adhibe";
    RulesApplyHint:
        "Open a mailbox to run the rules on it.",
        "Ouvrez un dossier pour lui appliquer les règles.",
        "Abre una carpeta para aplicarle las reglas.",
        "Deschideți un dosar pentru a-i aplica regulile.",
        "Abra uma pasta para aplicar as regras a ela.",
        "Abra uma pasta para lhe aplicar as regras.",
        "Capsam aperi ut ei regulas adhibeas.";
    AddRule:
        "Add a rule…",
        "Ajouter une règle…",
        "Añadir una regla…",
        "Adăugați o regulă…",
        "Adicionar uma regra…",
        "Adicionar uma regra…",
        "Regulam adde…";
    EditRule:
        "Edit…",
        "Modifier…",
        "Editar…",
        "Editați…",
        "Editar…",
        "Editar…",
        "Recense…";
    RuleEnabled:
        "Enabled",
        "Active",
        "Activa",
        "Activă",
        "Ativa",
        "Ativa",
        "Activa";
    RuleSummary:
        "{when} → {then}",
        "{when} → {then}",
        "{when} → {then}",
        "{when} → {then}",
        "{when} → {then}",
        "{when} → {then}",
        "{when} → {then}";
    RuleDoNothing:
        "do nothing",
        "ne rien faire",
        "no hacer nada",
        "nu face nimic",
        "não fazer nada",
        "não fazer nada",
        "nihil age";
    RuleEveryMessage:
        "every message",
        "tout message",
        "todos los mensajes",
        "orice mesaj",
        "todas as mensagens",
        "todas as mensagens",
        "omnis epistula";
    RuleNoMessage:
        "no message",
        "aucun message",
        "ningún mensaje",
        "niciun mesaj",
        "nenhuma mensagem",
        "nenhuma mensagem",
        "nulla epistula";
    RuleAnd:
        " and ",
        " et ",
        " y ",
        " și ",
        " e ",
        " e ",
        " et ";
    RuleOr:
        " or ",
        " ou ",
        " o ",
        " sau ",
        " ou ",
        " ou ",
        " aut ";
    RuleNot:
        "not ({condition})",
        "non ({condition})",
        "no ({condition})",
        "nu ({condition})",
        "não ({condition})",
        "não ({condition})",
        "non ({condition})";
    RuleCondFrom:
        "sender contains “{text}”",
        "l’expéditeur contient « {text} »",
        "el remitente contiene «{text}»",
        "expeditorul conține „{text}”",
        "o remetente contém “{text}”",
        "o remetente contém “{text}”",
        "missor continet «{text}»";
    RuleCondTo:
        "To contains “{text}”",
        "À contient « {text} »",
        "Para contiene «{text}»",
        "Către conține „{text}”",
        "Para contém “{text}”",
        "Para contém “{text}”",
        "Ad continet «{text}»";
    RuleCondCc:
        "Cc contains “{text}”",
        "Cc contient « {text} »",
        "CC contiene «{text}»",
        "Cc conține „{text}”",
        "Cc contém “{text}”",
        "Cc contém “{text}”",
        "Cc continet «{text}»";
    RuleCondSubject:
        "subject contains “{text}”",
        "l’objet contient « {text} »",
        "el asunto contiene «{text}»",
        "subiectul conține „{text}”",
        "o assunto contém “{text}”",
        "o assunto contém “{text}”",
        "res continet «{text}»";
    RuleCondList:
        "mailing list contains “{text}”",
        "la liste de diffusion contient « {text} »",
        "la lista de correo contiene «{text}»",
        "lista de corespondență conține „{text}”",
        "a lista de e-mails contém “{text}”",
        "a lista de correio contém “{text}”",
        "index epistularum continet «{text}»";
    RuleCondAttachment:
        "has an attachment",
        "a une pièce jointe",
        "tiene un adjunto",
        "are un atașament",
        "tem um anexo",
        "tem um anexo",
        "adiunctum habet";
    RuleCondMailbox:
        "in {path}",
        "dans {path}",
        "en {path}",
        "în {path}",
        "em {path}",
        "em {path}",
        "in {path}";
    RuleActMove:
        "move to {to}",
        "déplacer vers {to}",
        "mover a {to}",
        "mutare în {to}",
        "mover para {to}",
        "mover para {to}",
        "move in {to}";
    RuleActLabel:
        "label {label}",
        "étiqueter {label}",
        "etiquetar {label}",
        "etichetare {label}",
        "rotular {label}",
        "etiquetar {label}",
        "titulo nota {label}";
    RuleActRead:
        "mark read",
        "marquer comme lu",
        "marcar como leído",
        "marcare ca citit",
        "marcar como lida",
        "marcar como lida",
        "ut lectum nota";
    RuleActStar:
        "star",
        "suivre",
        "destacar",
        "marcare cu stea",
        "adicionar estrela",
        "adicionar estrela",
        "stella nota";
    RuleActArchive:
        "archive",
        "archiver",
        "archivar",
        "arhivare",
        "arquivar",
        "arquivar",
        "in archivum conde";
    RuleWhichTest:
        "Match mail by what?",
        "Reconnaître le courrier à quoi ?",
        "¿Reconocer el correo por qué?",
        "Recunoașteți e-mailurile după ce?",
        "Reconhecer os e-mails por quê?",
        "Reconhecer o correio por quê?",
        "Epistulas qua re agnoscere?";
    RuleTestFrom:
        "Sender",
        "Expéditeur",
        "Remitente",
        "Expeditor",
        "Remetente",
        "Remetente",
        "Missor";
    RuleTestTo:
        "Recipient (To)",
        "Destinataire (À)",
        "Destinatario (Para)",
        "Destinatar (Către)",
        "Destinatário (Para)",
        "Destinatário (Para)",
        "Recipiens (Ad)";
    RuleTestCc:
        "Copy (Cc)",
        "Copie (Cc)",
        "Copia (CC)",
        "Copie (Cc)",
        "Cópia (Cc)",
        "Cópia (Cc)",
        "Exemplar (Cc)";
    RuleTestSubject:
        "Subject",
        "Objet",
        "Asunto",
        "Subiect",
        "Assunto",
        "Assunto",
        "Res";
    RuleTestList:
        "Mailing list",
        "Liste de diffusion",
        "Lista de correo",
        "Listă de corespondență",
        "Lista de e-mails",
        "Lista de correio",
        "Index epistularum";
    RuleTestAttachment:
        "Has an attachment",
        "A une pièce jointe",
        "Tiene un adjunto",
        "Are un atașament",
        "Tem um anexo",
        "Tem um anexo",
        "Adiunctum habet";
    RuleContainsWhat:
        "Containing what?",
        "Contenant quoi ?",
        "¿Que contenga qué?",
        "Care conține ce?",
        "Contendo o quê?",
        "Contendo o quê?",
        "Quid continens?";
    RuleContainsHint:
        "Upper or lower case does not matter",
        "Majuscules et minuscules indifférentes",
        "Da igual mayúsculas o minúsculas",
        "Nu contează literele mari sau mici",
        "Maiúsculas ou minúsculas tanto faz",
        "Maiúsculas ou minúsculas são indiferentes",
        "Litterae maiores aut minores nihil refert";
    RuleWhichAction:
        "Then do what?",
        "Puis faire quoi ?",
        "¿Y luego qué hacer?",
        "Apoi ce să facă?",
        "E depois fazer o quê?",
        "E depois fazer o quê?",
        "Deinde quid agere?";
    RuleActionMove:
        "Move to a folder…",
        "Déplacer dans un dossier…",
        "Mover a una carpeta…",
        "Mutare într-un dosar…",
        "Mover para uma pasta…",
        "Mover para uma pasta…",
        "In capsam move…";
    RuleActionArchive:
        "Archive",
        "Archiver",
        "Archivar",
        "Arhivare",
        "Arquivar",
        "Arquivar",
        "In archivum conde";
    RuleActionRead:
        "Mark read",
        "Marquer comme lu",
        "Marcar como leído",
        "Marcare ca citit",
        "Marcar como lida",
        "Marcar como lida",
        "Ut lectum nota";
    RuleActionStar:
        "Star",
        "Suivre",
        "Destacar",
        "Marcare cu stea",
        "Adicionar estrela",
        "Adicionar estrela",
        "Stella nota";
    RuleEditWhat:
        "Change what?",
        "Modifier quoi ?",
        "¿Cambiar qué?",
        "Ce schimbați?",
        "Alterar o quê?",
        "Alterar o quê?",
        "Quid mutare?";
    RuleEditName:
        "Name",
        "Nom",
        "Nombre",
        "Nume",
        "Nome",
        "Nome",
        "Nomen";
    RuleEditCondition:
        "Condition",
        "Condition",
        "Condición",
        "Condiție",
        "Condição",
        "Condição",
        "Condicio";
    RuleEditAction:
        "Action",
        "Action",
        "Acción",
        "Acțiune",
        "Ação",
        "Ação",
        "Actio";
    RuleDelete:
        "Delete",
        "Supprimer",
        "Eliminar",
        "Ștergeți",
        "Excluir",
        "Eliminar",
        "Dele";
    RuleNewName:
        "New name?",
        "Nouveau nom ?",
        "¿Nombre nuevo?",
        "Nume nou?",
        "Novo nome?",
        "Novo nome?",
        "Novum nomen?";
    MakeRuleFromMessage:
        "Make a rule from this message",
        "Créer une règle à partir de ce message",
        "Crear una regla a partir de este mensaje",
        "Creați o regulă din acest mesaj",
        "Criar uma regra a partir desta mensagem",
        "Criar uma regra a partir desta mensagem",
        "Regulam ex hac epistula crea";
    RuleFromMessageWhich:
        "Match mail like this one by…",
        "Reconnaître le courrier semblable par…",
        "Reconocer el correo como este por…",
        "Recunoașteți e-mailurile ca acesta după…",
        "Reconhecer e-mails como este por…",
        "Reconhecer correio como este por…",
        "Epistulas huic similes agnosce per…";
    RulesFooter:
        "Tab section · j/k move · Enter edit · Space on/off · a add · x apply · p store · Esc close",
        "Tab section · j/k déplacer · Entrée modifier · Espace activer · a ajouter · x appliquer · p enregistrer · Échap fermer",
        "Tab sección · j/k mover · Intro editar · Espacio activar · a añadir · x aplicar · p guardar · Esc cerrar",
        "Tab secțiune · j/k deplasare · Enter editați · Spațiu activați · a adăugați · x aplicați · p salvați · Esc închideți",
        "Tab seção · j/k mover · Enter editar · Espaço ativar · a adicionar · x aplicar · p salvar · Esc fechar",
        "Tab secção · j/k mover · Enter editar · Espaço ativar · a adicionar · x aplicar · p guardar · Esc fechar",
        "Tab sectio · j/k move · Enter recense · Spatium accende · a adde · x adhibe · p serva · Esc claude";
    RuleEditComplex:
        "Change what? This rule's conditions or actions are too complex to edit here: use the CLI or an agent.",
        "Modifier quoi ? Les conditions ou actions de cette règle sont trop complexes pour être modifiées ici : utilisez la CLI ou un agent.",
        "¿Cambiar qué? Las condiciones o acciones de esta regla son demasiado complejas para editarlas aquí: usa la CLI o un agente.",
        "Ce schimbați? Condițiile sau acțiunile acestei reguli sunt prea complexe pentru a fi editate aici: folosiți CLI-ul sau un agent.",
        "Alterar o quê? As condições ou ações desta regra são complexas demais para editar aqui: use a CLI ou um agente.",
        "Alterar o quê? As condições ou ações desta regra são demasiado complexas para editar aqui: use a CLI ou um agente.",
        "Quid mutare? Condiciones aut actiones huius regulae nimis implicatae sunt ut hic recenseantur: CLI aut agente utere.";
    RuleEnable:
        "Turn on",
        "Activer",
        "Activar",
        "Activați",
        "Ativar",
        "Ativar",
        "Accende";
    RuleDisable:
        "Turn off",
        "Désactiver",
        "Desactivar",
        "Dezactivați",
        "Desativar",
        "Desativar",
        "Exstingue";
    MessageActionName:
        "{action}, message from {sender}",
        "{action}, message de {sender}",
        "{action}, mensaje de {sender}",
        "{action}, mesaj de la {sender}",
        "{action}, mensagem de {sender}",
        "{action}, mensagem de {sender}",
        "{action}, epistula a {sender}";
    MessageCursorHelp:
        "n p            the message cursor: the next / previous message of the conversation; its keys (r a f u s #) then act on that message alone, Enter edits a draft",
        "n p            le curseur de message : le message suivant / précédent de la discussion ; ses touches (r a f u s #) agissent alors sur ce seul message, Entrée modifie un brouillon",
        "n p            el cursor de mensaje: el mensaje siguiente / anterior de la conversación; sus teclas (r a f u s #) actúan entonces solo sobre ese mensaje, Intro edita un borrador",
        "n p            cursorul de mesaj: mesajul următor / anterior din conversație; tastele lui (r a f u s #) acționează apoi doar asupra acelui mesaj, Enter editează o ciornă",
        "n p            o cursor de mensagem: a próxima / anterior mensagem da conversa; suas teclas (r a f u s #) agem então só sobre essa mensagem, Enter edita um rascunho",
        "n p            o cursor de mensagem: a mensagem seguinte / anterior da conversa; as suas teclas (r a f u s #) agem então só sobre essa mensagem, Enter edita um rascunho",
        "n p            cursor epistulae: epistula sequens / prior colloquii; claves eius (r a f u s #) tum in sola illa epistula agunt, Enter adumbrationem recenset";
    NoteDelete:
        "Delete",
        "Supprimer",
        "Eliminar",
        "Ștergeți",
        "Excluir",
        "Eliminar",
        "Dele";
    NewConversations:
        "{n} new conversations",
        "{n} nouvelles discussions",
        "{n} conversaciones nuevas",
        "{n} conversații noi",
        "{n} conversas novas",
        "{n} conversas novas",
        "{n} colloquia nova";
    MakeRule:
        "Make a rule…",
        "Créer une règle…",
        "Crear una regla…",
        "Creați o regulă…",
        "Criar uma regra…",
        "Criar uma regra…",
        "Regulam crea…";
    ApplyRules:
        "Apply rules",
        "Appliquer les règles",
        "Aplicar las reglas",
        "Aplicați regulile",
        "Aplicar as regras",
        "Aplicar as regras",
        "Regulas adhibe";
    OfflineWindow:
        "Offline window…",
        "Copie hors ligne…",
        "Copia sin conexión…",
        "Copie offline…",
        "Cópia offline…",
        "Cópia offline…",
        "Exemplar sine conexione…";
    SendingAddresses:
        "Sending addresses…",
        "Adresses d’envoi…",
        "Direcciones de envío…",
        "Adrese de trimitere…",
        "Endereços de envio…",
        "Endereços de envio…",
        "Inscriptiones missionis…";
    NotificationsBackground:
        "Notifications & background",
        "Notifications et arrière-plan",
        "Notificaciones y segundo plano",
        "Notificări și fundal",
        "Notificações e segundo plano",
        "Notificações e segundo plano",
        "Nuntii et recessus";
    NoKeyboardAttached:
        "No keyboard attached",
        "Aucun clavier branché",
        "Ningún teclado conectado",
        "Nicio tastatură conectată",
        "Nenhum teclado conectado",
        "Nenhum teclado ligado",
        "Nulla claviatura coniuncta";
    SettingsAccountCount:
        "{n} account(s)",
        "{n} compte(s)",
        "{n} cuenta(s)",
        "{n} cont(uri)",
        "{n} conta(s)",
        "{n} conta(s)",
        "{n} ratio(nes)";
    SettingsRuleCount:
        "{n} rule(s)",
        "{n} règle(s)",
        "{n} regla(s)",
        "{n} regulă(i)",
        "{n} regra(s)",
        "{n} regra(s)",
        "{n} regula(e)";
    BackgroundSyncHint:
        "Periodic checks wait for the phone's maintenance windows. Push keeps a silent notification while it watches the inboxes.",
        "Les vérifications périodiques attendent les fenêtres de maintenance du téléphone. Le mode push garde une notification silencieuse pendant qu’il surveille les boîtes de réception.",
        "Las comprobaciones periódicas esperan a las ventanas de mantenimiento del teléfono. Push mantiene una notificación silenciosa mientras vigila las bandejas de entrada.",
        "Verificările periodice așteaptă ferestrele de mentenanță ale telefonului. Modul push păstrează o notificare silențioasă cât timp urmărește căsuțele de intrare.",
        "As verificações periódicas aguardam as janelas de manutenção do telefone. O push mantém uma notificação silenciosa enquanto monitora as caixas de entrada.",
        "As verificações periódicas aguardam as janelas de manutenção do telemóvel. O push mantém uma notificação silenciosa enquanto monitoriza as caixas de entrada.",
        "Inspectiones periodicae fenestras curationis telephoni exspectant. Push nuntium tacitum servat dum capsas acceptorum observat.";
    BatteryExemptHint:
        "Some phones stop background checks until dail is exempt from battery optimization.",
        "Certains téléphones arrêtent les vérifications en arrière-plan tant que dail n’est pas exempté de l’optimisation de la batterie.",
        "Algunos teléfonos detienen las comprobaciones en segundo plano hasta que dail queda exento de la optimización de batería.",
        "Unele telefoane opresc verificările din fundal până când dail este exceptat de la optimizarea bateriei.",
        "Alguns telefones param as verificações em segundo plano até que o dail seja isento da otimização de bateria.",
        "Alguns telemóveis param as verificações em segundo plano até que o dail seja isento da otimização da bateria.",
        "Quaedam telephona inspectiones in recessu sistunt donec dail ab optimizatione pilae eximatur.";
    SettingsLastPulled:
        "Last pulled",
        "Dernière lecture",
        "Última lectura",
        "Ultima citire",
        "Última leitura",
        "Última leitura",
        "Ultima lectio";
    SettingsLastStored:
        "Last stored",
        "Dernier enregistrement",
        "Último guardado",
        "Ultima salvare",
        "Último salvamento",
        "Última gravação",
        "Ultima servatio";
    SettingsPullNow:
        "Pull now",
        "Lire maintenant",
        "Leer ahora",
        "Citiți acum",
        "Ler agora",
        "Ler agora",
        "Nunc lege";
    SettingsLastError:
        "Last error",
        "Dernière erreur",
        "Último error",
        "Ultima eroare",
        "Último erro",
        "Último erro",
        "Ultimus error";
    PairWhichKindOfDevice:
        "Pair which kind of device?",
        "Associer quel type d’appareil ?",
        "¿Qué tipo de dispositivo quieres vincular?",
        "Ce tip de dispozitiv asociați?",
        "Parear qual tipo de dispositivo?",
        "Emparelhar que tipo de dispositivo?",
        "Quod genus instrumenti copulare?";
    DeviceOfYours:
        "A device of yours",
        "Un de vos appareils",
        "Un dispositivo tuyo",
        "Un dispozitiv al dvs.",
        "Um dispositivo seu",
        "Um dispositivo seu",
        "Instrumentum tuum";
    DeviceOfYoursDetail:
        "It can act as you and approve sends",
        "Il peut agir en votre nom et approuver les envois",
        "Puede actuar como tú y aprobar envíos",
        "Poate acționa în numele dvs. și aproba trimiterile",
        "Ele pode agir como você e aprovar envios",
        "Pode agir como o utilizador e aprovar envios",
        "Pro te agere et missiones probare potest";
    AnAgent:
        "An agent",
        "Un agent",
        "Un agente",
        "Un agent",
        "Um agente",
        "Um agente",
        "Agens";
    AnAgentDetail:
        "It can act only as an agent (MCP)",
        "Il ne peut agir qu’en tant qu’agent (MCP)",
        "Solo puede actuar como agente (MCP)",
        "Poate acționa doar ca agent (MCP)",
        "Ele só pode agir como agente (MCP)",
        "Só pode agir como agente (MCP)",
        "Solum ut agens (MCP) agere potest";
    DeviceNameQuestion:
        "Its name?",
        "Son nom ?",
        "¿Su nombre?",
        "Numele său?",
        "O nome dele?",
        "O nome?",
        "Nomen eius?";
    DeviceNameHint:
        "Phone",
        "Téléphone",
        "Teléfono",
        "Telefon",
        "Celular",
        "Telemóvel",
        "Telephonum";
    RevokeWhichDevice:
        "Revoke which device?",
        "Révoquer quel appareil ?",
        "¿Revocar qué dispositivo?",
        "Revocați ce dispozitiv?",
        "Revogar qual dispositivo?",
        "Revogar que dispositivo?",
        "Quod instrumentum revocare?";
    RevokeThisDevice:
        "Revoke this device? It has to be paired again to reconnect.",
        "Révoquer cet appareil ? Il devra être associé de nouveau pour se reconnecter.",
        "¿Revocar este dispositivo? Habrá que vincularlo de nuevo para reconectarlo.",
        "Revocați acest dispozitiv? Va trebui asociat din nou pentru a se reconecta.",
        "Revogar este dispositivo? Será preciso pareá-lo de novo para reconectar.",
        "Revogar este dispositivo? Terá de ser emparelhado de novo para voltar a ligar.",
        "Hoc instrumentum revocare? Iterum copulandum erit ut rursus conectat.";
    RevokeName:
        "Revoke {name}",
        "Révoquer {name}",
        "Revocar {name}",
        "Revocați {name}",
        "Revogar {name}",
        "Revogar {name}",
        "Revoca {name}";
    WhichDailServer:
        "Which dail server?",
        "Quel serveur dail ?",
        "¿Qué servidor dail?",
        "Ce server dail?",
        "Qual servidor dail?",
        "Que servidor dail?",
        "Quis servitor dail?";
    ServerHostPortHint:
        "host:port, e.g. home.example.org:7443",
        "hôte:port, par ex. home.example.org:7443",
        "host:puerto, p. ej. home.example.org:7443",
        "gazdă:port, de ex. home.example.org:7443",
        "host:porta, ex.: home.example.org:7443",
        "anfitrião:porta, p. ex. home.example.org:7443",
        "hospes:porta, e.g. home.example.org:7443";
    DoesTheServerShowThisFingerprint:
        "Does the dail server show this fingerprint?",
        "Le serveur dail affiche-t-il cette empreinte ?",
        "¿El servidor dail muestra esta huella?",
        "Serverul dail afișează această amprentă?",
        "O servidor dail mostra esta impressão digital?",
        "O servidor dail mostra esta impressão digital?",
        "Monstratne servitor dail hoc vestigium?";
    TrustFingerprint:
        "Trust {fingerprint}",
        "Faire confiance à {fingerprint}",
        "Confiar en {fingerprint}",
        "Acceptați {fingerprint}",
        "Confiar em {fingerprint}",
        "Confiar em {fingerprint}",
        "Confide {fingerprint}";
    PairingCodeQuestion:
        "The pairing code the server shows?",
        "Le code d’association affiché par le serveur ?",
        "¿El código de vinculación que muestra el servidor?",
        "Codul de asociere afișat de server?",
        "O código de pareamento que o servidor mostra?",
        "O código de emparelhamento que o servidor mostra?",
        "Nota copulationis quam servitor monstrat?";
    DisconnectFromTheServer:
        "Disconnect from the dail server? Its mail and the offline changes not sent yet leave this device.",
        "Se déconnecter du serveur dail ? Son courrier et les modifications hors ligne pas encore envoyées quittent cet appareil.",
        "¿Desconectarte del servidor dail? Su correo y los cambios sin conexión aún no enviados salen de este dispositivo.",
        "Vă deconectați de la serverul dail? E-mailurile sale și modificările offline încă netrimise dispar de pe acest dispozitiv.",
        "Desconectar do servidor dail? Os e-mails dele e as alterações offline ainda não enviadas saem deste dispositivo.",
        "Desligar do servidor dail? O correio dele e as alterações offline ainda não enviadas saem deste dispositivo.",
        "A servitore dail disiungere? Epistulae eius et mutationes sine conexione nondum missae ex hoc instrumento abeunt.";
    DisconnectFrom:
        "Disconnect from {server}",
        "Se déconnecter de {server}",
        "Desconectarte de {server}",
        "Deconectați-vă de la {server}",
        "Desconectar de {server}",
        "Desligar de {server}",
        "Disiunge a {server}";
    DismissWhichRefusedChange:
        "Dismiss which refused change?",
        "Abandonner quelle modification refusée ?",
        "¿Descartar qué cambio rechazado?",
        "Renunțați la ce modificare refuzată?",
        "Descartar qual alteração recusada?",
        "Rejeitar que alteração recusada?",
        "Quam mutationem recusatam abicere?";
    PairADevice:
        "Pair a device…",
        "Associer un appareil…",
        "Vincular un dispositivo…",
        "Asociați un dispozitiv…",
        "Parear um dispositivo…",
        "Emparelhar um dispositivo…",
        "Instrumentum copula…";
    Pair:
        "Pair",
        "Associer",
        "Vincular",
        "Asociați",
        "Parear",
        "Emparelhar",
        "Copula";
    PairingWaiting:
        "Opening a pairing code…",
        "Ouverture d’un code d’association…",
        "Abriendo un código de vinculación…",
        "Se deschide un cod de asociere…",
        "Abrindo um código de pareamento…",
        "A abrir um código de emparelhamento…",
        "Nota copulationis aperitur…";
    PairingCode:
        "Pairing code",
        "Code d’association",
        "Código de vinculación",
        "Cod de asociere",
        "Código de pareamento",
        "Código de emparelhamento",
        "Nota copulationis";
    PairingExpiresIn:
        "Single use, valid for {time} more",
        "Usage unique, valable encore {time}",
        "De un solo uso, válido durante {time} más",
        "De unică folosință, valabil încă {time}",
        "Uso único, válido por mais {time}",
        "Utilização única, válido durante mais {time}",
        "Semel utenda, valet adhuc {time}";
    PairingExpired:
        "This code has expired. Pair again for a new one.",
        "Ce code a expiré. Associez de nouveau pour en obtenir un autre.",
        "Este código ha caducado. Vuelve a vincular para obtener uno nuevo.",
        "Acest cod a expirat. Asociați din nou pentru unul nou.",
        "Este código expirou. Pareie de novo para obter outro.",
        "Este código expirou. Emparelhe de novo para obter outro.",
        "Haec nota exspiravit. Iterum copula ut novam accipias.";
    PairingFingerprint:
        "Server certificate (SHA-256)",
        "Certificat du serveur (SHA-256)",
        "Certificado del servidor (SHA-256)",
        "Certificatul serverului (SHA-256)",
        "Certificado do servidor (SHA-256)",
        "Certificado do servidor (SHA-256)",
        "Testimonium servitoris (SHA-256)";
    PairingAddress:
        "Address: {address}",
        "Adresse : {address}",
        "Dirección: {address}",
        "Adresă: {address}",
        "Endereço: {address}",
        "Endereço: {address}",
        "Inscriptio: {address}";
    PairingScanHint:
        "Scan the QR code with dail on the device, or enter the code there. The device must show the same fingerprint.",
        "Scannez le code QR avec dail sur l’appareil, ou saisissez-y le code. L’appareil doit afficher la même empreinte.",
        "Escanea el código QR con dail en el dispositivo, o escribe allí el código. El dispositivo debe mostrar la misma huella.",
        "Scanați codul QR cu dail pe dispozitiv sau introduceți codul acolo. Dispozitivul trebuie să arate aceeași amprentă.",
        "Escaneie o código QR com o dail no dispositivo, ou digite o código nele. O dispositivo deve mostrar a mesma impressão digital.",
        "Leia o código QR com o dail no dispositivo, ou introduza lá o código. O dispositivo tem de mostrar a mesma impressão digital.",
        "Notam QR cum dail in instrumento perlege, aut notam ibi inscribe. Instrumentum eandem impressionem ostendere debet.";
    PairingNoQr:
        "No QR code: this dail does not know an address the device can reach. Start dail serve with --advertise <host>.",
        "Pas de code QR : ce dail ne connaît aucune adresse que l’appareil peut joindre. Lancez dail serve avec --advertise <hôte>.",
        "Sin código QR: este dail no conoce ninguna dirección a la que llegue el dispositivo. Inicia dail serve con --advertise <host>.",
        "Fără cod QR: acest dail nu cunoaște nicio adresă la care dispozitivul să ajungă. Porniți dail serve cu --advertise <gazdă>.",
        "Sem código QR: este dail não conhece um endereço que o dispositivo alcance. Inicie o dail serve com --advertise <host>.",
        "Sem código QR: este dail não conhece um endereço que o dispositivo alcance. Inicie o dail serve com --advertise <anfitrião>.",
        "Nulla nota QR: hic dail nullam inscriptionem novit quam instrumentum attingat. Incipe dail serve cum --advertise <hospes>.";
    PairingFailed:
        "Pairing failed: {error}",
        "Échec de l’association : {error}",
        "La vinculación falló: {error}",
        "Asocierea a eșuat: {error}",
        "O pareamento falhou: {error}",
        "O emparelhamento falhou: {error}",
        "Copulatio defecit: {error}";
    PairAnother:
        "Pair another device",
        "Associer un autre appareil",
        "Vincular otro dispositivo",
        "Asociați alt dispozitiv",
        "Parear outro dispositivo",
        "Emparelhar outro dispositivo",
        "Aliud instrumentum copula";
    CopyCode:
        "Copy code",
        "Copier le code",
        "Copiar el código",
        "Copiați codul",
        "Copiar código",
        "Copiar código",
        "Notam exscribe";
    PairingCodeCopied:
        "Code copied.",
        "Code copié.",
        "Código copiado.",
        "Cod copiat.",
        "Código copiado.",
        "Código copiado.",
        "Nota exscripta.";
    PairFormFooter:
        "Tab: kind of device · Enter: pair · Esc: close",
        "Tab : type d’appareil · Entrée : associer · Échap : fermer",
        "Tab: tipo de dispositivo · Intro: vincular · Esc: cerrar",
        "Tab: tip de dispozitiv · Enter: asociați · Esc: închideți",
        "Tab: tipo de dispositivo · Enter: parear · Esc: fechar",
        "Tab: tipo de dispositivo · Enter: emparelhar · Esc: fechar",
        "Tab: genus instrumenti · Enter: copula · Esc: claude";
    PairOpenedFooter:
        "n: pair another device · Esc: close",
        "n : associer un autre appareil · Échap : fermer",
        "n: vincular otro dispositivo · Esc: cerrar",
        "n: asociați alt dispozitiv · Esc: închideți",
        "n: parear outro dispositivo · Esc: fechar",
        "n: emparelhar outro dispositivo · Esc: fechar",
        "n: aliud instrumentum copula · Esc: claude";
    PairingNoLink:
        "No QR code: {error}",
        "Pas de code QR : {error}",
        "Sin código QR: {error}",
        "Fără cod QR: {error}",
        "Sem código QR: {error}",
        "Sem código QR: {error}",
        "Nulla nota QR: {error}";
    PairingEnlarge:
        "Enlarge the terminal to show the QR code.",
        "Agrandissez le terminal pour afficher le code QR.",
        "Amplía la terminal para mostrar el código QR.",
        "Măriți terminalul pentru a afișa codul QR.",
        "Aumente o terminal para mostrar o código QR.",
        "Aumente o terminal para mostrar o código QR.",
        "Terminale amplifica ut nota QR ostendatur.";
    ConnectToServerInstead:
        "Connect to a dail server instead",
        "Se connecter plutôt à un serveur dail",
        "Conectarte a un servidor dail en su lugar",
        "Conectați-vă în schimb la un server dail",
        "Conectar a um servidor dail em vez disso",
        "Ligar antes a um servidor dail",
        "Potius ad servitorem dail conecte";
    ConnectToServerDetail:
        "Your accounts stay on a dail server you run (dail serve); this device mirrors its mail.",
        "Vos comptes restent sur un serveur dail que vous faites tourner (dail serve) ; cet appareil reflète son courrier.",
        "Tus cuentas se quedan en un servidor dail que tú ejecutas (dail serve); este dispositivo refleja su correo.",
        "Conturile dvs. rămân pe un server dail pe care îl rulați (dail serve); acest dispozitiv îi oglindește e-mailurile.",
        "Suas contas ficam em um servidor dail que você executa (dail serve); este dispositivo espelha os e-mails dele.",
        "As suas contas ficam num servidor dail que executa (dail serve); este dispositivo espelha o correio dele.",
        "Rationes tuae in servitore dail tuo (dail serve) manent; hoc instrumentum epistulas eius imitatur.";
    ConnectToServer:
        "Connect to a dail server",
        "Se connecter à un serveur dail",
        "Conectarte a un servidor dail",
        "Conectați-vă la un server dail",
        "Conectar a um servidor dail",
        "Ligar a um servidor dail",
        "Ad servitorem dail conecte";
    ConnectServerLabel:
        "Server",
        "Serveur",
        "Servidor",
        "Server",
        "Servidor",
        "Servidor",
        "Servitor";
    ConnectAddressHint:
        "host or host:port (port 7443 if none), or paste a dail://pair link",
        "hôte ou hôte:port (port 7443 par défaut), ou collez un lien dail://pair",
        "host o host:puerto (7443 si no hay puerto), o pega un enlace dail://pair",
        "gazdă sau gazdă:port (portul 7443 implicit) sau lipiți un link dail://pair",
        "host ou host:porta (porta 7443 se não houver), ou cole um link dail://pair",
        "anfitrião ou anfitrião:porta (porta 7443 por omissão), ou cole uma ligação dail://pair",
        "hospes aut hospes:porta (porta 7443 nisi datur), aut nexum dail://pair insere";
    ConnectExpectedHostPort:
        "Enter the server as host or host:port.",
        "Saisissez le serveur sous la forme hôte ou hôte:port.",
        "Escribe el servidor como host o host:puerto.",
        "Introduceți serverul ca gazdă sau gazdă:port.",
        "Digite o servidor como host ou host:porta.",
        "Introduza o servidor como anfitrião ou anfitrião:porta.",
        "Servitorem ut hospes aut hospes:porta inscribe.";
    ConnectBadPort:
        "The port must be 1 to 65535.",
        "Le port doit être compris entre 1 et 65535.",
        "El puerto debe estar entre 1 y 65535.",
        "Portul trebuie să fie între 1 și 65535.",
        "A porta deve estar entre 1 e 65535.",
        "A porta tem de estar entre 1 e 65535.",
        "Porta inter 1 et 65535 esse debet.";
    ConnectBadHost:
        "The host is not a DNS name or an IP address.",
        "L’hôte n’est ni un nom DNS ni une adresse IP.",
        "El host no es un nombre DNS ni una dirección IP.",
        "Gazda nu este un nume DNS sau o adresă IP.",
        "O host não é um nome DNS nem um endereço IP.",
        "O anfitrião não é um nome DNS nem um endereço IP.",
        "Hospes neque nomen DNS neque inscriptio IP est.";
    ConnectLinkInvalid:
        "Not a usable pairing link: {error}",
        "Lien d’association inutilisable : {error}",
        "Enlace de vinculación no válido: {error}",
        "Link de asociere inutilizabil: {error}",
        "Link de pareamento inválido: {error}",
        "Ligação de emparelhamento inválida: {error}",
        "Nexus copulationis inutilis: {error}";
    ConnectLinkRead:
        "From the pairing link: {server} and its code. The server must show the link's fingerprint.",
        "Depuis le lien d’association : {server} et son code. Le serveur doit afficher l’empreinte du lien.",
        "Del enlace de vinculación: {server} y su código. El servidor debe mostrar la huella del enlace.",
        "Din linkul de asociere: {server} și codul său. Serverul trebuie să afișeze amprenta din link.",
        "Do link de pareamento: {server} e o código. O servidor deve mostrar a impressão digital do link.",
        "Da ligação de emparelhamento: {server} e o código. O servidor tem de mostrar a impressão digital da ligação.",
        "Ex nexu copulationis: {server} et nota eius. Servitor vestigium nexus monstrare debet.";
    Connect:
        "Connect",
        "Se connecter",
        "Conectar",
        "Conectați",
        "Conectar",
        "Ligar",
        "Conecte";
    ConnectProbing:
        "Contacting {server}…",
        "Contact de {server}…",
        "Contactando con {server}…",
        "Se contactează {server}…",
        "Contatando {server}…",
        "A contactar {server}…",
        "Ad {server} acceditur…";
    ConnectCompare:
        "{server} shows this certificate fingerprint (SHA-256). Pair only if it is the one dail serve or Pair a device… shows on the server.",
        "{server} présente cette empreinte de certificat (SHA-256). N’associez que si c’est celle que dail serve ou Associer un appareil… affiche sur le serveur.",
        "{server} muestra esta huella de certificado (SHA-256). Vincula solo si es la que muestran dail serve o Vincular un dispositivo… en el servidor.",
        "{server} afișează această amprentă de certificat (SHA-256). Asociați doar dacă este cea pe care o arată dail serve sau Asociați un dispozitiv… pe server.",
        "{server} mostra esta impressão digital de certificado (SHA-256). Pareie só se for a que o dail serve ou Parear um dispositivo… mostra no servidor.",
        "{server} mostra esta impressão digital de certificado (SHA-256). Emparelhe só se for a que o dail serve ou Emparelhar um dispositivo… mostra no servidor.",
        "{server} hoc vestigium testimonii (SHA-256) monstrat. Copula solum si idem est quod dail serve aut Instrumentum copula… in servitore monstrat.";
    ConnectMatchesLink:
        "It matches the fingerprint of the pairing link.",
        "Elle correspond à l’empreinte du lien d’association.",
        "Coincide con la huella del enlace de vinculación.",
        "Se potrivește cu amprenta din linkul de asociere.",
        "Ela confere com a impressão digital do link de pareamento.",
        "Coincide com a impressão digital da ligação de emparelhamento.",
        "Cum vestigio nexus copulationis congruit.";
    ConnectMismatch:
        "{server} shows another fingerprint than the pairing link. Do not pair: another machine may be answering in its place.",
        "{server} présente une autre empreinte que le lien d’association. N’associez pas : une autre machine répond peut-être à sa place.",
        "{server} muestra una huella distinta de la del enlace de vinculación. No vincules: puede que otra máquina responda en su lugar.",
        "{server} afișează altă amprentă decât linkul de asociere. Nu asociați: poate răspunde altă mașină în locul lui.",
        "{server} mostra outra impressão digital que não a do link de pareamento. Não pareie: outra máquina pode estar respondendo no lugar dele.",
        "{server} mostra outra impressão digital que não a da ligação de emparelhamento. Não emparelhe: outra máquina pode estar a responder no lugar dele.",
        "{server} aliud vestigium quam nexus copulationis monstrat. Noli copulare: alia machina fortasse loco eius respondet.";
    ConnectLinkFingerprint:
        "In the pairing link",
        "Dans le lien d’association",
        "En el enlace de vinculación",
        "În linkul de asociere",
        "No link de pareamento",
        "Na ligação de emparelhamento",
        "In nexu copulationis";
    ConnectCodeHint:
        "The 8-character code from dail serve pair or Pair a device…",
        "Le code de 8 caractères de dail serve pair ou Associer un appareil…",
        "El código de 8 caracteres de dail serve pair o Vincular un dispositivo…",
        "Codul de 8 caractere din dail serve pair sau Asociați un dispozitiv…",
        "O código de 8 caracteres do dail serve pair ou Parear um dispositivo…",
        "O código de 8 caracteres do dail serve pair ou Emparelhar um dispositivo…",
        "Nota octo signorum ex dail serve pair aut Instrumentum copula…";
    ConnectBadCode:
        "The code is 8 characters, A–Z and 2–7.",
        "Le code fait 8 caractères, A–Z et 2–7.",
        "El código tiene 8 caracteres, A–Z y 2–7.",
        "Codul are 8 caractere, A–Z și 2–7.",
        "O código tem 8 caracteres, A–Z e 2–7.",
        "O código tem 8 caracteres, A–Z e 2–7.",
        "Nota octo signorum est, A–Z et 2–7.";
    ConnectPairing:
        "Pairing with {server}…",
        "Association avec {server}…",
        "Vinculando con {server}…",
        "Se asociază cu {server}…",
        "Pareando com {server}…",
        "A emparelhar com {server}…",
        "Cum {server} copulatur…";
    ConnectPaired:
        "Paired with {server}: its certificate is pinned and this device now mirrors its mail.",
        "Associé à {server} : son certificat est épinglé et cet appareil reflète désormais son courrier.",
        "Vinculado con {server}: su certificado está fijado y este dispositivo ahora refleja su correo.",
        "Asociat cu {server}: certificatul său este fixat, iar acest dispozitiv îi oglindește acum e-mailurile.",
        "Pareado com {server}: o certificado dele está fixado e este dispositivo agora espelha os e-mails dele.",
        "Emparelhado com {server}: o certificado dele está fixado e este dispositivo espelha agora o correio dele.",
        "Cum {server} copulatum: testimonium eius fixum est et hoc instrumentum nunc epistulas eius imitatur.";
    ConnectFailed:
        "Could not reach the server: {error}",
        "Impossible de joindre le serveur : {error}",
        "No se pudo contactar con el servidor: {error}",
        "Serverul nu a putut fi contactat: {error}",
        "Não foi possível alcançar o servidor: {error}",
        "Não foi possível contactar o servidor: {error}",
        "Servitor attingi non potuit: {error}";
    TryAgain:
        "Try again",
        "Réessayer",
        "Intentar de nuevo",
        "Încercați din nou",
        "Tentar novamente",
        "Tentar novamente",
        "Iterum conare";
    AnotherCode:
        "Enter another code",
        "Saisir un autre code",
        "Escribir otro código",
        "Introduceți alt cod",
        "Digitar outro código",
        "Introduzir outro código",
        "Aliam notam inscribe";
    ConnectUseAnotherProfile:
        "This profile has mail accounts, so it cannot mirror a dail server. Start dail with another profile: dail --profile <name>.",
        "Ce profil a des comptes de messagerie : il ne peut pas refléter un serveur dail. Lancez dail avec un autre profil : dail --profile <nom>.",
        "Este perfil tiene cuentas de correo, así que no puede reflejar un servidor dail. Inicia dail con otro perfil: dail --profile <nombre>.",
        "Acest profil are conturi de e-mail, deci nu poate oglindi un server dail. Porniți dail cu alt profil: dail --profile <nume>.",
        "Este perfil tem contas de e-mail, então não pode espelhar um servidor dail. Inicie o dail com outro perfil: dail --profile <nome>.",
        "Este perfil tem contas de correio, por isso não pode espelhar um servidor dail. Inicie o dail com outro perfil: dail --profile <nome>.",
        "Hoc profilum rationes epistularum habet, ergo servitorem dail imitari non potest. Incipe dail cum alio profilo: dail --profile <nomen>.";
    PairLinkRefused:
        "This phone already has mail accounts or a dail server, so the pairing link was not used.",
        "Ce téléphone a déjà des comptes de messagerie ou un serveur dail : le lien d’association n’a pas été utilisé.",
        "Este teléfono ya tiene cuentas de correo o un servidor dail, así que no se usó el enlace de vinculación.",
        "Acest telefon are deja conturi de e-mail sau un server dail, așa că linkul de asociere nu a fost folosit.",
        "Este celular já tem contas de e-mail ou um servidor dail, então o link de pareamento não foi usado.",
        "Este telemóvel já tem contas de correio ou um servidor dail, por isso a ligação de emparelhamento não foi usada.",
        "Hoc telephonum iam rationes epistularum aut servitorem dail habet, ergo nexus copulationis non adhibitus est.";
    PairLinkBusy:
        "A pairing is already under way, so the pairing link was not used. Finish or cancel it first.",
        "Une association est déjà en cours : le lien d’association n’a pas été utilisé. Terminez-la ou annulez-la d’abord.",
        "Ya hay una vinculación en curso, así que no se usó el enlace de vinculación. Termínala o cancélala primero.",
        "O asociere este deja în curs, așa că linkul de asociere nu a fost folosit. Finalizați-o sau anulați-o mai întâi.",
        "Já há um pareamento em andamento, então o link de pareamento não foi usado. Conclua ou cancele esse pareamento primeiro.",
        "Já há um emparelhamento em curso, por isso a ligação de emparelhamento não foi usada. Conclua-o ou cancele-o primeiro.",
        "Copulatio iam agitur, ergo nexus copulationis non adhibitus est. Eam prius perfice aut rescinde.";
    PairLinkWarning:
        "A link opened this pairing with {server}. Only continue if you just scanned or opened this code from your own dail server.",
        "Un lien a ouvert cette association avec {server}. Ne continuez que si vous venez de scanner ou d’ouvrir ce code depuis votre propre serveur dail.",
        "Un enlace abrió esta vinculación con {server}. Continúa solo si acabas de escanear o abrir este código desde tu propio servidor dail.",
        "Un link a deschis această asociere cu {server}. Continuați doar dacă tocmai ați scanat sau ați deschis acest cod de pe propriul server dail.",
        "Um link abriu este pareamento com {server}. Só continue se você acabou de escanear ou abrir este código do seu próprio servidor dail.",
        "Uma ligação abriu este emparelhamento com {server}. Só continue se acabou de ler ou abrir este código a partir do seu próprio servidor dail.",
        "Nexus hanc copulationem cum {server} aperuit. Perge solum si hanc notam modo ex tuo servitore dail legisti aut aperuisti.";
    ConnectAddressFooter:
        "Enter connect · Esc back",
        "Entrée se connecter · Échap retour",
        "Intro conectar · Esc atrás",
        "Enter conectați · Esc înapoi",
        "Enter conectar · Esc voltar",
        "Enter ligar · Esc retroceder",
        "Enter conecte · Esc retro";
    ConnectCodeFooter:
        "type the code · Enter pair · Esc back",
        "saisissez le code · Entrée associer · Échap retour",
        "escribe el código · Intro vincular · Esc atrás",
        "introduceți codul · Enter asociați · Esc înapoi",
        "digite o código · Enter parear · Esc voltar",
        "introduza o código · Enter emparelhar · Esc retroceder",
        "notam inscribe · Enter copula · Esc retro";
    ConnectWaitFooter:
        "Esc cancel",
        "Échap annuler",
        "Esc cancelar",
        "Esc anulați",
        "Esc cancelar",
        "Esc cancelar",
        "Esc rescinde";
    ConnectRetryFooter:
        "Enter try again · Esc back",
        "Entrée réessayer · Échap retour",
        "Intro reintentar · Esc atrás",
        "Enter reîncercați · Esc înapoi",
        "Enter tentar novamente · Esc voltar",
        "Enter tentar novamente · Esc retroceder",
        "Enter iterum conare · Esc retro";
    ConnectDoneFooter:
        "Enter close",
        "Entrée fermer",
        "Intro cerrar",
        "Enter închideți",
        "Enter fechar",
        "Enter fechar",
        "Enter claude";
    LinkOnline:
        "Online",
        "En ligne",
        "En línea",
        "Online",
        "Online",
        "Online",
        "Conexum";
    LinkConnecting:
        "Connecting…",
        "Connexion…",
        "Conectando…",
        "Se conectează…",
        "Conectando…",
        "A ligar…",
        "Conectitur…";
    LinkOffline:
        "Offline",
        "Hors ligne",
        "Sin conexión",
        "Offline",
        "Offline",
        "Offline",
        "Sine conexione";
    LinkNotPaired:
        "Not paired yet",
        "Pas encore associé",
        "Aún no vinculado",
        "Încă neasociat",
        "Ainda não pareado",
        "Ainda não emparelhado",
        "Nondum copulatum";
    LinkQueued:
        "{status} · {n} queued",
        "{status} · {n} en file",
        "{status} · {n} en cola",
        "{status} · {n} în coadă",
        "{status} · {n} na fila",
        "{status} · {n} em fila",
        "{status} · {n} in ordine";
    DailServer:
        "dail server",
        "Serveur dail",
        "Servidor dail",
        "Server dail",
        "Servidor dail",
        "Servidor dail",
        "Servitor dail";
    LinkStatusLabel:
        "Connection",
        "Connexion",
        "Conexión",
        "Conexiune",
        "Conexão",
        "Ligação",
        "Conexio";
    LinkRevision:
        "Revision",
        "Révision",
        "Revisión",
        "Revizie",
        "Revisão",
        "Revisão",
        "Recensio";
    LinkQueuedLabel:
        "Waiting to be sent",
        "En attente d’envoi",
        "Pendientes de envío",
        "În așteptarea trimiterii",
        "Aguardando envio",
        "A aguardar envio",
        "Mittenda exspectant";
    RetryRefused:
        "Retry refused changes",
        "Renvoyer les modifications refusées",
        "Reintentar los cambios rechazados",
        "Reîncercați modificările refuzate",
        "Tentar de novo as alterações recusadas",
        "Tentar de novo as alterações recusadas",
        "Mutationes recusatas iterum mitte";
    RetryAll:
        "Retry all",
        "Tout réessayer",
        "Reintentar todo",
        "Reîncercați tot",
        "Tentar tudo de novo",
        "Tentar tudo de novo",
        "Omnia iterum conare";
    DisconnectDots:
        "Disconnect…",
        "Se déconnecter…",
        "Desconectar…",
        "Deconectați…",
        "Desconectar…",
        "Desligar…",
        "Disiunge…";
    RefusedOffline:
        "Refused by the server: {command}",
        "Refusé par le serveur : {command}",
        "Rechazado por el servidor: {command}",
        "Refuzat de server: {command}",
        "Recusado pelo servidor: {command}",
        "Recusado pelo servidor: {command}",
        "A servitore recusatum: {command}";
    RefusedOfflineHint:
        "Made while offline; the server refused it when it arrived. Retry all sends every refused change again; Dismiss drops this one.",
        "Faite hors ligne ; le serveur l’a refusée à son arrivée. Tout réessayer renvoie toutes les modifications refusées ; Ignorer abandonne celle-ci.",
        "Hecho sin conexión; el servidor lo rechazó al llegar. Reintentar todo vuelve a enviar todos los cambios rechazados; Descartar elimina este.",
        "Făcută offline; serverul a refuzat-o la sosire. Reîncercați tot retrimite toate modificările refuzate; Închideți renunță la aceasta.",
        "Feita offline; o servidor a recusou quando chegou. Tentar tudo de novo reenvia todas as alterações recusadas; Dispensar descarta esta.",
        "Feita offline; o servidor recusou-a quando chegou. Tentar tudo de novo reenvia todas as alterações recusadas; Ignorar descarta esta.",
        "Sine conexione facta; servitor eam advenientem recusavit. Omnia iterum conare omnes mutationes recusatas rursus mittit; Dimitte hanc abicit.";
    RefusedAt:
        "refused {when}",
        "refusé {when}",
        "rechazado {when}",
        "refuzat {when}",
        "recusado {when}",
        "recusado {when}",
        "recusatum {when}";
    Devices:
        "Devices",
        "Appareils",
        "Dispositivos",
        "Dispozitive",
        "Dispositivos",
        "Dispositivos",
        "Instrumenta";
    DevicesHint:
        "Devices paired with this dail server. Revoking one ends its access at once.",
        "Appareils associés à ce serveur dail. En révoquer un met fin à son accès immédiatement.",
        "Dispositivos vinculados con este servidor dail. Revocar uno termina su acceso al momento.",
        "Dispozitive asociate cu acest server dail. Revocarea unuia îi încheie imediat accesul.",
        "Dispositivos pareados com este servidor dail. Revogar um encerra o acesso dele na hora.",
        "Dispositivos emparelhados com este servidor dail. Revogar um termina de imediato o acesso dele.",
        "Instrumenta cum hoc servitore dail copulata. Unum revocare accessum eius statim finit.";
    NoDevices:
        "No device is paired yet.",
        "Aucun appareil associé pour l’instant.",
        "Aún no hay ningún dispositivo vinculado.",
        "Niciun dispozitiv asociat încă.",
        "Nenhum dispositivo pareado ainda.",
        "Ainda não há dispositivos emparelhados.",
        "Nullum instrumentum adhuc copulatum.";
    DevicePaired:
        "paired {date}",
        "associé : {date}",
        "vinculado: {date}",
        "asociat: {date}",
        "pareado: {date}",
        "emparelhado: {date}",
        "copulatum: {date}";
    DeviceSeen:
        "last seen {age}",
        "dernière connexion {age}",
        "última conexión {age}",
        "ultima conectare {age}",
        "última conexão {age}",
        "última ligação {age}",
        "ultima conexio {age}";
    DeviceNeverSeen:
        "never connected",
        "jamais connecté",
        "nunca conectado",
        "niciodată conectat",
        "nunca conectado",
        "nunca ligado",
        "numquam conexum";
    RevokeDots:
        "Revoke…",
        "Révoquer…",
        "Revocar…",
        "Revocați…",
        "Revogar…",
        "Revogar…",
        "Revoca…";
    FixSettingsAccounts:
        " (settings › Accounts)",
        " (paramètres › Comptes)",
        " (ajustes › Cuentas)",
        " (setări › Conturi)",
        " (configurações › Contas)",
        " (definições › Contas)",
        " (optiones › Rationes)";
    LinkWaitingForMail:
        "Linked to {server}. Its mail shows up here once this device has connected.",
        "Relié à {server}. Son courrier apparaît ici une fois cet appareil connecté.",
        "Vinculado con {server}. Su correo aparece aquí cuando este dispositivo se conecte.",
        "Legat la {server}. E-mailurile sale apar aici după ce acest dispozitiv se conectează.",
        "Ligado a {server}. Os e-mails dele aparecem aqui quando este dispositivo se conectar.",
        "Ligado a {server}. O correio dele aparece aqui quando este dispositivo se ligar.",
        "Cum {server} coniunctum. Epistulae eius hic apparent cum hoc instrumentum conexum erit.";
}

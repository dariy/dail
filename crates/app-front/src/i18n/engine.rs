//! The engine's texts in a language. `app-core` stays in English: its errors and undo labels
//! are part of the JSON that agents read. The front-ends translate them here, for display.
//! An English text unknown here (a server's message, a new label) is shown as it is.
//! Agents read English: the CLI and MCP outputs are not translated.

use app_core::{CommandName, CoreError};

use super::{Lang, commands, pseudo};

/// A text in French, Spanish and Romanian, in [`column`] order.
type Tr = [&'static str; 3];

/// English reasons of `error_002_invalid_argument` and similar errors, with their translations.
const REASONS: &[(&str, Tr)] = &[
    (
        "there is no folder for the draft's attachments",
        [
            "il n’y a pas de dossier pour les pièces jointes du brouillon",
            "no hay carpeta para los adjuntos del borrador",
            "nu există un dosar pentru atașamentele ciornei",
        ],
    ),
    (
        "expected an address like sales@example.org",
        [
            "adresse attendue, par exemple sales@example.org",
            "se esperaba una dirección como ventas@example.org",
            "se aștepta o adresă precum vanzari@example.org",
        ],
    ),
    (
        "the body is not downloaded yet (open_thread downloads it)",
        [
            "le corps n’est pas encore téléchargé (open_thread le télécharge)",
            "el cuerpo aún no se ha descargado (open_thread lo descarga)",
            "corpul nu este descărcat încă (open_thread îl descarcă)",
        ],
    ),
    (
        "expected the absolute path of a folder",
        [
            "chemin absolu d’un dossier attendu",
            "se esperaba la ruta absoluta de una carpeta",
            "se aștepta calea absolută a unui dosar",
        ],
    ),
    (
        "give a mode, a light theme or a dark theme",
        [
            "indiquez un mode, un thème clair ou un thème sombre",
            "indica un modo, un tema claro o un tema oscuro",
            "indicați un mod, o temă luminoasă sau o temă întunecată",
        ],
    ),
    (
        "nothing to undo",
        ["rien à annuler", "nada que deshacer", "nimic de anulat"],
    ),
    (
        "nothing to redo",
        ["rien à rétablir", "nada que rehacer", "nimic de refăcut"],
    ),
    (
        "session closed",
        ["session fermée", "sesión cerrada", "sesiune închisă"],
    ),
    (
        "dail stopped while reading this message, so it is not read again",
        [
            "dail s’est arrêté en lisant ce message, il n’est donc plus lu",
            "dail se detuvo al leer este mensaje, así que no se vuelve a leer",
            "dail s-a oprit în timp ce citea acest mesaj, așa că nu îl mai citește",
        ],
    ),
    (
        "dail hit an internal error; the log has the details",
        [
            "dail a rencontré une erreur interne ; le journal contient les détails",
            "dail sufrió un error interno; el registro tiene los detalles",
            "dail a întâmpinat o eroare internă; jurnalul conține detaliile",
        ],
    ),
    (
        "the session is closed",
        [
            "la session est fermée",
            "la sesión está cerrada",
            "sesiunea este închisă",
        ],
    ),
    (
        "dail's engine stopped; restart the app",
        [
            "le moteur de dail s’est arrêté ; redémarrez l’application",
            "el motor de dail se detuvo; reinicia la aplicación",
            "motorul dail s-a oprit; reporniți aplicația",
        ],
    ),
    (
        "not a PNG, JPEG, GIF or WebP image",
        [
            "pas une image PNG, JPEG, GIF ou WebP",
            "no es una imagen PNG, JPEG, GIF ni WebP",
            "nu este o imagine PNG, JPEG, GIF sau WebP",
        ],
    ),
    (
        "this build trusts no update signing key",
        [
            "cette version ne fait confiance à aucune clé de signature de mise à jour",
            "esta compilación no confía en ninguna clave de firma de actualizaciones",
            "această versiune nu are încredere în nicio cheie de semnare a actualizărilor",
        ],
    ),
    (
        "the manifest signature does not verify",
        [
            "la signature du manifeste n’est pas valide",
            "la firma del manifiesto no es válida",
            "semnătura manifestului nu este validă",
        ],
    ),
    (
        "the download's sha256 does not match the manifest",
        [
            "le sha256 du téléchargement ne correspond pas au manifeste",
            "el sha256 de la descarga no coincide con el manifiesto",
            "sha256 al descărcării nu corespunde manifestului",
        ],
    ),
    (
        "no data directory found for profiles (is $HOME set?)",
        [
            "aucun dossier de données trouvé pour les profils ($HOME est-il défini ?)",
            "no se encontró una carpeta de datos para los perfiles (¿está definido $HOME?)",
            "nu s-a găsit un dosar de date pentru profiluri (este setat $HOME?)",
        ],
    ),
    (
        "the page size is out of range",
        [
            "la taille de la page est hors limites",
            "el tamaño de página está fuera de rango",
            "dimensiunea paginii este în afara limitelor",
        ],
    ),
    (
        "the tile is out of range",
        [
            "la tuile est hors limites",
            "el mosaico está fuera de rango",
            "dala este în afara limitelor",
        ],
    ),
    (
        "the PDF renderer failed",
        [
            "le moteur de rendu PDF a échoué",
            "falló el motor de PDF",
            "motorul de randare PDF a eșuat",
        ],
    ),
    (
        "it is being sent and can't be stopped any more",
        [
            "il est en cours d’envoi et ne peut plus être arrêté",
            "se está enviando y ya no se puede detener",
            "se trimite și nu mai poate fi oprit",
        ],
    ),
    (
        "no send is scheduled",
        [
            "aucun envoi n’est programmé",
            "no hay ningún envío programado",
            "nu este programată nicio trimitere",
        ],
    ),
    (
        "it is being sent",
        ["il est en cours d’envoi", "se está enviando", "se trimite"],
    ),
    (
        "a send is scheduled; run cancel_send first",
        [
            "un envoi est programmé ; lancez d’abord cancel_send",
            "hay un envío programado; ejecuta antes cancel_send",
            "este programată o trimitere; rulați mai întâi cancel_send",
        ],
    ),
    (
        "it has no recipients; set to, cc or bcc with update_draft",
        [
            "il n’a pas de destinataire ; indiquez to, cc ou bcc avec update_draft",
            "no tiene destinatarios; indica to, cc o bcc con update_draft",
            "nu are destinatari; setați to, cc sau bcc cu update_draft",
        ],
    ),
    (
        "no thread is open (open_thread)",
        [
            "aucune discussion n’est ouverte (open_thread)",
            "no hay ninguna conversación abierta (open_thread)",
            "nu este deschisă nicio conversație (open_thread)",
        ],
    ),
    (
        "no draft is open (open_composer)",
        [
            "aucun brouillon n’est ouvert (open_composer)",
            "no hay ningún borrador abierto (open_composer)",
            "nu este deschisă nicio ciornă (open_composer)",
        ],
    ),
    (
        "the pane is not shown",
        [
            "le volet n’est pas affiché",
            "el panel no se muestra",
            "panoul nu este afișat",
        ],
    ),
    (
        "must be a message ID without spaces, line breaks or control characters",
        [
            "doit être un identifiant de message sans espace, saut de ligne ni caractère de contrôle",
            "debe ser un ID de mensaje sin espacios, saltos de línea ni caracteres de control",
            "trebuie să fie un ID de mesaj fără spații, sfârșituri de rând sau caractere de control",
        ],
    ),
    (
        "empty phrase; remove the quotes",
        [
            "expression vide ; retirez les guillemets",
            "frase vacía; quita las comillas",
            "expresie goală; eliminați ghilimelele",
        ],
    ),
    (
        "unterminated quote; add a closing `\"`",
        [
            "guillemet non fermé ; ajoutez un `\"` final",
            "comillas sin cerrar; añade un `\"` de cierre",
            "ghilimele neînchise; adăugați un `\"` de închidere",
        ],
    ),
    (
        "expected a space after the closing `\"`",
        [
            "espace attendu après le `\"` final",
            "se esperaba un espacio tras el `\"` de cierre",
            "se aștepta un spațiu după `\"` de închidere",
        ],
    ),
    (
        "give `ids` (threads), `messages` or both",
        [
            "indiquez `ids` (discussions), `messages` ou les deux",
            "indica `ids` (conversaciones), `messages` o ambos",
            "indicați `ids` (conversații), `messages` sau ambele",
        ],
    ),
    (
        "the mailbox belongs to another account than the messages",
        [
            "le dossier appartient à un autre compte que les messages",
            "la carpeta pertenece a otra cuenta que los mensajes",
            "dosarul aparține altui cont decât mesajele",
        ],
    ),
    (
        "the op is running on the server; discard it after it failed",
        [
            "l’opération est en cours sur le serveur ; abandonnez-la après son échec",
            "la operación se está ejecutando en el servidor; descártala cuando falle",
            "operațiunea rulează pe server; renunțați la ea după ce eșuează",
        ],
    ),
    (
        "expected an email address like news@example.org",
        [
            "adresse attendue, par exemple news@example.org",
            "se esperaba una dirección como news@example.org",
            "se aștepta o adresă precum news@example.org",
        ],
    ),
    (
        "no conversation is open (open_thread)",
        [
            "aucune discussion n’est ouverte (open_thread)",
            "no hay ninguna conversación abierta (open_thread)",
            "nu este deschisă nicio conversație (open_thread)",
        ],
    ),
    (
        "give either id or account",
        [
            "indiquez id ou account",
            "indica id o account",
            "indicați id sau account",
        ],
    ),
    (
        "is empty; give at least one term, e.g. `is:unread`",
        [
            "est vide ; donnez au moins un terme, par exemple `is:unread`",
            "está vacía; indica al menos un término, p. ej. `is:unread`",
            "este goală; indicați cel puțin un termen, de ex. `is:unread`",
        ],
    ),
    (
        "the inbox can't be renamed",
        [
            "la boîte de réception ne peut pas être renommée",
            "no se puede cambiar el nombre de la bandeja de entrada",
            "căsuța de intrare nu poate fi redenumită",
        ],
    ),
    (
        "the inbox can't be deleted",
        [
            "la boîte de réception ne peut pas être supprimée",
            "no se puede eliminar la bandeja de entrada",
            "căsuța de intrare nu poate fi ștearsă",
        ],
    ),
    (
        "that is its name already",
        [
            "c’est déjà son nom",
            "ya se llama así",
            "acesta este deja numele său",
        ],
    ),
    (
        "a folder can't move into itself",
        [
            "un dossier ne peut pas être déplacé dans lui-même",
            "una carpeta no se puede mover dentro de sí misma",
            "un dosar nu poate fi mutat în el însuși",
        ],
    ),
    (
        "delete its subfolders first",
        [
            "supprimez d’abord ses sous-dossiers",
            "elimina antes sus subcarpetas",
            "ștergeți mai întâi subdosarele sale",
        ],
    ),
    (
        "must not be empty",
        [
            "ne doit pas être vide",
            "no puede estar vacío",
            "nu trebuie să fie gol",
        ],
    ),
    (
        "nothing to update: give display_name, imap or smtp",
        [
            "rien à modifier : indiquez display_name, imap ou smtp",
            "nada que cambiar: indica display_name, imap o smtp",
            "nimic de modificat: indicați display_name, imap sau smtp",
        ],
    ),
    (
        "expected an address like name@example.com",
        [
            "adresse attendue, par exemple nom@example.com",
            "se esperaba una dirección como nombre@example.com",
            "se aștepta o adresă precum nume@example.com",
        ],
    ),
    (
        "expected an address like me@example.org",
        [
            "adresse attendue, par exemple moi@example.org",
            "se esperaba una dirección como yo@example.org",
            "se aștepta o adresă precum eu@example.org",
        ],
    ),
    (
        "must be non-empty and contain no control characters",
        [
            "ne doit être ni vide ni contenir de caractères de contrôle",
            "no puede estar vacío ni contener caracteres de control",
            "nu trebuie să fie gol și nici să conțină caractere de control",
        ],
    ),
    (
        "host must be a DNS name or IP address",
        [
            "l’hôte doit être un nom DNS ou une adresse IP",
            "el host debe ser un nombre DNS o una dirección IP",
            "gazda trebuie să fie un nume DNS sau o adresă IP",
        ],
    ),
    (
        "port must be between 1 and 65535",
        [
            "le port doit être compris entre 1 et 65535",
            "el puerto debe estar entre 1 y 65535",
            "portul trebuie să fie între 1 și 65535",
        ],
    ),
    (
        "this build cannot update itself (the runtime reported no version)",
        [
            "cette version ne peut pas se mettre à jour elle-même",
            "esta compilación no puede actualizarse sola",
            "această versiune nu se poate actualiza singură",
        ],
    ),
    (
        "no update is available; run check_for_updates first",
        [
            "aucune mise à jour disponible ; lancez d’abord check_for_updates",
            "no hay ninguna actualización disponible; ejecuta antes check_for_updates",
            "nu este disponibilă nicio actualizare; rulați mai întâi check_for_updates",
        ],
    ),
    (
        "no update is downloaded; run download_update first",
        [
            "aucune mise à jour téléchargée ; lancez d’abord download_update",
            "no hay ninguna actualización descargada; ejecuta antes download_update",
            "nu este descărcată nicio actualizare; rulați mai întâi download_update",
        ],
    ),
    (
        "the runtime reported no install kind",
        [
            "type d’installation inconnu",
            "tipo de instalación desconocido",
            "tip de instalare necunoscut",
        ],
    ),
    (
        "expected a version like 0.2.0, or null",
        [
            "version attendue, par exemple 0.2.0, ou null",
            "se esperaba una versión como 0.2.0, o null",
            "se aștepta o versiune precum 0.2.0 sau null",
        ],
    ),
    (
        "expected 1 to 720 hours",
        [
            "de 1 à 720 heures attendues",
            "se esperaban de 1 a 720 horas",
            "se așteptau între 1 și 720 de ore",
        ],
    ),
    (
        "nothing to update: give to, cc, bcc, subject or body",
        [
            "rien à modifier : indiquez to, cc, bcc, subject ou body",
            "nada que cambiar: indica to, cc, bcc, subject o body",
            "nimic de modificat: indicați to, cc, bcc, subject sau body",
        ],
    ),
    (
        "must not contain line breaks or control characters",
        [
            "ne doit contenir ni saut de ligne ni caractère de contrôle",
            "no puede contener saltos de línea ni caracteres de control",
            "nu trebuie să conțină sfârșituri de rând sau caractere de control",
        ],
    ),
    (
        "must not contain NUL characters",
        [
            "ne doit pas contenir de caractère NUL",
            "no puede contener caracteres NUL",
            "nu trebuie să conțină caractere NUL",
        ],
    ),
    (
        "must be an absolute file path",
        [
            "doit être un chemin de fichier absolu",
            "debe ser una ruta de archivo absoluta",
            "trebuie să fie o cale absolută de fișier",
        ],
    ),
    (
        "must name a file, not a directory",
        [
            "doit désigner un fichier, pas un répertoire",
            "debe indicar un archivo, no un directorio",
            "trebuie să indice un fișier, nu un director",
        ],
    ),
    (
        "expected a SHA-256 fingerprint: 64 hex digits",
        [
            "empreinte SHA-256 attendue : 64 chiffres hexadécimaux",
            "se esperaba una huella SHA-256: 64 dígitos hexadecimales",
            "se aștepta o amprentă SHA-256: 64 de cifre hexazecimale",
        ],
    ),
    (
        "the account trusts this certificate already",
        [
            "le compte fait déjà confiance à ce certificat",
            "la cuenta ya confía en este certificado",
            "contul acceptă deja acest certificat",
        ],
    ),
];

/// Reasons with variable parts: each `{}` of the English is a value, kept in order in each translation.
const PATTERNS: &[(&str, Tr)] = &[
    (
        "{} expects a non-negative integer",
        [
            "{} attend un entier positif ou nul",
            "{} espera un entero no negativo",
            "{} așteaptă un număr întreg nenegativ",
        ],
    ),
    (
        "at most {} bytes",
        [
            "{} octets au maximum",
            "{} bytes como máximo",
            "cel mult {} octeți",
        ],
    ),
    (
        "at most {} parts",
        [
            "{} parties au maximum",
            "{} partes como máximo",
            "cel mult {} părți",
        ],
    ),
    (
        "at most {} keys",
        [
            "{} touches au maximum",
            "{} teclas como máximo",
            "cel mult {} taste",
        ],
    ),
    (
        "{} is not an address of the account",
        [
            "{} n’est pas une adresse du compte",
            "{} no es una dirección de la cuenta",
            "{} nu este o adresă a contului",
        ],
    ),
    (
        "no command is named {}",
        [
            "aucune commande ne s’appelle {}",
            "ningún comando se llama {}",
            "nicio comandă nu se numește {}",
        ],
    ),
    (
        "not a key: {}",
        [
            "pas une touche : {}",
            "no es una tecla: {}",
            "nu este o tastă: {}",
        ],
    ),
    (
        "invalid command: {}",
        [
            "commande non valide : {}",
            "comando no válido: {}",
            "comandă nevalidă: {}",
        ],
    ),
    (
        "invalid argument `{}`: {}",
        [
            "argument `{}` non valide : {}",
            "argumento `{}` no válido: {}",
            "argument `{}` nevalid: {}",
        ],
    ),
    (
        "invalid replay record on line {}: {}",
        [
            "enregistrement non valide à la ligne {} : {}",
            "registro no válido en la línea {}: {}",
            "înregistrare nevalidă la rândul {}: {}",
        ],
    ),
    (
        "{} {} not found",
        [
            "{} {} introuvable",
            "{} {}: no se encontró",
            "{} {}: negăsit",
        ],
    ),
    (
        "{} {} already exists",
        [
            "{} {} existe déjà",
            "{} {}: ya existe",
            "{} {}: există deja",
        ],
    ),
    (
        "{} is irreversible; repeat it with `confirm: true`",
        [
            "{} est irréversible ; répétez avec `confirm: true`",
            "{} es irreversible; repítelo con `confirm: true`",
            "{} este ireversibil; repetați cu `confirm: true`",
        ],
    ),
    (
        "account {} has no {} mailbox",
        [
            "le compte {} n’a pas de dossier {}",
            "la cuenta {} no tiene carpeta {}",
            "contul {} nu are dosar {}",
        ],
    ),
    (
        "{} can't be run from {}; the user has to do it in the GUI or the CLI",
        [
            "{} ne peut pas être lancé depuis {} ; l’utilisateur doit le faire dans l’interface graphique ou la ligne de commande",
            "{} no se puede ejecutar desde {}; el usuario debe hacerlo en la interfaz gráfica o en la línea de comandos",
            "{} nu poate fi rulat din {}; utilizatorul trebuie să o facă în interfața grafică sau în linia de comandă",
        ],
    ),
    (
        "draft {} can't be sent: {}",
        [
            "le brouillon {} ne peut pas être envoyé : {}",
            "no se puede enviar el borrador {}: {}",
            "ciorna {} nu poate fi trimisă: {}",
        ],
    ),
    (
        "“{}” is larger than 25 MiB and cannot be attached",
        [
            "« {} » dépasse 25 Mio et ne peut pas être joint",
            "«{}» supera los 25 MiB y no se puede adjuntar",
            "„{}” depășește 25 MiB și nu poate fi atașat",
        ],
    ),
    (
        "the draft's attachments could not be written: {}",
        [
            "les pièces jointes du brouillon n’ont pas pu être écrites : {}",
            "no se pudieron escribir los adjuntos del borrador: {}",
            "atașamentele ciornei nu au putut fi scrise: {}",
        ],
    ),
    (
        "“{}” could not be copied: {}",
        [
            "« {} » n’a pas pu être copié : {}",
            "no se pudo copiar «{}»: {}",
            "„{}” nu a putut fi copiat: {}",
        ],
    ),
    (
        "invalid request: {}",
        [
            "requête non valide : {}",
            "solicitud no válida: {}",
            "cerere nevalidă: {}",
        ],
    ),
    (
        "runtime: {}",
        [
            "environnement d’exécution : {}",
            "entorno de ejecución: {}",
            "mediu de execuție: {}",
        ],
    ),
    (
        "the profile {} is in use by another dail process that does not accept connections; try again when it has finished",
        [
            "le profil {} est utilisé par un autre processus dail qui n’accepte pas de connexions ; réessayez quand il aura terminé",
            "el perfil {} lo usa otro proceso de dail que no acepta conexiones; reinténtalo cuando haya terminado",
            "profilul {} este folosit de alt proces dail care nu acceptă conexiuni; reîncercați după ce se termină",
        ],
    ),
    (
        "the profile {} is in use",
        [
            "le profil {} est utilisé",
            "el perfil {} está en uso",
            "profilul {} este în uz",
        ],
    ),
    (
        "the {} backend is not in this build",
        [
            "le moteur {} n’est pas dans cette version",
            "el motor {} no está en esta compilación",
            "motorul {} nu este inclus în această versiune",
        ],
    ),
    ("demo: {}", ["démo : {}", "demo: {}", "demo: {}"]),
    (
        "secrets: {}",
        ["secrets : {}", "secretos: {}", "secrete: {}"],
    ),
    (
        "no host with handle {}",
        [
            "aucun hôte avec l’identifiant {}",
            "ningún host con el identificador {}",
            "nicio gazdă cu identificatorul {}",
        ],
    ),
    (
        "debug bridge: {}",
        [
            "pont de débogage : {}",
            "puente de depuración: {}",
            "punte de depanare: {}",
        ],
    ),
    (
        "backend `{}` does not support {}",
        [
            "le moteur `{}` ne prend pas en charge {}",
            "el motor `{}` no admite {}",
            "motorul `{}` nu acceptă {}",
        ],
    ),
    (
        "keyring: {}",
        ["trousseau : {}", "llavero: {}", "portofel de chei: {}"],
    ),
    (
        "TLS setup failed: {}",
        [
            "échec de la configuration TLS : {}",
            "error al configurar TLS: {}",
            "configurarea TLS a eșuat: {}",
        ],
    ),
    (
        "IPC: {}",
        [
            "communication entre processus : {}",
            "comunicación entre procesos: {}",
            "comunicare între procese: {}",
        ],
    ),
    (
        "the manifest signature is malformed: {}",
        [
            "la signature du manifeste est mal formée : {}",
            "la firma del manifiesto está mal formada: {}",
            "semnătura manifestului este incorectă: {}",
        ],
    ),
    (
        "the manifest is malformed: {}",
        [
            "le manifeste est mal formé : {}",
            "el manifiesto está mal formado: {}",
            "manifestul este incorect: {}",
        ],
    ),
    (
        "the signature is for {}, the manifest says {}",
        [
            "la signature est pour {}, le manifeste indique {}",
            "la firma es para {}, el manifiesto dice {}",
            "semnătura este pentru {}, manifestul indică {}",
        ],
    ),
    (
        "the manifest's version {} is not newer than {}",
        [
            "la version {} du manifeste n’est pas plus récente que {}",
            "la versión {} del manifiesto no es más reciente que {}",
            "versiunea {} din manifest nu este mai nouă decât {}",
        ],
    ),
    (
        "the download has {} bytes, the manifest says {}",
        [
            "le téléchargement fait {} octets, le manifeste indique {}",
            "la descarga tiene {} bytes, el manifiesto dice {}",
            "descărcarea are {} octeți, manifestul indică {}",
        ],
    ),
    (
        "store unavailable: {}",
        [
            "stockage indisponible : {}",
            "almacenamiento no disponible: {}",
            "stocare indisponibilă: {}",
        ],
    ),
    (
        "invalid config {}: {}",
        [
            "configuration {} non valide : {}",
            "configuración {} no válida: {}",
            "configurație {} nevalidă: {}",
        ],
    ),
    (
        "invalid profile name `{}`: use letters, digits, `-` and `_` only",
        [
            "nom de profil `{}` non valide : utilisez uniquement des lettres, des chiffres, `-` et `_`",
            "nombre de perfil `{}` no válido: usa solo letras, dígitos, `-` y `_`",
            "nume de profil `{}` nevalid: folosiți doar litere, cifre, `-` și `_`",
        ],
    ),
    (
        "{}: schema version {} is newer than this dail supports ({})",
        [
            "{} : la version de schéma {} est plus récente que celle prise en charge par ce dail ({})",
            "{}: la versión de esquema {} es más reciente que la que admite este dail ({})",
            "{}: versiunea de schemă {} este mai nouă decât cea acceptată de acest dail ({})",
        ],
    ),
    (
        "invalid blob id `{}`",
        [
            "identifiant de blob `{}` non valide",
            "identificador de blob `{}` no válido",
            "identificator de blob `{}` nevalid",
        ],
    ),
    (
        "the PDF is larger than {} bytes",
        [
            "le PDF dépasse {} octets",
            "el PDF supera los {} bytes",
            "PDF-ul depășește {} octeți",
        ],
    ),
    (
        "the PDF has more than {} pages",
        [
            "le PDF a plus de {} pages",
            "el PDF tiene más de {} páginas",
            "PDF-ul are mai mult de {} pagini",
        ],
    ),
    (
        "unreadable PDF: {}",
        ["PDF illisible : {}", "PDF ilegible: {}", "PDF ilizibil: {}"],
    ),
    (
        "no page {}",
        ["pas de page {}", "no hay página {}", "nu există pagina {}"],
    ),
    (
        "session did not shut down within {}",
        [
            "la session ne s’est pas arrêtée en moins de {}",
            "la sesión no se cerró en menos de {}",
            "sesiunea nu s-a închis în mai puțin de {}",
        ],
    ),
    (
        "at column {}: {}",
        [
            "à la colonne {} : {}",
            "en la columna {}: {}",
            "la coloana {}: {}",
        ],
    ),
    (
        "{} exists already",
        ["{} existe déjà", "{} ya existe", "{} există deja"],
    ),
    (
        "message {} is not in the open conversation",
        [
            "le message {} n’est pas dans la discussion ouverte",
            "el mensaje {} no está en la conversación abierta",
            "mesajul {} nu este în conversația deschisă",
        ],
    ),
    (
        "account {} has no folder {}",
        [
            "le compte {} n’a pas de dossier {}",
            "la cuenta {} no tiene la carpeta {}",
            "contul {} nu are dosarul {}",
        ],
    ),
    (
        "must be between 1 and {}",
        [
            "doit être compris entre 1 et {}",
            "debe estar entre 1 y {}",
            "trebuie să fie între 1 și {}",
        ],
    ),
    (
        "must be between 0 and {} (the number of attachments)",
        [
            "doit être compris entre 0 et {} (le nombre de pièces jointes)",
            "debe estar entre 0 y {} (el número de adjuntos)",
            "trebuie să fie între 0 și {} (numărul de atașamente)",
        ],
    ),
    (
        "the draft has {} attachment(s)",
        [
            "le brouillon a {} pièce(s) jointe(s)",
            "el borrador tiene {} adjunto(s)",
            "ciorna are {} atașament(e)",
        ],
    ),
    (
        "expected a folder path like {}",
        [
            "chemin de dossier attendu, par exemple {}",
            "se esperaba una ruta de carpeta como {}",
            "se aștepta o cale de dosar precum {}",
        ],
    ),
    (
        "required for the {} preset",
        [
            "obligatoire pour le préréglage {}",
            "obligatorio para el preajuste {}",
            "obligatoriu pentru presetarea {}",
        ],
    ),
    (
        "this install cannot download the update; open {}",
        [
            "cette installation ne peut pas télécharger la mise à jour ; ouvrez {}",
            "esta instalación no puede descargar la actualización; abre {}",
            "această instalare nu poate descărca actualizarea; deschideți {}",
        ],
    ),
    (
        "{} is still being checked; try again",
        [
            "{} est encore en cours de vérification ; réessayez",
            "{} aún se está comprobando; reinténtalo",
            "{} este încă în verificare; reîncercați",
        ],
    ),
    (
        "`{}:` needs a value",
        [
            "`{}:` demande une valeur",
            "`{}:` necesita un valor",
            "`{}:` necesită o valoare",
        ],
    ),
    (
        "unknown `is:{}`; use is:unread, is:read or is:starred",
        [
            "`is:{}` inconnu ; utilisez is:unread, is:read ou is:starred",
            "`is:{}` desconocido; usa is:unread, is:read o is:starred",
            "`is:{}` necunoscut; folosiți is:unread, is:read sau is:starred",
        ],
    ),
    (
        "unknown `has:{}`; use has:attachment",
        [
            "`has:{}` inconnu ; utilisez has:attachment",
            "`has:{}` desconocido; usa has:attachment",
            "`has:{}` necunoscut; folosiți has:attachment",
        ],
    ),
    (
        "invalid date `{}`; write YYYY-MM-DD",
        [
            "date `{}` non valide ; écrivez AAAA-MM-JJ",
            "fecha `{}` no válida; escribe AAAA-MM-DD",
            "dată `{}` nevalidă; scrieți AAAA-LL-ZZ",
        ],
    ),
    (
        "`{}` is not a calendar date",
        [
            "`{}` n’est pas une date du calendrier",
            "`{}` no es una fecha del calendario",
            "`{}` nu este o dată calendaristică",
        ],
    ),
    // Last: it matches any text with these words.
    (
        "{} failed: {}",
        ["échec de {} : {}", "error en {}: {}", "{} a eșuat: {}"],
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
const LABELS: &[(&str, Tr)] = &[
    ("Reply", ["Répondre", "Responder", "Răspuns"]),
    (
        "Reply all",
        ["Répondre à tous", "Responder a todos", "Răspuns tuturor"],
    ),
    ("Forward", ["Transférer", "Reenviar", "Redirecționare"]),
    (
        "Offer every version again",
        [
            "Proposer de nouveau toutes les versions",
            "Volver a ofrecer todas las versiones",
            "Propunerea din nou a tuturor versiunilor",
        ],
    ),
    (
        "Change update checks",
        [
            "Modifier la recherche de mises à jour",
            "Cambiar la búsqueda de actualizaciones",
            "Schimbarea căutării actualizărilor",
        ],
    ),
    (
        "Create draft",
        ["Créer le brouillon", "Crear el borrador", "Crearea ciornei"],
    ),
    (
        "Edit draft",
        [
            "Modifier le brouillon",
            "Editar el borrador",
            "Editarea ciornei",
        ],
    ),
    (
        "Discard draft",
        [
            "Supprimer le brouillon",
            "Descartar el borrador",
            "Renunțarea la ciornă",
        ],
    ),
    (
        "Restore draft",
        [
            "Restaurer le brouillon",
            "Restaurar el borrador",
            "Restabilirea ciornei",
        ],
    ),
    (
        "Attach file",
        [
            "Joindre le fichier",
            "Adjuntar el archivo",
            "Atașarea fișierului",
        ],
    ),
    (
        "Remove attachment",
        [
            "Retirer la pièce jointe",
            "Quitar el adjunto",
            "Eliminarea atașamentului",
        ],
    ),
    ("Send", ["Envoi", "Envío", "Trimitere"]),
    (
        "Cancel send",
        [
            "Annulation de l’envoi",
            "Cancelación del envío",
            "Anularea trimiterii",
        ],
    ),
    (
        "Change the mailbox's address",
        [
            "Changer l’adresse du dossier",
            "Cambiar la dirección de la carpeta",
            "Schimbarea adresei dosarului",
        ],
    ),
    (
        "Change the sender",
        [
            "Changer l’expéditeur",
            "Cambiar el remitente",
            "Schimbarea expeditorului",
        ],
    ),
    (
        "Key preset",
        [
            "Jeu de raccourcis",
            "Preajuste de atajos",
            "Presetare de comenzi rapide",
        ],
    ),
    ("Theme", ["Thème", "Tema", "Temă"]),
    (
        "Trust the certificate",
        [
            "Faire confiance au certificat",
            "Confiar en el certificado",
            "Acceptarea certificatului",
        ],
    ),
    (
        "Forget the certificate",
        [
            "Oublier le certificat",
            "Olvidar el certificado",
            "Uitarea certificatului",
        ],
    ),
];

/// Undo labels `<prefix><value>`. Longer prefixes come first.
const PREFIXES: &[(&str, Tr)] = &[
    (
        "Mark all read ",
        [
            "Tout marquer comme lu ",
            "Marcar todo como leído ",
            "Marcare totală ca citit ",
        ],
    ),
    (
        "Mark unread ",
        [
            "Marquer comme non lu ",
            "Marcar como no leído ",
            "Marcare ca necitit ",
        ],
    ),
    (
        "Mark read ",
        [
            "Marquer comme lu ",
            "Marcar como leído ",
            "Marcare ca citit ",
        ],
    ),
    (
        "Move to Trash ",
        [
            "Mettre à la corbeille ",
            "Mover a la papelera ",
            "Mutare în coșul de gunoi ",
        ],
    ),
    (
        "Mark as junk ",
        [
            "Signaler comme indésirable ",
            "Marcar como spam ",
            "Marcare ca spam ",
        ],
    ),
    (
        "Not junk ",
        ["Pas indésirable ", "No es spam ", "Nu este spam "],
    ),
    ("Archive ", ["Archiver ", "Archivar ", "Arhivare "]),
    ("Move ", ["Déplacer ", "Mover ", "Mutare "]),
    (
        "Unstar ",
        ["Ne plus suivre ", "Quitar destacado ", "Eliminarea stelei "],
    ),
    ("Star ", ["Suivre ", "Destacar ", "Marcare cu stea "]),
    (
        "Add account ",
        [
            "Ajouter le compte ",
            "Añadir la cuenta ",
            "Adăugarea contului ",
        ],
    ),
    (
        "Edit account ",
        [
            "Modifier le compte ",
            "Editar la cuenta ",
            "Editarea contului ",
        ],
    ),
    (
        "Show images from ",
        [
            "Afficher les images de ",
            "Mostrar las imágenes de ",
            "Afișarea imaginilor de la ",
        ],
    ),
    (
        "Hide images from ",
        [
            "Masquer les images de ",
            "Ocultar las imágenes de ",
            "Ascunderea imaginilor de la ",
        ],
    ),
    (
        "Skip version ",
        [
            "Ignorer la version ",
            "Omitir la versión ",
            "Omiterea versiunii ",
        ],
    ),
    (
        "Default offline window for ",
        [
            "Fenêtre hors ligne par défaut pour ",
            "Copia sin conexión predeterminada para ",
            "Copie offline implicită pentru ",
        ],
    ),
    (
        "Keep all of ",
        [
            "Tout garder hors ligne : ",
            "Conservar todo sin conexión: ",
            "Păstrare integrală offline: ",
        ],
    ),
    ("Keys of ", ["Touches de ", "Teclas de ", "Tastele pentru "]),
    (
        "Rename ",
        ["Renommer ", "Cambiar el nombre de ", "Redenumire "],
    ),
    ("Remove ", ["Retirer ", "Quitar ", "Eliminare "]),
    ("Unpin ", ["Détacher ", "Desfijar ", "Anularea fixării "]),
    ("Pin ", ["Épingler ", "Fijar ", "Fixare "]),
    ("Hide ", ["Masquer ", "Ocultar ", "Ascundere "]),
    ("Show ", ["Afficher ", "Mostrar ", "Afișare "]),
    ("Set ", ["Régler ", "Ajustar ", "Setare "]),
];

/// "Keep the newest {n} offline of {path}".
const KEEP_NEWEST: Tr = [
    "Garder hors ligne les {n} plus récents de {path}",
    "Conservar sin conexión los {n} más recientes de {path}",
    "Păstrarea offline a celor mai noi {n} din {path}",
];

/// The column of `lang` in a [`Tr`]; `None` for English and the pseudo-locale.
const fn column(lang: Lang) -> Option<usize> {
    match lang {
        Lang::French => Some(0),
        Lang::Spanish => Some(1),
        Lang::Romanian => Some(2),
        Lang::English | Lang::Pseudo => None,
    }
}

/// `template` with each `{}` replaced by the next of `values`.
fn positional(template: &str, values: &[&str]) -> String {
    let mut parts = template.split("{}");
    let mut out = parts.next().unwrap_or_default().to_owned();
    for (part, value) in parts.zip(values.iter().chain(std::iter::repeat(&""))) {
        out.push_str(value);
        out.push_str(part);
    }
    out
}

fn reason_in(col: usize, reason: &str) -> String {
    if let Some((_, tr)) = REASONS.iter().find(|(english, _)| *english == reason) {
        return tr[col].to_owned();
    }
    PATTERNS
        .iter()
        .find_map(|(english, tr)| {
            let values = values(english, reason)?;
            // A column's reason is itself a reason.
            let values: Vec<String> = values.iter().map(|v| reason_in(col, v)).collect();
            let values: Vec<&str> = values.iter().map(String::as_str).collect();
            Some(positional(tr[col], &values))
        })
        .unwrap_or_else(|| entity(col, &role(col, reason)))
}

fn role(col: usize, role: &str) -> String {
    let tr: Option<Tr> = match role {
        "Inbox" => Some(["de réception", "de entrada", "de intrare"]),
        "Sent" => Some(["des messages envoyés", "de enviados", "pentru trimise"]),
        "Drafts" => Some(["de brouillons", "de borradores", "pentru ciorne"]),
        "Trash" => Some(["corbeille", "de papelera", "coș de gunoi"]),
        "Archive" => Some(["d’archives", "de archivo", "de arhivă"]),
        "Junk" => Some(["de courrier indésirable", "de spam", "spam"]),
        "All" => Some([
            "de tout le courrier",
            "de todo el correo",
            "cu toate e-mailurile",
        ]),
        "Flagged" => Some(["de messages suivis", "de destacados", "cu stea"]),
        _ => None,
    };
    tr.map_or_else(|| role.to_owned(), |tr| tr[col].to_owned())
}

fn entity(col: usize, entity: &str) -> String {
    let tr: Option<Tr> = match entity {
        "account" => Some(["compte", "cuenta", "contul"]),
        "attachment" => Some(["pièce jointe", "adjunto", "atașamentul"]),
        "draft" => Some(["brouillon", "borrador", "ciorna"]),
        "mailbox" => Some(["dossier", "carpeta", "dosarul"]),
        "message" => Some(["message", "mensaje", "mesajul"]),
        "thread" => Some(["discussion", "conversación", "conversația"]),
        "certificate offer" => Some([
            "certificat proposé",
            "certificado ofrecido",
            "certificatul oferit",
        ]),
        "trusted certificate" => Some([
            "certificat de confiance",
            "certificado de confianza",
            "certificatul acceptat",
        ]),
        "identity" => Some([
            "adresse d’envoi",
            "dirección de envío",
            "adresa de trimitere",
        ]),
        _ => None,
    };
    tr.map_or_else(|| entity.to_owned(), |tr| tr[col].to_owned())
}

/// `code` and `message` joined as `lang` writes it: French puts a space before the colon.
fn with_code(lang: Lang, code: &str, message: &str) -> String {
    if lang == Lang::French {
        format!("{code} : {message}")
    } else {
        format!("{code}: {message}")
    }
}

/// `err` as the user reads it in `lang`. The code stays as it is in every language.
pub fn error_in(lang: Lang, err: &CoreError) -> String {
    let text = err.to_string();
    let prefix = format!("{}: ", err.code());
    let english = text.strip_prefix(&prefix).unwrap_or(&text);
    match (lang, column(lang)) {
        (Lang::Pseudo, _) => format!("{prefix}{}", pseudo(english)),
        (_, Some(col)) => with_code(lang, err.code(), &translated_error(col, err)),
        (_, None) => text,
    }
}

/// An error that reached a front-end as text, `ERR<code>: <English>` or plain, in `lang`.
/// The code stays. A text unknown here (a server's message) is shown as it is.
pub fn message_in(lang: Lang, text: &str) -> String {
    let (code, message) = match text.split_once(": ") {
        Some((code, rest))
            if code.len() > 3
                && code.starts_with("ERR")
                && code[3..].bytes().all(|b| b.is_ascii_digit()) =>
        {
            (Some(code), rest)
        }
        _ => (None, text),
    };
    match (lang, column(lang), code) {
        (Lang::Pseudo, _, Some(code)) => format!("{code}: {}", pseudo(message)),
        (Lang::Pseudo, _, None) => pseudo(message),
        (_, Some(col), Some(code)) => with_code(lang, code, &reason_in(col, message)),
        (_, Some(col), None) => reason_in(col, message),
        (_, None, _) => text.to_owned(),
    }
}

/// The match is exhaustive, so a new error does not compile without its translations.
fn translated_error(col: usize, err: &CoreError) -> String {
    let t = |tr: Tr, values: &[&str]| positional(tr[col], values);
    match err {
        CoreError::InvalidCommand { reason } => t(
            [
                "commande non valide : {}",
                "comando no válido: {}",
                "comandă nevalidă: {}",
            ],
            &[&reason_in(col, reason)],
        ),
        CoreError::InvalidArgument { field, reason } => t(
            [
                "argument `{}` non valide : {}",
                "argumento `{}` no válido: {}",
                "argument `{}` nevalid: {}",
            ],
            &[field, &reason_in(col, reason)],
        ),
        CoreError::InvalidReplay { line, reason } => t(
            [
                "enregistrement non valide à la ligne {} : {}",
                "registro no válido en la línea {}: {}",
                "înregistrare nevalidă la rândul {}: {}",
            ],
            &[&line.to_string(), reason],
        ),
        CoreError::NotFound { entity: what, id } => t(
            [
                "{} {} introuvable",
                "{} {}: no se encontró",
                "{} {}: negăsit",
            ],
            &[&entity(col, what), id],
        ),
        CoreError::AlreadyExists { entity: what, key } => t(
            [
                "{} {} existe déjà",
                "{} {}: ya existe",
                "{} {}: există deja",
            ],
            &[&entity(col, what), key],
        ),
        CoreError::ConfirmationRequired { command } => t(
            [
                "{} est irréversible ; répétez avec `confirm: true`",
                "{} es irreversible; repítelo con `confirm: true`",
                "{} este ireversibil; repetați cu `confirm: true`",
            ],
            &[command],
        ),
        CoreError::NoMailboxForRole {
            account,
            role: which,
        } => t(
            [
                "le compte {} n’a pas de dossier {}",
                "la cuenta {} no tiene carpeta {}",
                "contul {} nu are dosar {}",
            ],
            &[&account.to_string(), &role(col, &format!("{which:?}"))],
        ),
        CoreError::OriginNotAllowed { command, origin } => t(
            [
                "{} ne peut pas être lancé depuis {} ; l’utilisateur doit le faire dans l’interface graphique ou la ligne de commande",
                "{} no se puede ejecutar desde {}; el usuario debe hacerlo en la interfaz gráfica o en la línea de comandos",
                "{} nu poate fi rulat din {}; utilizatorul trebuie să o facă în interfața grafică sau în linia de comandă",
            ],
            &[command, &origin.to_string()],
        ),
        CoreError::NotSendable { draft, reason } => t(
            [
                "le brouillon {} ne peut pas être envoyé : {}",
                "no se puede enviar el borrador {}: {}",
                "ciorna {} nu poate fi trimisă: {}",
            ],
            &[&draft.to_string(), &reason_in(col, reason)],
        ),
        CoreError::NothingToUndo => reason_in(col, "nothing to undo"),
        CoreError::NothingToRedo => reason_in(col, "nothing to redo"),
    }
}

/// The undo label `label` in `lang`.
pub fn label_in(lang: Lang, label: &str) -> String {
    match (lang, column(lang)) {
        (Lang::Pseudo, _) => pseudo(label),
        (_, Some(col)) => translated_label(col, label),
        (_, None) => label.to_owned(),
    }
}

fn translated_label(col: usize, label: &str) -> String {
    if let Some((_, tr)) = LABELS.iter().find(|(english, _)| *english == label) {
        return tr[col].to_owned();
    }
    let keep_newest = label
        .strip_prefix("Keep the newest ")
        .and_then(|rest| rest.strip_suffix(" offline"))
        .and_then(|rest| rest.split_once(" of "));
    if let Some((n, path)) = keep_newest {
        return super::fill(KEEP_NEWEST[col], &[("n", n), ("path", path)]);
    }
    PREFIXES
        .iter()
        .find_map(|(english, tr)| {
            let rest = label.strip_prefix(english)?;
            let rest = rest.strip_suffix(" offline").unwrap_or(rest);
            Some(format!("{}{rest}", tr[col]))
        })
        .unwrap_or_else(|| label.to_owned())
}

/// A command's one-sentence help in `lang`: settings tips and the palette's help line.
pub fn help_in(lang: Lang, name: CommandName) -> String {
    match lang {
        Lang::English => first_sentence(name),
        Lang::French => commands::french_help(name).to_owned(),
        Lang::Spanish => commands::spanish_help(name).to_owned(),
        Lang::Romanian => commands::romanian_help(name).to_owned(),
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

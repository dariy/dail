//! The engine's texts in a language. `app-core` stays in English: its errors and undo labels
//! are part of the JSON that agents read. The front-ends translate them here, for display.
//! An English text unknown here (a server's message, a new label) is shown as it is.
//! Agents read English: the CLI and MCP outputs are not translated.

use app_core::{CommandName, CoreError};

use super::{Lang, commands, pseudo};

/// A text in French, Spanish, Romanian, Brazilian and European Portuguese, and Latin, in [`column`]
/// order.
type Tr = [&'static str; 6];

/// English reasons of `error_002_invalid_argument` and similar errors, with their translations.
const REASONS: &[(&str, Tr)] = &[
    (
        "there is no folder for the draft's attachments",
        [
            "il n’y a pas de dossier pour les pièces jointes du brouillon",
            "no hay carpeta para los adjuntos del borrador",
            "nu există un dosar pentru atașamentele ciornei",
            "não há pasta para os anexos do rascunho",
            "não há pasta para os anexos do rascunho",
            "nulla capsa est pro adiunctis adumbrationis",
        ],
    ),
    (
        "expected an address like sales@example.org",
        [
            "adresse attendue, par exemple sales@example.org",
            "se esperaba una dirección como ventas@example.org",
            "se aștepta o adresă precum vanzari@example.org",
            "esperava-se um endereço como vendas@example.org",
            "esperava-se um endereço como vendas@example.org",
            "inscriptio exspectabatur ut venditio@example.org",
        ],
    ),
    (
        "the body is not downloaded yet (open_thread downloads it)",
        [
            "le corps n’est pas encore téléchargé (open_thread le télécharge)",
            "el cuerpo aún no se ha descargado (open_thread lo descarga)",
            "corpul nu este descărcat încă (open_thread îl descarcă)",
            "o corpo ainda não foi baixado (open_thread o baixa)",
            "o corpo ainda não foi transferido (open_thread transfere-o)",
            "corpus nondum depressum est (open_thread id deprimit)",
        ],
    ),
    (
        "expected the absolute path of a folder",
        [
            "chemin absolu d’un dossier attendu",
            "se esperaba la ruta absoluta de una carpeta",
            "se aștepta calea absolută a unui dosar",
            "esperava-se o caminho absoluto de uma pasta",
            "esperava-se o caminho absoluto de uma pasta",
            "via absoluta capsae exspectabatur",
        ],
    ),
    (
        "give a mode, a light theme or a dark theme",
        [
            "indiquez un mode, un thème clair ou un thème sombre",
            "indica un modo, un tema claro o un tema oscuro",
            "indicați un mod, o temă luminoasă sau o temă întunecată",
            "informe um modo, um tema claro ou um tema escuro",
            "indique um modo, um tema claro ou um tema escuro",
            "da modum, thema lucidum aut thema obscurum",
        ],
    ),
    (
        "nothing to undo",
        [
            "rien à annuler",
            "nada que deshacer",
            "nimic de anulat",
            "nada para desfazer",
            "nada para anular",
            "nihil revocandum",
        ],
    ),
    (
        "nothing to redo",
        [
            "rien à rétablir",
            "nada que rehacer",
            "nimic de refăcut",
            "nada para refazer",
            "nada para refazer",
            "nihil restituendum",
        ],
    ),
    (
        "session closed",
        [
            "session fermée",
            "sesión cerrada",
            "sesiune închisă",
            "sessão encerrada",
            "sessão terminada",
            "sessio clausa",
        ],
    ),
    (
        "dail stopped while reading this message, so it is not read again",
        [
            "dail s’est arrêté en lisant ce message, il n’est donc plus lu",
            "dail se detuvo al leer este mensaje, así que no se vuelve a leer",
            "dail s-a oprit în timp ce citea acest mesaj, așa că nu îl mai citește",
            "o dail parou ao ler esta mensagem, então ela não é lida de novo",
            "o dail parou ao ler esta mensagem, por isso não volta a ser lida",
            "dail constitit dum hanc epistulam legit, itaque non iterum legitur",
        ],
    ),
    (
        "dail hit an internal error; the log has the details",
        [
            "dail a rencontré une erreur interne ; le journal contient les détails",
            "dail sufrió un error interno; el registro tiene los detalles",
            "dail a întâmpinat o eroare internă; jurnalul conține detaliile",
            "o dail encontrou um erro interno; o log tem os detalhes",
            "o dail encontrou um erro interno; o registo tem os detalhes",
            "dail errorem internum offendit; acta singula habent",
        ],
    ),
    (
        "the session is closed",
        [
            "la session est fermée",
            "la sesión está cerrada",
            "sesiunea este închisă",
            "a sessão está encerrada",
            "a sessão está terminada",
            "sessio clausa est",
        ],
    ),
    (
        "dail's engine stopped; restart the app",
        [
            "le moteur de dail s’est arrêté ; redémarrez l’application",
            "el motor de dail se detuvo; reinicia la aplicación",
            "motorul dail s-a oprit; reporniți aplicația",
            "o motor do dail parou; reinicie o app",
            "o motor do dail parou; reinicie a aplicação",
            "machina dail constitit; applicationem iterum incipe",
        ],
    ),
    (
        "not a PNG, JPEG, GIF or WebP image",
        [
            "pas une image PNG, JPEG, GIF ou WebP",
            "no es una imagen PNG, JPEG, GIF ni WebP",
            "nu este o imagine PNG, JPEG, GIF sau WebP",
            "não é uma imagem PNG, JPEG, GIF ou WebP",
            "não é uma imagem PNG, JPEG, GIF ou WebP",
            "non est imago PNG, JPEG, GIF aut WebP",
        ],
    ),
    (
        "this build trusts no update signing key",
        [
            "cette version ne fait confiance à aucune clé de signature de mise à jour",
            "esta compilación no confía en ninguna clave de firma de actualizaciones",
            "această versiune nu are încredere în nicio cheie de semnare a actualizărilor",
            "esta versão não confia em nenhuma chave de assinatura de atualizações",
            "esta versão não confia em nenhuma chave de assinatura de atualizações",
            "haec versio nulli clavi signandarum renovationum confidit",
        ],
    ),
    (
        "the manifest signature does not verify",
        [
            "la signature du manifeste n’est pas valide",
            "la firma del manifiesto no es válida",
            "semnătura manifestului nu este validă",
            "a assinatura do manifesto não é válida",
            "a assinatura do manifesto não é válida",
            "signum manifesti non convenit",
        ],
    ),
    (
        "the download's sha256 does not match the manifest",
        [
            "le sha256 du téléchargement ne correspond pas au manifeste",
            "el sha256 de la descarga no coincide con el manifiesto",
            "sha256 al descărcării nu corespunde manifestului",
            "o sha256 do download não corresponde ao manifesto",
            "o sha256 da transferência não corresponde ao manifesto",
            "sha256 depressionis cum manifesto non congruit",
        ],
    ),
    (
        "no data directory found for profiles (is $HOME set?)",
        [
            "aucun dossier de données trouvé pour les profils ($HOME est-il défini ?)",
            "no se encontró una carpeta de datos para los perfiles (¿está definido $HOME?)",
            "nu s-a găsit un dosar de date pentru profiluri (este setat $HOME?)",
            "nenhuma pasta de dados encontrada para os perfis ($HOME está definido?)",
            "nenhuma pasta de dados encontrada para os perfis ($HOME está definido?)",
            "nulla capsa datorum pro profilis inventa ($HOME constitutum est?)",
        ],
    ),
    (
        "the page size is out of range",
        [
            "la taille de la page est hors limites",
            "el tamaño de página está fuera de rango",
            "dimensiunea paginii este în afara limitelor",
            "o tamanho da página está fora do intervalo",
            "o tamanho da página está fora do intervalo",
            "magnitudo paginae extra fines est",
        ],
    ),
    (
        "the tile is out of range",
        [
            "la tuile est hors limites",
            "el mosaico está fuera de rango",
            "dala este în afara limitelor",
            "o bloco está fora do intervalo",
            "o bloco está fora do intervalo",
            "tessella extra fines est",
        ],
    ),
    (
        "the PDF renderer failed",
        [
            "le moteur de rendu PDF a échoué",
            "falló el motor de PDF",
            "motorul de randare PDF a eșuat",
            "o renderizador de PDF falhou",
            "o motor de PDF falhou",
            "pictor PDF defecit",
        ],
    ),
    (
        "it is being sent and can't be stopped any more",
        [
            "il est en cours d’envoi et ne peut plus être arrêté",
            "se está enviando y ya no se puede detener",
            "se trimite și nu mai poate fi oprit",
            "está sendo enviada e não pode mais ser interrompida",
            "está a ser enviada e já não pode ser interrompida",
            "mittitur neque iam sisti potest",
        ],
    ),
    (
        "no send is scheduled",
        [
            "aucun envoi n’est programmé",
            "no hay ningún envío programado",
            "nu este programată nicio trimitere",
            "nenhum envio está programado",
            "nenhum envio está agendado",
            "nulla missio constituta est",
        ],
    ),
    (
        "it is being sent",
        [
            "il est en cours d’envoi",
            "se está enviando",
            "se trimite",
            "está sendo enviada",
            "está a ser enviada",
            "mittitur",
        ],
    ),
    (
        "a send is scheduled; run cancel_send first",
        [
            "un envoi est programmé ; lancez d’abord cancel_send",
            "hay un envío programado; ejecuta antes cancel_send",
            "este programată o trimitere; rulați mai întâi cancel_send",
            "há um envio programado; execute cancel_send primeiro",
            "há um envio agendado; execute primeiro cancel_send",
            "missio constituta est; prius cancel_send exsequere",
        ],
    ),
    (
        "it has no recipients; set to, cc or bcc with update_draft",
        [
            "il n’a pas de destinataire ; indiquez to, cc ou bcc avec update_draft",
            "no tiene destinatarios; indica to, cc o bcc con update_draft",
            "nu are destinatari; setați to, cc sau bcc cu update_draft",
            "não tem destinatários; defina to, cc ou bcc com update_draft",
            "não tem destinatários; defina to, cc ou bcc com update_draft",
            "recipientes non habet; to, cc aut bcc per update_draft constitue",
        ],
    ),
    (
        "no thread is open (open_thread)",
        [
            "aucune discussion n’est ouverte (open_thread)",
            "no hay ninguna conversación abierta (open_thread)",
            "nu este deschisă nicio conversație (open_thread)",
            "nenhuma conversa está aberta (open_thread)",
            "nenhuma conversa está aberta (open_thread)",
            "nullum colloquium apertum est (open_thread)",
        ],
    ),
    (
        "no draft is open (open_composer)",
        [
            "aucun brouillon n’est ouvert (open_composer)",
            "no hay ningún borrador abierto (open_composer)",
            "nu este deschisă nicio ciornă (open_composer)",
            "nenhum rascunho está aberto (open_composer)",
            "nenhum rascunho está aberto (open_composer)",
            "nulla adumbratio aperta est (open_composer)",
        ],
    ),
    (
        "the pane is not shown",
        [
            "le volet n’est pas affiché",
            "el panel no se muestra",
            "panoul nu este afișat",
            "o painel não está sendo mostrado",
            "o painel não está a ser mostrado",
            "tabulatum non monstratur",
        ],
    ),
    (
        "must be a message ID without spaces, line breaks or control characters",
        [
            "doit être un identifiant de message sans espace, saut de ligne ni caractère de contrôle",
            "debe ser un ID de mensaje sin espacios, saltos de línea ni caracteres de control",
            "trebuie să fie un ID de mesaj fără spații, sfârșituri de rând sau caractere de control",
            "deve ser um ID de mensagem sem espaços, quebras de linha ou caracteres de controle",
            "deve ser um ID de mensagem sem espaços, quebras de linha ou caracteres de controlo",
            "debet esse signum epistulae sine spatiis, fractionibus versuum aut characteribus imperii",
        ],
    ),
    (
        "empty phrase; remove the quotes",
        [
            "expression vide ; retirez les guillemets",
            "frase vacía; quita las comillas",
            "expresie goală; eliminați ghilimelele",
            "frase vazia; remova as aspas",
            "frase vazia; remova as aspas",
            "locutio vacua; virgulas remove",
        ],
    ),
    (
        "unterminated quote; add a closing `\"`",
        [
            "guillemet non fermé ; ajoutez un `\"` final",
            "comillas sin cerrar; añade un `\"` de cierre",
            "ghilimele neînchise; adăugați un `\"` de închidere",
            "aspas não fechadas; adicione um `\"` de fechamento",
            "aspas não fechadas; acrescente um `\"` de fecho",
            "virgulae non clausae; `\"` claudens adde",
        ],
    ),
    (
        "expected a space after the closing `\"`",
        [
            "espace attendu après le `\"` final",
            "se esperaba un espacio tras el `\"` de cierre",
            "se aștepta un spațiu după `\"` de închidere",
            "esperava-se um espaço após o `\"` de fechamento",
            "esperava-se um espaço após o `\"` de fecho",
            "spatium post `\"` claudens exspectabatur",
        ],
    ),
    (
        "give `ids` (threads), `messages` or both",
        [
            "indiquez `ids` (discussions), `messages` ou les deux",
            "indica `ids` (conversaciones), `messages` o ambos",
            "indicați `ids` (conversații), `messages` sau ambele",
            "informe `ids` (conversas), `messages` ou ambos",
            "indique `ids` (conversas), `messages` ou ambos",
            "da `ids` (colloquia), `messages` aut utraque",
        ],
    ),
    (
        "the mailbox belongs to another account than the messages",
        [
            "le dossier appartient à un autre compte que les messages",
            "la carpeta pertenece a otra cuenta que los mensajes",
            "dosarul aparține altui cont decât mesajele",
            "a pasta pertence a outra conta que não a das mensagens",
            "a pasta pertence a outra conta que não a das mensagens",
            "capsa ad aliam rationem pertinet quam epistulae",
        ],
    ),
    (
        "the op is running on the server; discard it after it failed",
        [
            "l’opération est en cours sur le serveur ; abandonnez-la après son échec",
            "la operación se está ejecutando en el servidor; descártala cuando falle",
            "operațiunea rulează pe server; renunțați la ea după ce eșuează",
            "a operação está em execução no servidor; descarte-a depois que falhar",
            "a operação está em execução no servidor; rejeite-a depois de falhar",
            "operatio in servitore agitur; eam abice postquam defecerit",
        ],
    ),
    (
        "expected an email address like news@example.org",
        [
            "adresse attendue, par exemple news@example.org",
            "se esperaba una dirección como news@example.org",
            "se aștepta o adresă precum news@example.org",
            "esperava-se um endereço como news@example.org",
            "esperava-se um endereço como news@example.org",
            "inscriptio exspectabatur ut news@example.org",
        ],
    ),
    (
        "no conversation is open (open_thread)",
        [
            "aucune discussion n’est ouverte (open_thread)",
            "no hay ninguna conversación abierta (open_thread)",
            "nu este deschisă nicio conversație (open_thread)",
            "nenhuma conversa está aberta (open_thread)",
            "nenhuma conversa está aberta (open_thread)",
            "nullum colloquium apertum est (open_thread)",
        ],
    ),
    (
        "give either id or account",
        [
            "indiquez id ou account",
            "indica id o account",
            "indicați id sau account",
            "informe id ou account",
            "indique id ou account",
            "da id aut account",
        ],
    ),
    (
        "is empty; give at least one term, e.g. `is:unread`",
        [
            "est vide ; donnez au moins un terme, par exemple `is:unread`",
            "está vacía; indica al menos un término, p. ej. `is:unread`",
            "este goală; indicați cel puțin un termen, de ex. `is:unread`",
            "está vazia; informe pelo menos um termo, p. ex. `is:unread`",
            "está vazia; indique pelo menos um termo, p. ex. `is:unread`",
            "vacua est; da saltem unum terminum, e.g. `is:unread`",
        ],
    ),
    (
        "the inbox can't be renamed",
        [
            "la boîte de réception ne peut pas être renommée",
            "no se puede cambiar el nombre de la bandeja de entrada",
            "căsuța de intrare nu poate fi redenumită",
            "a caixa de entrada não pode ser renomeada",
            "não é possível mudar o nome da caixa de entrada",
            "capsa acceptorum renominari non potest",
        ],
    ),
    (
        "the inbox can't be deleted",
        [
            "la boîte de réception ne peut pas être supprimée",
            "no se puede eliminar la bandeja de entrada",
            "căsuța de intrare nu poate fi ștearsă",
            "a caixa de entrada não pode ser excluída",
            "a caixa de entrada não pode ser eliminada",
            "capsa acceptorum deleri non potest",
        ],
    ),
    (
        "that is its name already",
        [
            "c’est déjà son nom",
            "ya se llama así",
            "acesta este deja numele său",
            "esse já é o nome dela",
            "esse já é o seu nome",
            "id iam nomen eius est",
        ],
    ),
    (
        "a folder can't move into itself",
        [
            "un dossier ne peut pas être déplacé dans lui-même",
            "una carpeta no se puede mover dentro de sí misma",
            "un dosar nu poate fi mutat în el însuși",
            "uma pasta não pode ser movida para dentro de si mesma",
            "uma pasta não pode ser movida para dentro de si mesma",
            "capsa in se ipsam moveri non potest",
        ],
    ),
    (
        "delete its subfolders first",
        [
            "supprimez d’abord ses sous-dossiers",
            "elimina antes sus subcarpetas",
            "ștergeți mai întâi subdosarele sale",
            "exclua as subpastas primeiro",
            "elimine primeiro as subpastas",
            "prius subcapsas eius dele",
        ],
    ),
    (
        "must not be empty",
        [
            "ne doit pas être vide",
            "no puede estar vacío",
            "nu trebuie să fie gol",
            "não pode estar vazio",
            "não pode estar vazio",
            "vacuum esse non debet",
        ],
    ),
    (
        "nothing to update: give display_name, imap or smtp",
        [
            "rien à modifier : indiquez display_name, imap ou smtp",
            "nada que cambiar: indica display_name, imap o smtp",
            "nimic de modificat: indicați display_name, imap sau smtp",
            "nada para alterar: informe display_name, imap ou smtp",
            "nada para alterar: indique display_name, imap ou smtp",
            "nihil mutandum: da display_name, imap aut smtp",
        ],
    ),
    (
        "expected an address like name@example.com",
        [
            "adresse attendue, par exemple nom@example.com",
            "se esperaba una dirección como nombre@example.com",
            "se aștepta o adresă precum nume@example.com",
            "esperava-se um endereço como nome@example.com",
            "esperava-se um endereço como nome@example.com",
            "inscriptio exspectabatur ut nomen@example.com",
        ],
    ),
    (
        "expected an address like me@example.org",
        [
            "adresse attendue, par exemple moi@example.org",
            "se esperaba una dirección como yo@example.org",
            "se aștepta o adresă precum eu@example.org",
            "esperava-se um endereço como eu@example.org",
            "esperava-se um endereço como eu@example.org",
            "inscriptio exspectabatur ut ego@example.org",
        ],
    ),
    (
        "must be non-empty and contain no control characters",
        [
            "ne doit être ni vide ni contenir de caractères de contrôle",
            "no puede estar vacío ni contener caracteres de control",
            "nu trebuie să fie gol și nici să conțină caractere de control",
            "não pode estar vazio nem conter caracteres de controle",
            "não pode estar vazio nem conter caracteres de controlo",
            "vacuum esse non debet neque characteres imperii continere",
        ],
    ),
    (
        "host must be a DNS name or IP address",
        [
            "l’hôte doit être un nom DNS ou une adresse IP",
            "el host debe ser un nombre DNS o una dirección IP",
            "gazda trebuie să fie un nume DNS sau o adresă IP",
            "o host deve ser um nome DNS ou um endereço IP",
            "o anfitrião deve ser um nome DNS ou um endereço IP",
            "hospes nomen DNS aut inscriptio IP esse debet",
        ],
    ),
    (
        "port must be between 1 and 65535",
        [
            "le port doit être compris entre 1 et 65535",
            "el puerto debe estar entre 1 y 65535",
            "portul trebuie să fie între 1 și 65535",
            "a porta deve estar entre 1 e 65535",
            "a porta deve estar entre 1 e 65535",
            "porta inter 1 et 65535 esse debet",
        ],
    ),
    (
        "this build cannot update itself (the runtime reported no version)",
        [
            "cette version ne peut pas se mettre à jour elle-même",
            "esta compilación no puede actualizarse sola",
            "această versiune nu se poate actualiza singură",
            "esta versão não pode se atualizar sozinha",
            "esta versão não se pode atualizar sozinha",
            "haec versio se ipsa renovare non potest",
        ],
    ),
    (
        "no update is available; run check_for_updates first",
        [
            "aucune mise à jour disponible ; lancez d’abord check_for_updates",
            "no hay ninguna actualización disponible; ejecuta antes check_for_updates",
            "nu este disponibilă nicio actualizare; rulați mai întâi check_for_updates",
            "nenhuma atualização disponível; execute check_for_updates primeiro",
            "nenhuma atualização disponível; execute primeiro check_for_updates",
            "nulla renovatio praesto est; prius check_for_updates exsequere",
        ],
    ),
    (
        "no update is downloaded; run download_update first",
        [
            "aucune mise à jour téléchargée ; lancez d’abord download_update",
            "no hay ninguna actualización descargada; ejecuta antes download_update",
            "nu este descărcată nicio actualizare; rulați mai întâi download_update",
            "nenhuma atualização baixada; execute download_update primeiro",
            "nenhuma atualização transferida; execute primeiro download_update",
            "nulla renovatio depressa est; prius download_update exsequere",
        ],
    ),
    (
        "the runtime reported no install kind",
        [
            "type d’installation inconnu",
            "tipo de instalación desconocido",
            "tip de instalare necunoscut",
            "tipo de instalação desconhecido",
            "tipo de instalação desconhecido",
            "genus institutionis ignotum",
        ],
    ),
    (
        "expected a version like 0.2.0, or null",
        [
            "version attendue, par exemple 0.2.0, ou null",
            "se esperaba una versión como 0.2.0, o null",
            "se aștepta o versiune precum 0.2.0 sau null",
            "esperava-se uma versão como 0.2.0, ou null",
            "esperava-se uma versão como 0.2.0, ou null",
            "versio exspectabatur ut 0.2.0, aut null",
        ],
    ),
    (
        "expected 1 to 720 hours",
        [
            "de 1 à 720 heures attendues",
            "se esperaban de 1 a 720 horas",
            "se așteptau între 1 și 720 de ore",
            "esperava-se de 1 a 720 horas",
            "esperava-se de 1 a 720 horas",
            "ab 1 ad 720 horas exspectabantur",
        ],
    ),
    (
        "nothing to update: give to, cc, bcc, subject or body",
        [
            "rien à modifier : indiquez to, cc, bcc, subject ou body",
            "nada que cambiar: indica to, cc, bcc, subject o body",
            "nimic de modificat: indicați to, cc, bcc, subject sau body",
            "nada para alterar: informe to, cc, bcc, subject ou body",
            "nada para alterar: indique to, cc, bcc, subject ou body",
            "nihil mutandum: da to, cc, bcc, subject aut body",
        ],
    ),
    (
        "must not contain line breaks or control characters",
        [
            "ne doit contenir ni saut de ligne ni caractère de contrôle",
            "no puede contener saltos de línea ni caracteres de control",
            "nu trebuie să conțină sfârșituri de rând sau caractere de control",
            "não pode conter quebras de linha nem caracteres de controle",
            "não pode conter quebras de linha nem caracteres de controlo",
            "fractiones versuum aut characteres imperii continere non debet",
        ],
    ),
    (
        "must not contain NUL characters",
        [
            "ne doit pas contenir de caractère NUL",
            "no puede contener caracteres NUL",
            "nu trebuie să conțină caractere NUL",
            "não pode conter caracteres NUL",
            "não pode conter caracteres NUL",
            "characteres NUL continere non debet",
        ],
    ),
    (
        "must be an absolute file path",
        [
            "doit être un chemin de fichier absolu",
            "debe ser una ruta de archivo absoluta",
            "trebuie să fie o cale absolută de fișier",
            "deve ser um caminho de arquivo absoluto",
            "deve ser um caminho de ficheiro absoluto",
            "via absoluta documenti esse debet",
        ],
    ),
    (
        "must name a file, not a directory",
        [
            "doit désigner un fichier, pas un répertoire",
            "debe indicar un archivo, no un directorio",
            "trebuie să indice un fișier, nu un director",
            "deve indicar um arquivo, não um diretório",
            "deve indicar um ficheiro, não um diretório",
            "documentum nominare debet, non directorium",
        ],
    ),
    (
        "expected a SHA-256 fingerprint: 64 hex digits",
        [
            "empreinte SHA-256 attendue : 64 chiffres hexadécimaux",
            "se esperaba una huella SHA-256: 64 dígitos hexadecimales",
            "se aștepta o amprentă SHA-256: 64 de cifre hexazecimale",
            "esperava-se uma impressão digital SHA-256: 64 dígitos hexadecimais",
            "esperava-se uma impressão digital SHA-256: 64 dígitos hexadecimais",
            "vestigium SHA-256 exspectabatur: 64 notae sedecimales",
        ],
    ),
    (
        "must be 1 to 64 characters without control characters",
        [
            "doit compter de 1 à 64 caractères, sans caractère de contrôle",
            "debe tener de 1 a 64 caracteres, sin caracteres de control",
            "trebuie să aibă între 1 și 64 de caractere, fără caractere de control",
            "deve ter de 1 a 64 caracteres, sem caracteres de controle",
            "deve ter de 1 a 64 carateres, sem carateres de controlo",
            "1 ad 64 characteres sine characteribus moderandi habere debet",
        ],
    ),
    (
        "the server has not shown a certificate yet; wait for it after connect_server",
        [
            "le serveur n’a pas encore présenté de certificat ; attendez-le après connect_server",
            "el servidor aún no ha presentado un certificado; espéralo después de connect_server",
            "serverul nu a prezentat încă un certificat; așteptați-l după connect_server",
            "o servidor ainda não apresentou um certificado; aguarde-o depois de connect_server",
            "o servidor ainda não apresentou um certificado; aguarde-o depois de connect_server",
            "servitor nondum testimonium ostendit; id post connect_server exspecta",
        ],
    ),
    (
        "does not match the certificate the server presented; do not pair",
        [
            "ne correspond pas au certificat présenté par le serveur ; ne vous associez pas",
            "no coincide con el certificado que presentó el servidor; no te vincules",
            "nu corespunde certificatului prezentat de server; nu asociați dispozitivul",
            "não corresponde ao certificado que o servidor apresentou; não faça o pareamento",
            "não corresponde ao certificado que o servidor apresentou; não emparelhe",
            "testimonio a servitore ostenso non congruit; noli copulare",
        ],
    ),
    (
        "expected the 8-character pairing code from start_pairing (A-Z, 2-7)",
        [
            "code d’association de 8 caractères attendu, donné par start_pairing (A-Z, 2-7)",
            "se esperaba el código de vinculación de 8 caracteres de start_pairing (A-Z, 2-7)",
            "se aștepta codul de asociere de 8 caractere de la start_pairing (A-Z, 2-7)",
            "esperava-se o código de pareamento de 8 caracteres de start_pairing (A-Z, 2-7)",
            "esperava-se o código de emparelhamento de 8 carateres de start_pairing (A-Z, 2-7)",
            "nota copulationis 8 characterum ex start_pairing exspectabatur (A-Z, 2-7)",
        ],
    ),
    (
        "the account trusts this certificate already",
        [
            "le compte fait déjà confiance à ce certificat",
            "la cuenta ya confía en este certificado",
            "contul acceptă deja acest certificat",
            "a conta já confia neste certificado",
            "a conta já confia neste certificado",
            "ratio huic testimonio iam confidit",
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
            "{} espera um inteiro não negativo",
            "{} espera um inteiro não negativo",
            "{} numerum integrum non negativum exspectat",
        ],
    ),
    (
        "at most {} bytes",
        [
            "{} octets au maximum",
            "{} bytes como máximo",
            "cel mult {} octeți",
            "no máximo {} bytes",
            "no máximo {} bytes",
            "summum {} octeti",
        ],
    ),
    (
        "at most {} parts",
        [
            "{} parties au maximum",
            "{} partes como máximo",
            "cel mult {} părți",
            "no máximo {} partes",
            "no máximo {} partes",
            "summum {} partes",
        ],
    ),
    (
        "at most {} keys",
        [
            "{} touches au maximum",
            "{} teclas como máximo",
            "cel mult {} taste",
            "no máximo {} teclas",
            "no máximo {} teclas",
            "summum {} claves",
        ],
    ),
    (
        "{} is not an address of the account",
        [
            "{} n’est pas une adresse du compte",
            "{} no es una dirección de la cuenta",
            "{} nu este o adresă a contului",
            "{} não é um endereço da conta",
            "{} não é um endereço da conta",
            "{} inscriptio rationis non est",
        ],
    ),
    (
        "no command is named {}",
        [
            "aucune commande ne s’appelle {}",
            "ningún comando se llama {}",
            "nicio comandă nu se numește {}",
            "nenhum comando se chama {}",
            "nenhum comando se chama {}",
            "nullum mandatum {} appellatur",
        ],
    ),
    (
        "not a key: {}",
        [
            "pas une touche : {}",
            "no es una tecla: {}",
            "nu este o tastă: {}",
            "não é uma tecla: {}",
            "não é uma tecla: {}",
            "non clavis: {}",
        ],
    ),
    (
        "invalid command: {}",
        [
            "commande non valide : {}",
            "comando no válido: {}",
            "comandă nevalidă: {}",
            "comando inválido: {}",
            "comando inválido: {}",
            "mandatum invalidum: {}",
        ],
    ),
    (
        "invalid argument `{}`: {}",
        [
            "argument `{}` non valide : {}",
            "argumento `{}` no válido: {}",
            "argument `{}` nevalid: {}",
            "argumento `{}` inválido: {}",
            "argumento `{}` inválido: {}",
            "argumentum `{}` invalidum: {}",
        ],
    ),
    (
        "invalid replay record on line {}: {}",
        [
            "enregistrement non valide à la ligne {} : {}",
            "registro no válido en la línea {}: {}",
            "înregistrare nevalidă la rândul {}: {}",
            "registro inválido na linha {}: {}",
            "registo inválido na linha {}: {}",
            "inscriptio invalida in versu {}: {}",
        ],
    ),
    (
        "{} {} not found",
        [
            "{} {} introuvable",
            "{} {}: no se encontró",
            "{} {}: negăsit",
            "{} {}: não encontrado",
            "{} {}: não encontrado",
            "{} {}: non inventum",
        ],
    ),
    (
        "{} {} already exists",
        [
            "{} {} existe déjà",
            "{} {}: ya existe",
            "{} {}: există deja",
            "{} {}: já existe",
            "{} {}: já existe",
            "{} {}: iam exstat",
        ],
    ),
    (
        "{} is irreversible; repeat it with `confirm: true`",
        [
            "{} est irréversible ; répétez avec `confirm: true`",
            "{} es irreversible; repítelo con `confirm: true`",
            "{} este ireversibil; repetați cu `confirm: true`",
            "{} é irreversível; repita com `confirm: true`",
            "{} é irreversível; repita com `confirm: true`",
            "{} irrevocabile est; repete cum `confirm: true`",
        ],
    ),
    (
        "account {} has no {} mailbox",
        [
            "le compte {} n’a pas de dossier {}",
            "la cuenta {} no tiene carpeta {}",
            "contul {} nu are dosar {}",
            "a conta {} não tem pasta {}",
            "a conta {} não tem pasta {}",
            "ratio {} capsam {} non habet",
        ],
    ),
    (
        "{} can't be run from {}; the user has to do it in the GUI or the CLI",
        [
            "{} ne peut pas être lancé depuis {} ; l’utilisateur doit le faire dans l’interface graphique ou la ligne de commande",
            "{} no se puede ejecutar desde {}; el usuario debe hacerlo en la interfaz gráfica o en la línea de comandos",
            "{} nu poate fi rulat din {}; utilizatorul trebuie să o facă în interfața grafică sau în linia de comandă",
            "{} não pode ser executado a partir de {}; o usuário deve fazê-lo na interface gráfica ou na linha de comando",
            "{} não pode ser executado a partir de {}; o utilizador tem de o fazer na interface gráfica ou na linha de comandos",
            "{} ex {} exsequi non potest; usor id in interfacie graphica aut in linea mandatorum facere debet",
        ],
    ),
    (
        "draft {} can't be sent: {}",
        [
            "le brouillon {} ne peut pas être envoyé : {}",
            "no se puede enviar el borrador {}: {}",
            "ciorna {} nu poate fi trimisă: {}",
            "o rascunho {} não pode ser enviado: {}",
            "o rascunho {} não pode ser enviado: {}",
            "adumbratio {} mitti non potest: {}",
        ],
    ),
    (
        "“{}” is larger than 25 MiB and cannot be attached",
        [
            "« {} » dépasse 25 Mio et ne peut pas être joint",
            "«{}» supera los 25 MiB y no se puede adjuntar",
            "„{}” depășește 25 MiB și nu poate fi atașat",
            "“{}” tem mais de 25 MiB e não pode ser anexado",
            "“{}” tem mais de 25 MiB e não pode ser anexado",
            "«{}» maius est quam 25 MiB neque adiungi potest",
        ],
    ),
    (
        "the draft's attachments could not be written: {}",
        [
            "les pièces jointes du brouillon n’ont pas pu être écrites : {}",
            "no se pudieron escribir los adjuntos del borrador: {}",
            "atașamentele ciornei nu au putut fi scrise: {}",
            "não foi possível gravar os anexos do rascunho: {}",
            "não foi possível escrever os anexos do rascunho: {}",
            "adiuncta adumbrationis scribi non potuerunt: {}",
        ],
    ),
    (
        "“{}” could not be copied: {}",
        [
            "« {} » n’a pas pu être copié : {}",
            "no se pudo copiar «{}»: {}",
            "„{}” nu a putut fi copiat: {}",
            "não foi possível copiar “{}”: {}",
            "não foi possível copiar “{}”: {}",
            "«{}» exscribi non potuit: {}",
        ],
    ),
    (
        "invalid request: {}",
        [
            "requête non valide : {}",
            "solicitud no válida: {}",
            "cerere nevalidă: {}",
            "solicitação inválida: {}",
            "pedido inválido: {}",
            "petitio invalida: {}",
        ],
    ),
    (
        "runtime: {}",
        [
            "environnement d’exécution : {}",
            "entorno de ejecución: {}",
            "mediu de execuție: {}",
            "ambiente de execução: {}",
            "ambiente de execução: {}",
            "ambitus exsecutionis: {}",
        ],
    ),
    (
        "the profile {} is in use by another dail process that does not accept connections; try again when it has finished",
        [
            "le profil {} est utilisé par un autre processus dail qui n’accepte pas de connexions ; réessayez quand il aura terminé",
            "el perfil {} lo usa otro proceso de dail que no acepta conexiones; reinténtalo cuando haya terminado",
            "profilul {} este folosit de alt proces dail care nu acceptă conexiuni; reîncercați după ce se termină",
            "o perfil {} está em uso por outro processo do dail que não aceita conexões; tente de novo quando ele terminar",
            "o perfil {} está a ser usado por outro processo do dail que não aceita ligações; tente novamente quando terminar",
            "profilum {} ab alio processu dail adhibetur qui conexiones non accipit; iterum conare cum finiverit",
        ],
    ),
    (
        "the profile {} is in use",
        [
            "le profil {} est utilisé",
            "el perfil {} está en uso",
            "profilul {} este în uz",
            "o perfil {} está em uso",
            "o perfil {} está em uso",
            "profilum {} in usu est",
        ],
    ),
    (
        "the {} backend is not in this build",
        [
            "le moteur {} n’est pas dans cette version",
            "el motor {} no está en esta compilación",
            "motorul {} nu este inclus în această versiune",
            "o back-end {} não está nesta versão",
            "o motor {} não está nesta versão",
            "machina {} in hac versione non est",
        ],
    ),
    (
        "demo: {}",
        [
            "démo : {}",
            "demo: {}",
            "demo: {}",
            "demonstração: {}",
            "demonstração: {}",
            "demonstratio: {}",
        ],
    ),
    (
        "secrets: {}",
        [
            "secrets : {}",
            "secretos: {}",
            "secrete: {}",
            "segredos: {}",
            "segredos: {}",
            "secreta: {}",
        ],
    ),
    (
        "no host with handle {}",
        [
            "aucun hôte avec l’identifiant {}",
            "ningún host con el identificador {}",
            "nicio gazdă cu identificatorul {}",
            "nenhum host com o identificador {}",
            "nenhum anfitrião com o identificador {}",
            "nullus hospes cum signo {}",
        ],
    ),
    (
        "debug bridge: {}",
        [
            "pont de débogage : {}",
            "puente de depuración: {}",
            "punte de depanare: {}",
            "ponte de depuração: {}",
            "ponte de depuração: {}",
            "pons purgandi: {}",
        ],
    ),
    (
        "backend `{}` does not support {}",
        [
            "le moteur `{}` ne prend pas en charge {}",
            "el motor `{}` no admite {}",
            "motorul `{}` nu acceptă {}",
            "o back-end `{}` não suporta {}",
            "o motor `{}` não suporta {}",
            "machina `{}` {} non sustinet",
        ],
    ),
    (
        "keyring: {}",
        [
            "trousseau : {}",
            "llavero: {}",
            "portofel de chei: {}",
            "chaveiro: {}",
            "porta-chaves: {}",
            "clavarium: {}",
        ],
    ),
    (
        "TLS setup failed: {}",
        [
            "échec de la configuration TLS : {}",
            "error al configurar TLS: {}",
            "configurarea TLS a eșuat: {}",
            "falha na configuração do TLS: {}",
            "falha na configuração do TLS: {}",
            "constitutio TLS defecit: {}",
        ],
    ),
    (
        "IPC: {}",
        [
            "communication entre processus : {}",
            "comunicación entre procesos: {}",
            "comunicare între procese: {}",
            "comunicação entre processos: {}",
            "comunicação entre processos: {}",
            "communicatio inter processus: {}",
        ],
    ),
    (
        "the manifest signature is malformed: {}",
        [
            "la signature du manifeste est mal formée : {}",
            "la firma del manifiesto está mal formada: {}",
            "semnătura manifestului este incorectă: {}",
            "a assinatura do manifesto está malformada: {}",
            "a assinatura do manifesto está mal formada: {}",
            "signum manifesti male formatum est: {}",
        ],
    ),
    (
        "the manifest is malformed: {}",
        [
            "le manifeste est mal formé : {}",
            "el manifiesto está mal formado: {}",
            "manifestul este incorect: {}",
            "o manifesto está malformado: {}",
            "o manifesto está mal formado: {}",
            "manifestum male formatum est: {}",
        ],
    ),
    (
        "the signature is for {}, the manifest says {}",
        [
            "la signature est pour {}, le manifeste indique {}",
            "la firma es para {}, el manifiesto dice {}",
            "semnătura este pentru {}, manifestul indică {}",
            "a assinatura é para {}, o manifesto diz {}",
            "a assinatura é para {}, o manifesto diz {}",
            "signum pro {} est, manifestum dicit {}",
        ],
    ),
    (
        "the manifest's version {} is not newer than {}",
        [
            "la version {} du manifeste n’est pas plus récente que {}",
            "la versión {} del manifiesto no es más reciente que {}",
            "versiunea {} din manifest nu este mai nouă decât {}",
            "a versão {} do manifesto não é mais recente que {}",
            "a versão {} do manifesto não é mais recente que {}",
            "versio manifesti {} recentior non est quam {}",
        ],
    ),
    (
        "the download has {} bytes, the manifest says {}",
        [
            "le téléchargement fait {} octets, le manifeste indique {}",
            "la descarga tiene {} bytes, el manifiesto dice {}",
            "descărcarea are {} octeți, manifestul indică {}",
            "o download tem {} bytes, o manifesto diz {}",
            "a transferência tem {} bytes, o manifesto diz {}",
            "depressio {} octetos habet, manifestum dicit {}",
        ],
    ),
    (
        "store unavailable: {}",
        [
            "stockage indisponible : {}",
            "almacenamiento no disponible: {}",
            "stocare indisponibilă: {}",
            "armazenamento indisponível: {}",
            "armazenamento indisponível: {}",
            "repositorium non praesto: {}",
        ],
    ),
    (
        "invalid config {}: {}",
        [
            "configuration {} non valide : {}",
            "configuración {} no válida: {}",
            "configurație {} nevalidă: {}",
            "configuração {} inválida: {}",
            "configuração {} inválida: {}",
            "configuratio {} invalida: {}",
        ],
    ),
    (
        "invalid profile name `{}`: use letters, digits, `-` and `_` only",
        [
            "nom de profil `{}` non valide : utilisez uniquement des lettres, des chiffres, `-` et `_`",
            "nombre de perfil `{}` no válido: usa solo letras, dígitos, `-` y `_`",
            "nume de profil `{}` nevalid: folosiți doar litere, cifre, `-` și `_`",
            "nome de perfil `{}` inválido: use só letras, dígitos, `-` e `_`",
            "nome de perfil `{}` inválido: use só letras, dígitos, `-` e `_`",
            "nomen profili `{}` invalidum: solum litteris, numeris, `-` et `_` utere",
        ],
    ),
    (
        "{}: schema version {} is newer than this dail supports ({})",
        [
            "{} : la version de schéma {} est plus récente que celle prise en charge par ce dail ({})",
            "{}: la versión de esquema {} es más reciente que la que admite este dail ({})",
            "{}: versiunea de schemă {} este mai nouă decât cea acceptată de acest dail ({})",
            "{}: a versão de esquema {} é mais recente que a suportada por este dail ({})",
            "{}: a versão de esquema {} é mais recente que a suportada por este dail ({})",
            "{}: versio schematis {} recentior est quam quam hic dail sustinet ({})",
        ],
    ),
    (
        "invalid blob id `{}`",
        [
            "identifiant de blob `{}` non valide",
            "identificador de blob `{}` no válido",
            "identificator de blob `{}` nevalid",
            "identificador de blob `{}` inválido",
            "identificador de blob `{}` inválido",
            "signum blob `{}` invalidum",
        ],
    ),
    (
        "the PDF is larger than {} bytes",
        [
            "le PDF dépasse {} octets",
            "el PDF supera los {} bytes",
            "PDF-ul depășește {} octeți",
            "o PDF tem mais de {} bytes",
            "o PDF tem mais de {} bytes",
            "PDF maius est quam {} octeti",
        ],
    ),
    (
        "the PDF has more than {} pages",
        [
            "le PDF a plus de {} pages",
            "el PDF tiene más de {} páginas",
            "PDF-ul are mai mult de {} pagini",
            "o PDF tem mais de {} páginas",
            "o PDF tem mais de {} páginas",
            "PDF plus quam {} paginas habet",
        ],
    ),
    (
        "unreadable PDF: {}",
        [
            "PDF illisible : {}",
            "PDF ilegible: {}",
            "PDF ilizibil: {}",
            "PDF ilegível: {}",
            "PDF ilegível: {}",
            "PDF illegibile: {}",
        ],
    ),
    (
        "no page {}",
        [
            "pas de page {}",
            "no hay página {}",
            "nu există pagina {}",
            "não há página {}",
            "não há página {}",
            "nulla pagina {}",
        ],
    ),
    (
        "session did not shut down within {}",
        [
            "la session ne s’est pas arrêtée en moins de {}",
            "la sesión no se cerró en menos de {}",
            "sesiunea nu s-a închis în mai puțin de {}",
            "a sessão não foi encerrada em menos de {}",
            "a sessão não terminou em menos de {}",
            "sessio intra {} non clausa est",
        ],
    ),
    (
        "at column {}: {}",
        [
            "à la colonne {} : {}",
            "en la columna {}: {}",
            "la coloana {}: {}",
            "na coluna {}: {}",
            "na coluna {}: {}",
            "in columna {}: {}",
        ],
    ),
    (
        "{} exists already",
        [
            "{} existe déjà",
            "{} ya existe",
            "{} există deja",
            "{} já existe",
            "{} já existe",
            "{} iam exstat",
        ],
    ),
    (
        "message {} is not in the open conversation",
        [
            "le message {} n’est pas dans la discussion ouverte",
            "el mensaje {} no está en la conversación abierta",
            "mesajul {} nu este în conversația deschisă",
            "a mensagem {} não está na conversa aberta",
            "a mensagem {} não está na conversa aberta",
            "epistula {} in colloquio aperto non est",
        ],
    ),
    (
        "account {} has no folder {}",
        [
            "le compte {} n’a pas de dossier {}",
            "la cuenta {} no tiene la carpeta {}",
            "contul {} nu are dosarul {}",
            "a conta {} não tem a pasta {}",
            "a conta {} não tem a pasta {}",
            "ratio {} capsam {} non habet",
        ],
    ),
    (
        "must be between 1 and {}",
        [
            "doit être compris entre 1 et {}",
            "debe estar entre 1 y {}",
            "trebuie să fie între 1 și {}",
            "deve estar entre 1 e {}",
            "deve estar entre 1 e {}",
            "inter 1 et {} esse debet",
        ],
    ),
    (
        "must be between 0 and {} (the number of attachments)",
        [
            "doit être compris entre 0 et {} (le nombre de pièces jointes)",
            "debe estar entre 0 y {} (el número de adjuntos)",
            "trebuie să fie între 0 și {} (numărul de atașamente)",
            "deve estar entre 0 e {} (o número de anexos)",
            "deve estar entre 0 e {} (o número de anexos)",
            "inter 0 et {} esse debet (numerus adiunctorum)",
        ],
    ),
    (
        "the draft has {} attachment(s)",
        [
            "le brouillon a {} pièce(s) jointe(s)",
            "el borrador tiene {} adjunto(s)",
            "ciorna are {} atașament(e)",
            "o rascunho tem {} anexo(s)",
            "o rascunho tem {} anexo(s)",
            "adumbratio {} adiunctum(a) habet",
        ],
    ),
    (
        "expected a folder path like {}",
        [
            "chemin de dossier attendu, par exemple {}",
            "se esperaba una ruta de carpeta como {}",
            "se aștepta o cale de dosar precum {}",
            "esperava-se um caminho de pasta como {}",
            "esperava-se um caminho de pasta como {}",
            "via capsae exspectabatur ut {}",
        ],
    ),
    (
        "required for the {} preset",
        [
            "obligatoire pour le préréglage {}",
            "obligatorio para el preajuste {}",
            "obligatoriu pentru presetarea {}",
            "obrigatório para a predefinição {}",
            "obrigatório para a predefinição {}",
            "necessarium pro praeformato {}",
        ],
    ),
    (
        "this install cannot download the update; open {}",
        [
            "cette installation ne peut pas télécharger la mise à jour ; ouvrez {}",
            "esta instalación no puede descargar la actualización; abre {}",
            "această instalare nu poate descărca actualizarea; deschideți {}",
            "esta instalação não pode baixar a atualização; abra {}",
            "esta instalação não pode transferir a atualização; abra {}",
            "haec institutio renovationem deprimere non potest; aperi {}",
        ],
    ),
    (
        "{} is still being checked; try again",
        [
            "{} est encore en cours de vérification ; réessayez",
            "{} aún se está comprobando; reinténtalo",
            "{} este încă în verificare; reîncercați",
            "{} ainda está sendo verificado; tente de novo",
            "{} ainda está a ser verificado; tente novamente",
            "{} adhuc inspicitur; iterum conare",
        ],
    ),
    (
        "`{}:` needs a value",
        [
            "`{}:` demande une valeur",
            "`{}:` necesita un valor",
            "`{}:` necesită o valoare",
            "`{}:` precisa de um valor",
            "`{}:` precisa de um valor",
            "`{}:` valorem requirit",
        ],
    ),
    (
        "unknown `is:{}`; use is:unread, is:read or is:starred",
        [
            "`is:{}` inconnu ; utilisez is:unread, is:read ou is:starred",
            "`is:{}` desconocido; usa is:unread, is:read o is:starred",
            "`is:{}` necunoscut; folosiți is:unread, is:read sau is:starred",
            "`is:{}` desconhecido; use is:unread, is:read ou is:starred",
            "`is:{}` desconhecido; use is:unread, is:read ou is:starred",
            "`is:{}` ignotum; utere is:unread, is:read aut is:starred",
        ],
    ),
    (
        "unknown `has:{}`; use has:attachment",
        [
            "`has:{}` inconnu ; utilisez has:attachment",
            "`has:{}` desconocido; usa has:attachment",
            "`has:{}` necunoscut; folosiți has:attachment",
            "`has:{}` desconhecido; use has:attachment",
            "`has:{}` desconhecido; use has:attachment",
            "`has:{}` ignotum; utere has:attachment",
        ],
    ),
    (
        "invalid date `{}`; write YYYY-MM-DD",
        [
            "date `{}` non valide ; écrivez AAAA-MM-JJ",
            "fecha `{}` no válida; escribe AAAA-MM-DD",
            "dată `{}` nevalidă; scrieți AAAA-LL-ZZ",
            "data `{}` inválida; escreva AAAA-MM-DD",
            "data `{}` inválida; escreva AAAA-MM-DD",
            "dies `{}` invalidus; scribe AAAA-MM-DD",
        ],
    ),
    (
        "`{}` is not a calendar date",
        [
            "`{}` n’est pas une date du calendrier",
            "`{}` no es una fecha del calendario",
            "`{}` nu este o dată calendaristică",
            "`{}` não é uma data do calendário",
            "`{}` não é uma data do calendário",
            "`{}` dies calendarii non est",
        ],
    ),
    // Last: it matches any text with these words.
    (
        "{} failed: {}",
        [
            "échec de {} : {}",
            "error en {}: {}",
            "{} a eșuat: {}",
            "{} falhou: {}",
            "{} falhou: {}",
            "{} defecit: {}",
        ],
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
    (
        "Reply",
        [
            "Répondre",
            "Responder",
            "Răspuns",
            "Responder",
            "Responder",
            "Responsum",
        ],
    ),
    (
        "Reply all",
        [
            "Répondre à tous",
            "Responder a todos",
            "Răspuns tuturor",
            "Responder a todos",
            "Responder a todos",
            "Responsum omnibus",
        ],
    ),
    (
        "Forward",
        [
            "Transférer",
            "Reenviar",
            "Redirecționare",
            "Encaminhar",
            "Reencaminhar",
            "Transmissio",
        ],
    ),
    (
        "Offer every version again",
        [
            "Proposer de nouveau toutes les versions",
            "Volver a ofrecer todas las versiones",
            "Propunerea din nou a tuturor versiunilor",
            "Oferecer de novo todas as versões",
            "Voltar a oferecer todas as versões",
            "Omnes versiones iterum offerre",
        ],
    ),
    (
        "Change update checks",
        [
            "Modifier la recherche de mises à jour",
            "Cambiar la búsqueda de actualizaciones",
            "Schimbarea căutării actualizărilor",
            "Alterar a verificação de atualizações",
            "Alterar a verificação de atualizações",
            "Inspectionem renovationum mutare",
        ],
    ),
    (
        "Create draft",
        [
            "Créer le brouillon",
            "Crear el borrador",
            "Crearea ciornei",
            "Criar o rascunho",
            "Criar o rascunho",
            "Adumbrationem creare",
        ],
    ),
    (
        "Edit draft",
        [
            "Modifier le brouillon",
            "Editar el borrador",
            "Editarea ciornei",
            "Editar o rascunho",
            "Editar o rascunho",
            "Adumbrationem recensere",
        ],
    ),
    (
        "Discard draft",
        [
            "Supprimer le brouillon",
            "Descartar el borrador",
            "Renunțarea la ciornă",
            "Descartar o rascunho",
            "Rejeitar o rascunho",
            "Adumbrationem abicere",
        ],
    ),
    (
        "Restore draft",
        [
            "Restaurer le brouillon",
            "Restaurar el borrador",
            "Restabilirea ciornei",
            "Restaurar o rascunho",
            "Restaurar o rascunho",
            "Adumbrationem restituere",
        ],
    ),
    (
        "Attach file",
        [
            "Joindre le fichier",
            "Adjuntar el archivo",
            "Atașarea fișierului",
            "Anexar o arquivo",
            "Anexar o ficheiro",
            "Documentum adiungere",
        ],
    ),
    (
        "Remove attachment",
        [
            "Retirer la pièce jointe",
            "Quitar el adjunto",
            "Eliminarea atașamentului",
            "Remover o anexo",
            "Remover o anexo",
            "Adiunctum removere",
        ],
    ),
    (
        "Send",
        ["Envoi", "Envío", "Trimitere", "Envio", "Envio", "Missio"],
    ),
    (
        "Cancel send",
        [
            "Annulation de l’envoi",
            "Cancelación del envío",
            "Anularea trimiterii",
            "Cancelamento do envio",
            "Cancelamento do envio",
            "Rescissio missionis",
        ],
    ),
    (
        "Change the mailbox's address",
        [
            "Changer l’adresse du dossier",
            "Cambiar la dirección de la carpeta",
            "Schimbarea adresei dosarului",
            "Alterar o endereço da pasta",
            "Alterar o endereço da pasta",
            "Inscriptionem capsae mutare",
        ],
    ),
    (
        "Change the sender",
        [
            "Changer l’expéditeur",
            "Cambiar el remitente",
            "Schimbarea expeditorului",
            "Alterar o remetente",
            "Alterar o remetente",
            "Missorem mutare",
        ],
    ),
    (
        "Key preset",
        [
            "Jeu de raccourcis",
            "Preajuste de atajos",
            "Presetare de comenzi rapide",
            "Predefinição de atalhos",
            "Predefinição de atalhos",
            "Praeformatum compendiorum",
        ],
    ),
    ("Theme", ["Thème", "Tema", "Temă", "Tema", "Tema", "Thema"]),
    (
        "Trust the certificate",
        [
            "Faire confiance au certificat",
            "Confiar en el certificado",
            "Acceptarea certificatului",
            "Confiar no certificado",
            "Confiar no certificado",
            "Testimonio confidere",
        ],
    ),
    (
        "Forget the certificate",
        [
            "Oublier le certificat",
            "Olvidar el certificado",
            "Uitarea certificatului",
            "Esquecer o certificado",
            "Esquecer o certificado",
            "Testimonii oblivisci",
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
            "Marcar tudo como lido ",
            "Marcar tudo como lido ",
            "Omnia ut lecta notare ",
        ],
    ),
    (
        "Mark unread ",
        [
            "Marquer comme non lu ",
            "Marcar como no leído ",
            "Marcare ca necitit ",
            "Marcar como não lida ",
            "Marcar como não lida ",
            "Ut non lectum notare ",
        ],
    ),
    (
        "Mark read ",
        [
            "Marquer comme lu ",
            "Marcar como leído ",
            "Marcare ca citit ",
            "Marcar como lida ",
            "Marcar como lida ",
            "Ut lectum notare ",
        ],
    ),
    (
        "Move to Trash ",
        [
            "Mettre à la corbeille ",
            "Mover a la papelera ",
            "Mutare în coșul de gunoi ",
            "Mover para a lixeira ",
            "Mover para o lixo ",
            "In quisquilias movere ",
        ],
    ),
    (
        "Mark as junk ",
        [
            "Signaler comme indésirable ",
            "Marcar como spam ",
            "Marcare ca spam ",
            "Marcar como spam ",
            "Marcar como spam ",
            "Ut purgamentum notare ",
        ],
    ),
    (
        "Not junk ",
        [
            "Pas indésirable ",
            "No es spam ",
            "Nu este spam ",
            "Não é spam ",
            "Não é spam ",
            "Non purgamentum ",
        ],
    ),
    (
        "Archive ",
        [
            "Archiver ",
            "Archivar ",
            "Arhivare ",
            "Arquivar ",
            "Arquivar ",
            "In archivum condere ",
        ],
    ),
    (
        "Move ",
        [
            "Déplacer ",
            "Mover ",
            "Mutare ",
            "Mover ",
            "Mover ",
            "Movere ",
        ],
    ),
    (
        "Unstar ",
        [
            "Ne plus suivre ",
            "Quitar destacado ",
            "Eliminarea stelei ",
            "Remover estrela ",
            "Remover estrela ",
            "Stellam removere ",
        ],
    ),
    (
        "Star ",
        [
            "Suivre ",
            "Destacar ",
            "Marcare cu stea ",
            "Adicionar estrela ",
            "Adicionar estrela ",
            "Stella notare ",
        ],
    ),
    (
        "Add account ",
        [
            "Ajouter le compte ",
            "Añadir la cuenta ",
            "Adăugarea contului ",
            "Adicionar a conta ",
            "Adicionar a conta ",
            "Rationem addere ",
        ],
    ),
    (
        "Edit account ",
        [
            "Modifier le compte ",
            "Editar la cuenta ",
            "Editarea contului ",
            "Editar a conta ",
            "Editar a conta ",
            "Rationem recensere ",
        ],
    ),
    (
        "Show images from ",
        [
            "Afficher les images de ",
            "Mostrar las imágenes de ",
            "Afișarea imaginilor de la ",
            "Mostrar as imagens de ",
            "Mostrar as imagens de ",
            "Imagines monstrare a ",
        ],
    ),
    (
        "Hide images from ",
        [
            "Masquer les images de ",
            "Ocultar las imágenes de ",
            "Ascunderea imaginilor de la ",
            "Ocultar as imagens de ",
            "Ocultar as imagens de ",
            "Imagines celare a ",
        ],
    ),
    (
        "Skip version ",
        [
            "Ignorer la version ",
            "Omitir la versión ",
            "Omiterea versiunii ",
            "Pular a versão ",
            "Ignorar a versão ",
            "Versionem praetermittere ",
        ],
    ),
    (
        "Default offline window for ",
        [
            "Fenêtre hors ligne par défaut pour ",
            "Copia sin conexión predeterminada para ",
            "Copie offline implicită pentru ",
            "Cópia offline padrão para ",
            "Cópia offline predefinida para ",
            "Exemplar sine conexione praesumptum pro ",
        ],
    ),
    (
        "Keep all of ",
        [
            "Tout garder hors ligne : ",
            "Conservar todo sin conexión: ",
            "Păstrare integrală offline: ",
            "Manter tudo offline: ",
            "Manter tudo offline: ",
            "Omnia sine conexione servare: ",
        ],
    ),
    (
        "Keys of ",
        [
            "Touches de ",
            "Teclas de ",
            "Tastele pentru ",
            "Teclas de ",
            "Teclas de ",
            "Claves ",
        ],
    ),
    (
        "Rename ",
        [
            "Renommer ",
            "Cambiar el nombre de ",
            "Redenumire ",
            "Renomear ",
            "Mudar o nome de ",
            "Renominare ",
        ],
    ),
    (
        "Remove ",
        [
            "Retirer ",
            "Quitar ",
            "Eliminare ",
            "Remover ",
            "Remover ",
            "Removere ",
        ],
    ),
    (
        "Unpin ",
        [
            "Détacher ",
            "Desfijar ",
            "Anularea fixării ",
            "Desafixar ",
            "Desafixar ",
            "Refigere ",
        ],
    ),
    (
        "Pin ",
        [
            "Épingler ",
            "Fijar ",
            "Fixare ",
            "Fixar ",
            "Fixar ",
            "Figere ",
        ],
    ),
    (
        "Hide ",
        [
            "Masquer ",
            "Ocultar ",
            "Ascundere ",
            "Ocultar ",
            "Ocultar ",
            "Celare ",
        ],
    ),
    (
        "Show ",
        [
            "Afficher ",
            "Mostrar ",
            "Afișare ",
            "Mostrar ",
            "Mostrar ",
            "Monstrare ",
        ],
    ),
    (
        "Set ",
        [
            "Régler ",
            "Ajustar ",
            "Setare ",
            "Definir ",
            "Definir ",
            "Constituere ",
        ],
    ),
];

/// "Keep the newest {n} offline of {path}".
const KEEP_NEWEST: Tr = [
    "Garder hors ligne les {n} plus récents de {path}",
    "Conservar sin conexión los {n} más recientes de {path}",
    "Păstrarea offline a celor mai noi {n} din {path}",
    "Manter offline as {n} mais recentes de {path}",
    "Manter offline as {n} mais recentes de {path}",
    "{n} recentissimas ex {path} sine conexione servare",
];

/// The column of `lang` in a [`Tr`]; `None` for English and the pseudo-locale.
const fn column(lang: Lang) -> Option<usize> {
    match lang {
        Lang::French => Some(0),
        Lang::Spanish => Some(1),
        Lang::Romanian => Some(2),
        Lang::PortugueseBrazil => Some(3),
        Lang::PortuguesePortugal => Some(4),
        Lang::Latin => Some(5),
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
        "Inbox" => Some([
            "de réception",
            "de entrada",
            "de intrare",
            "de entrada",
            "de entrada",
            "acceptorum",
        ]),
        "Sent" => Some([
            "des messages envoyés",
            "de enviados",
            "pentru trimise",
            "de enviados",
            "de enviados",
            "missorum",
        ]),
        "Drafts" => Some([
            "de brouillons",
            "de borradores",
            "pentru ciorne",
            "de rascunhos",
            "de rascunhos",
            "adumbrationum",
        ]),
        "Trash" => Some([
            "corbeille",
            "de papelera",
            "coș de gunoi",
            "de lixeira",
            "de lixo",
            "quisquiliarum",
        ]),
        "Archive" => Some([
            "d’archives",
            "de archivo",
            "de arhivă",
            "de arquivo",
            "de arquivo",
            "archivi",
        ]),
        "Junk" => Some([
            "de courrier indésirable",
            "de spam",
            "spam",
            "de spam",
            "de spam",
            "purgamenti",
        ]),
        "All" => Some([
            "de tout le courrier",
            "de todo el correo",
            "cu toate e-mailurile",
            "de todos os e-mails",
            "de todo o correio",
            "omnis cursus",
        ]),
        "Flagged" => Some([
            "de messages suivis",
            "de destacados",
            "cu stea",
            "com estrela",
            "com estrela",
            "stellatorum",
        ]),
        _ => None,
    };
    tr.map_or_else(|| role.to_owned(), |tr| tr[col].to_owned())
}

fn entity(col: usize, entity: &str) -> String {
    let tr: Option<Tr> = match entity {
        "account" => Some(["compte", "cuenta", "contul", "conta", "conta", "ratio"]),
        "attachment" => Some([
            "pièce jointe",
            "adjunto",
            "atașamentul",
            "anexo",
            "anexo",
            "adiunctum",
        ]),
        "draft" => Some([
            "brouillon",
            "borrador",
            "ciorna",
            "rascunho",
            "rascunho",
            "adumbratio",
        ]),
        "mailbox" => Some(["dossier", "carpeta", "dosarul", "pasta", "pasta", "capsa"]),
        "message" => Some([
            "message", "mensaje", "mesajul", "mensagem", "mensagem", "epistula",
        ]),
        "thread" => Some([
            "discussion",
            "conversación",
            "conversația",
            "conversa",
            "conversa",
            "colloquium",
        ]),
        "certificate offer" => Some([
            "certificat proposé",
            "certificado ofrecido",
            "certificatul oferit",
            "certificado oferecido",
            "certificado oferecido",
            "testimonium oblatum",
        ]),
        "trusted certificate" => Some([
            "certificat de confiance",
            "certificado de confianza",
            "certificatul acceptat",
            "certificado confiável",
            "certificado aceite",
            "testimonium acceptum",
        ]),
        "device" => Some([
            "appareil",
            "dispositivo",
            "dispozitivul",
            "dispositivo",
            "dispositivo",
            "instrumentum",
        ]),
        "offline rejection" => Some([
            "modification hors ligne refusée",
            "cambio sin conexión rechazado",
            "modificarea offline refuzată",
            "alteração offline recusada",
            "alteração offline recusada",
            "mutatio sine conexione recusata",
        ]),
        "server link" => Some([
            "liaison au serveur",
            "enlace con el servidor",
            "legătura cu serverul",
            "vínculo com o servidor",
            "ligação ao servidor",
            "nexus cum servitore",
        ]),
        "identity" => Some([
            "adresse d’envoi",
            "dirección de envío",
            "adresa de trimitere",
            "endereço de envio",
            "endereço de envio",
            "inscriptio missionis",
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
                "comando inválido: {}",
                "comando inválido: {}",
                "mandatum invalidum: {}",
            ],
            &[&reason_in(col, reason)],
        ),
        CoreError::InvalidArgument { field, reason } => t(
            [
                "argument `{}` non valide : {}",
                "argumento `{}` no válido: {}",
                "argument `{}` nevalid: {}",
                "argumento `{}` inválido: {}",
                "argumento `{}` inválido: {}",
                "argumentum `{}` invalidum: {}",
            ],
            &[field, &reason_in(col, reason)],
        ),
        CoreError::InvalidReplay { line, reason } => t(
            [
                "enregistrement non valide à la ligne {} : {}",
                "registro no válido en la línea {}: {}",
                "înregistrare nevalidă la rândul {}: {}",
                "registro inválido na linha {}: {}",
                "registo inválido na linha {}: {}",
                "inscriptio invalida in versu {}: {}",
            ],
            &[&line.to_string(), reason],
        ),
        CoreError::NotFound { entity: what, id } => t(
            [
                "{} {} introuvable",
                "{} {}: no se encontró",
                "{} {}: negăsit",
                "{} {}: não encontrado",
                "{} {}: não encontrado",
                "{} {}: non inventum",
            ],
            &[&entity(col, what), id],
        ),
        CoreError::AlreadyExists { entity: what, key } => t(
            [
                "{} {} existe déjà",
                "{} {}: ya existe",
                "{} {}: există deja",
                "{} {}: já existe",
                "{} {}: já existe",
                "{} {}: iam exstat",
            ],
            &[&entity(col, what), key],
        ),
        CoreError::ConfirmationRequired { command } => t(
            [
                "{} est irréversible ; répétez avec `confirm: true`",
                "{} es irreversible; repítelo con `confirm: true`",
                "{} este ireversibil; repetați cu `confirm: true`",
                "{} é irreversível; repita com `confirm: true`",
                "{} é irreversível; repita com `confirm: true`",
                "{} irrevocabile est; repete cum `confirm: true`",
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
                "a conta {} não tem pasta {}",
                "a conta {} não tem pasta {}",
                "ratio {} capsam {} non habet",
            ],
            &[&account.to_string(), &role(col, &format!("{which:?}"))],
        ),
        CoreError::OriginNotAllowed { command, origin } => t(
            [
                "{} ne peut pas être lancé depuis {} ; l’utilisateur doit le faire dans l’interface graphique ou la ligne de commande",
                "{} no se puede ejecutar desde {}; el usuario debe hacerlo en la interfaz gráfica o en la línea de comandos",
                "{} nu poate fi rulat din {}; utilizatorul trebuie să o facă în interfața grafică sau în linia de comandă",
                "{} não pode ser executado a partir de {}; o usuário deve fazê-lo na interface gráfica ou na linha de comando",
                "{} não pode ser executado a partir de {}; o utilizador tem de o fazer na interface gráfica ou na linha de comandos",
                "{} ex {} exsequi non potest; usor id in interfacie graphica aut in linea mandatorum facere debet",
            ],
            &[command, &origin.to_string()],
        ),
        CoreError::NotSendable { draft, reason } => t(
            [
                "le brouillon {} ne peut pas être envoyé : {}",
                "no se puede enviar el borrador {}: {}",
                "ciorna {} nu poate fi trimisă: {}",
                "o rascunho {} não pode ser enviado: {}",
                "o rascunho {} não pode ser enviado: {}",
                "adumbratio {} mitti non potest: {}",
            ],
            &[&draft.to_string(), &reason_in(col, reason)],
        ),
        CoreError::NothingToUndo => reason_in(col, "nothing to undo"),
        CoreError::NothingToRedo => reason_in(col, "nothing to redo"),
        CoreError::NotLinked => t(
            [
                "ce profil n’est relié à aucun serveur dail ; lancez d’abord connect_server",
                "este perfil no está enlazado con ningún servidor dail; ejecuta primero connect_server",
                "acest profil nu este legat de un server dail; rulați mai întâi connect_server",
                "este perfil não está vinculado a um servidor dail; execute connect_server primeiro",
                "este perfil não está associado a um servidor dail; execute primeiro connect_server",
                "haec profilis cum nullo servitore dail coniuncta est; prius connect_server exsequere",
            ],
            &[],
        ),
        CoreError::ProfileHasAccounts { accounts } => t(
            [
                "ce profil a {} compte(s) de messagerie ; un profil relié à un serveur dail n’en a aucun, utilisez donc un nouveau profil",
                "este perfil tiene {} cuenta(s) de correo; un perfil enlazado con un servidor dail no tiene ninguna, así que usa un perfil nuevo",
                "acest profil are {} cont(uri) de e-mail; un profil legat de un server dail nu are niciunul, deci folosiți un profil nou",
                "este perfil tem {} conta(s) de e-mail; um perfil vinculado a um servidor dail não tem nenhuma, então use um perfil novo",
                "este perfil tem {} conta(s) de correio; um perfil associado a um servidor dail não tem nenhuma, por isso use um perfil novo",
                "haec profilis {} rationem/rationes epistularum habet; profilis cum servitore dail coniuncta nullam habet, ergo novam profilem adhibe",
            ],
            &[&accounts.to_string()],
        ),
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
        Lang::PortugueseBrazil => commands::portuguese_brazil_help(name).to_owned(),
        Lang::PortuguesePortugal => commands::portuguese_portugal_help(name).to_owned(),
        Lang::Latin => commands::latin_help(name).to_owned(),
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

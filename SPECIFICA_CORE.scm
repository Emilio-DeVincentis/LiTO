#|
    SPECIFICA_CORE.scm - Ancora di Verità

    Questo documento definisce la specifica formale per un ambiente di sviluppo
    distribuito, suckless e agente-centrico.

    Il formato è un ibrido Markdown/S-Expression. Tutto il contenuto
    è racchiuso in commenti e S-Expression di Steel Lisp, rendendo il file
    parsabile da un interprete Steel e allo stesso tempo leggibile da un umano.

    Questo file è la sorgente autoritativa per l'architettura del sistema.
|#

;;;----------------------------------------------------------------------------
;;; ONTOLOGIA DEL SISTEMA
;;;----------------------------------------------------------------------------
#|
    L'ontologia definisce le entità fondamentali e le loro responsabilità.
    Utilizziamo una macro `define-entity` (ipotetica, da implementare in Steel)
    per garantire una struttura formale e ispezionabile dalla macchina.
|#

(define-entity kernel
  (language 'rust)
  (responsibilities
   '(security           ; Garantisce che i tool non accedano a risorse non autorizzate.
     pty                ; Gestisce i terminali virtuali per i processi figli.
     lsp-bridge         ; Converte il protocollo LSP in S-expressions per il Mind.
     )))

(define-entity mind
  (language 'steel)
  (responsibilities
   '(logic-orchestration ; Interpreta gli eventi e decide le azioni da intraprendere.
     repl               ; Fornisce un'interfaccia interattiva per il controllo dell'agente.
     macros             ; Permette l'estensione dinamica del comportamento del sistema.
     )))

(define-entity tool
  (type 'external-cli)
  (description "Rappresenta un programma esterno (es. Kakoune, Lazygit) eseguito come processo figlio, le cui PTY sono controllate dal Kernel e la cui logica è orchestrata dal Mind."))

;;;----------------------------------------------------------------------------
;;; PROTOCOLLO DI COMUNICAZIONE (The Steel-Bus)
;;;----------------------------------------------------------------------------
#|
    Il "Steel-Bus" è il protocollo di comunicazione asincrono che connette
    tutte le entità del sistema. Ogni messaggio è una S-expression, garantendo
    uniformità e facilità di parsing per il Mind.

    Esistono due categorie principali di messaggi: Eventi (dal Kernel al Mind)
    e Comandi (dal Mind al Kernel/Tool).
|#

;; Esempio di Evento: Un errore LSP rilevato dal Kernel
;; (event :source 'lsp :type 'error :payload '(:file "Main.java" :line 80 :message "Missing semicolon"))

;; Esempio di Comando: Il Mind ordina al Kernel di inviare un comando a Kakoune
;; (tool-send 'kakoune "session1" '(:command "echo 'Hello from Steel'"))

;;;----------------------------------------------------------------------------
;;; ARCHITETTURA SUCKLESS
;;;----------------------------------------------------------------------------
#|
    L'architettura del sistema aderisce a due principi "suckless" fondamentali,
    mirati a garantire semplicità, componibilità e longevità.

    1.  **Sostituibilità dei Componenti:** Ogni entità (Kernel, Mind, Tool)
        è un'interfaccia ben definita. Qualsiasi componente che rispetti
        il protocollo "The Steel-Bus" può sostituire un'implementazione
        esistente. Ad esempio, un Kernel in Go potrebbe sostituire quello
        in Rust, o un altro editor di testo potrebbe prendere il posto di Kakoune,
        a patto di avere un adattatore che parli il linguaggio del Bus.

    2.  **Interfaccia Puramente Testuale (TUI)/Buffer-Based:** Il sistema
        non dipende da interfacce grafiche (GUI). L'interazione avviene
        tramite buffer di testo e terminali virtuali (PTY). Il "Mind" agisce
        come un "multiplexer logico", orchestrando e combinando queste
        interfacce testuali in workflow complessi, senza che i singoli
        componenti debbano essere consapevoli l'uno dell'altro.
|#

;;;----------------------------------------------------------------------------
;;; MECCANISMO DI SELF-TESTING RICORSIVO
;;;----------------------------------------------------------------------------
#|
    Il sistema deve essere in grado di auto-validarsi in modo ricorsivo.
    Questo significa che l'Agente (il "Mind" in esecuzione) deve essere capace
    di scrivere, eseguire e validare unit test per ogni modulo del sistema,
    incluso sé stesso.

    Il processo concettuale è il seguente:
    1.  **Generazione del Test:** L'Agente scrive un nuovo test come un file Steel.
    2.  **Esecuzione in un Contesto Pulito:** Il Kernel avvia un'istanza "figlia"
        del sistema (o di un suo sottomodulo) in un ambiente controllato.
    3.  **Simulazione dell'Interazione:** Il test in Steel invia comandi tramite
        il "Steel-Bus" per manipolare i `Tool` (es. `(tool-send 'kakoune ...)`),
        simulando l'input di un utente in un terminale virtuale.
    4.  **Asserzione dello Stato:** Il test ispeziona lo stato dei `Tool`
        (es. il contenuto di un buffer, l'output di un comando) inviando
        richieste di stato sempre tramite il Bus.
    5.  **Validazione:** Il test confronta lo stato osservato con lo stato
        atteso e determina il successo o il fallimento.

    Questo approccio permette al sistema di verificare la corretta integrazione
    di tutti i componenti in modo end-to-end, simulando fedelmente l'uso reale.
|#

;;;----------------------------------------------------------------------------
;;; ROADMAP DI BOOTSTRAP: MINIMO PRODOTTO FUNZIONANTE (MVP)
;;;----------------------------------------------------------------------------
#|
    Per validare l'architettura di base e avviare lo sviluppo iterativo,
    definiamo un Minimo Prodotto Funzionante (MVP) con criteri di successo
    chiari e inequivocabili.
|#

(define-milestone mvp-bootstrap
  (title "Integrazione Kernel-Mind-Tool di base")
  (success-criteria
   '(;; 1. Il Kernel (Rust) deve essere in grado di avviare un processo figlio (Tool).
     (kernel-spawns-tool 'kakoune)

     ;; 2. Il Kernel deve gestire la PTY del Tool, catturando il suo output.
     (kernel-manages-pty-for 'kakoune)

     ;; 3. Il Mind (Steel) deve potersi connettere al Kernel e inviare un comando.
     (mind-sends-command)

     ;; 4. Il comando deve essere specifico per inserire testo in Kakoune.
     ;;    Esempio: (tool-send 'kakoune "session-id" '(insert-text "Hello, World!"))
     (mind-commands-text-insertion)

     ;; 5. Il testo "Hello, World!" deve apparire visibilmente nel buffer di Kakoune.
     (text-appears-in-kakoune-buffer))))

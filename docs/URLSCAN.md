# Integrazione URLScan.io

## Descrizione

L'integrazione URLScan.io permette al bot di analizzare automaticamente il comportamento delle pagine web per identificare potenziali minacce di sicurezza. Utilizzando una sandbox automatizzata, URLScan.io visita il sito web, analizza il contenuto, gli script, i collegamenti esterni e assegna un punteggio di rischio basato sul comportamento osservato.

## Funzionalità

- ✅ **Analisi comportamentale** delle pagine web in sandbox isolata
- ✅ **Rilevamento phishing** e contenuti malevoli
- ✅ **Punteggio di rischio** (Risk Score da 0 a 100)
- ✅ **Scansioni private** - i tuoi URL non sono pubblici
- ✅ **Report dettagliati** con screenshot, risorse caricate e connessioni
- ✅ **Modalità alert-only** - ricevi notifiche solo per link sospetti
- ✅ **Completamente opzionale** - funziona anche senza API key

## Configurazione

### 1. Ottenere una API Key

1. Vai su [URLScan.io](https://urlscan.io/)
2. Crea un account gratuito: [Sign Up](https://urlscan.io/user/signup)
3. Accedi e vai su [Profile](https://urlscan.io/user/profile/)
4. Nella sezione "API Key", copia la tua chiave API

### 2. Configurare il bot

Apri il file `.env` nella root del progetto e aggiungi:

```bash
# --- URLScan.io Integration (Optional) ---
# Web reputation scanning and page behavior analysis
# Get API key: https://urlscan.io/user/signup
URLSCAN_API_KEY=your_api_key_here

# Send URLScan.io messages only for suspicious/malicious URLs (default: true)
URLSCAN_ALERT_ONLY=true
```

Sostituisci `your_api_key_here` con la tua API key.

**Modalità Alert-Only:**

- `true` (default): Il bot invia messaggi solo per URL sospetti/pericolosi (score >= 50 o malicious=true)
- `false`: Il bot invia report dettagliati anche per URL sicuri

### 3. Riavvia il bot

```bash
# Se stai usando Docker/Podman
podman restart clearurlsbot

# Se stai eseguendo il binario direttamente
pkill clear_urls_bot
./target/release/clear_urls_bot
```

## Limiti API

### Account Gratuito

- **Scansioni al minuto**: Circa 2-3 (non documentato ufficialmente)
- **Scansioni al giorno**: ~100-200 (limite soft)
- **Visibilità**: Scansioni private disponibili

### Account Pro/Enterprise

- Limiti più alti e priorità nella coda
- Ulteriori features come API search e automazione avanzata
- Maggiori dettagli su [pagina prezzi](https://urlscan.io/pricing/)

## Come Funziona

1. L'utente invia un messaggio con uno o più URL
2. Il bot estrae tutti gli URL dal messaggio
3. Per ogni URL:
   - **Prima controlla se è già stato scansionato** usando la Search API di URLScan
   - Se trovata una scansione precedente, utilizza quel report (evita scansioni duplicate)
   - Se non trovato, sottomette l'URL a URLScan.io con visibilità "private"
   - Attende il completamento della scansione (polling con retry)
   - Recupera i risultati con punteggio di rischio e flag malicious/potentially malicious
4. Se viene rilevato un URL malevolo (malicious=true OR potentially_malicious):
   - Invia un messaggio di allerta all'utente
   - Mostra il risk score e la classificazione della minaccia
   - Fornisce link al report completo
   - **Stesso comportamento di VirusTotal**: verifica il flag di malware
5. Procede con la pulizia dell'URL dai parametri di tracciamento

## Esempi di Avvisi

### Link Malevolo Rilevato

```text
🚨 ALLERTA SICUREZZA 🚨
━━━━━━━━━━━━━━━━
🌐 URLScan.io Web Reputation

🔴 LINK PERICOLOSO RILEVATO

📊 Analisi Comportamentale:
📈 Risk Score: 75.0/100
🔴 Classificato come: MALICIOUS

🔒 ATTENZIONE: Pagina web sospetta
Potrebbe contenere phishing o malware.

📋 Visualizza Scansione Completa ›
```

### URL Verificato Sicuro (solo se URLSCAN_ALERT_ONLY=false)

```text
✅ URL VERIFICATO
───────────────────
🌐 URLScan.io Web Reputation

✅ COMPLETAMENTE SICURO

📊 Analisi Comportamentale:
📈 Risk Score: 0.0/100
🔍 Status: Nessuna minaccia rilevata

✨ Pagina web verificata sicura
📋 Visualizza Scansione ›
```

## Soglie di Rilevamento

- **Malevolo** (alert): `malicious=true` - URL rilevato come dannoso
- **Sicuro**: `malicious=false` - Nessuna minaccia rilevata

La logica è identica a VirusTotal:

- **VirusTotal**: invia avviso se `malicious > 0`
- **URLScan**: invia avviso se `malicious=true`

Il `score` viene mostrato nel report per riferimento, ma non è usato per decidere se inviare l'avviso.

## Performance

- **Ricerca scansioni precedenti**: ~1-2 secondi (evita duplicati)
- **Submissione nuova scansione**: ~1-2 secondi
- **Polling risultati**: Fino a 4 retry con intervallo di 1.5 secondi (~6 secondi max)
- **Tempo medio totale**: 3-8 secondi (5-8 solo se è una nuova scansione)
- **Vantaggio**: Le scansioni precedenti vengono riutilizzate (niente timeout, risultati istantanei)
- **Parallelizzazione**: Le richieste sono asincrone

## Logica Condivisa con VirusTotal

Sia URLScan che VirusTotal **seguono la stessa logica di rilevamento**:

| Aspetto              | URLScan                             | VirusTotal                      |
| -------------------- | ----------------------------------- | ------------------------------- |
| **Condizione alert** | `malicious=true` | `malicious > 0` |
| **Message format** | Uguale | Uguale |
| **Alert-only mode** | Sì (default) | Sì (default) |
| **Tipo minaccia** | Phishing, comportamenti sospetti | Malware, virus, trojan |

### Differenze tecniche

| Caratteristica  | URLScan.io                             | VirusTotal                        |
| --------------- | -------------------------------------- | --------------------------------- |
| **Analisi** | Comportamentale (sandbox) | Database + 70+ antivirus |
| **Cosa rileva** | Phishing, script malevoli, redirect | Malware, virus, trojan |
| **Report** | Screenshot, risorse, connessioni | Hash file, rilevazioni AV |
| **Privacy** | Scansioni private | Tutti gli URL pubblici |
| **Velocità** | ~5-8 secondi | ~1-2 secondi |

**Raccomandazione:** Usa entrambi per copertura completa!

- VirusTotal: Malware e minacce conosciute
- URLScan: Phishing e comportamenti sospetti

## Troubleshooting

### Il bot non scansiona gli URL

1. Verifica che `URLSCAN_API_KEY` sia configurato nel file `.env`
2. Controlla i log del bot per errori API:

   ```bash
   tail -f bot.log | grep -i urlscan
   ```

### Rate Limit Exceeded

Se vedi errori 429 nei log:

```text
⏱️ URLScan.io: limite richieste raggiunto
Attendi e riprova.
```

**Soluzioni:**

- **Breve termine**: Attendi qualche minuto
- **Lungo termine**: Abilita solo alert-only mode o considera account premium

### Scansioni che non completano

URLScan.io a volte impiega più tempo per siti complessi. Il bot:

- Fa 4 tentativi con intervallo 1.5s (max ~6 secondi)
- Se timeout, restituisce link al report senza score
- La scansione continua in background su URLScan.io

### Non ricevo messaggi per URL sicuri

È normale se `URLSCAN_ALERT_ONLY=true` (default). Per ricevere report anche per URL sicuri:

```bash
URLSCAN_ALERT_ONLY=false
```

## API Documentation

Documentazione ufficiale:

- [URLScan.io API](https://urlscan.io/docs/api/)
- [Search API](https://urlscan.io/docs/search/)
- [Rate Limits](https://urlscan.io/docs/api/#rate-limits)

## Disabilitare l'Integrazione

Rimuovi o commenta la linea `URLSCAN_API_KEY` nel file `.env`:

```bash
# URLSCAN_API_KEY=your_api_key_here
```

Il bot continuerà a funzionare normalmente senza le scansioni URLScan.io.

## Privacy e Sicurezza

### Privacy

- Le scansioni sono **private** (parametro `visibility: "private"`)
- Solo tu puoi vedere i risultati tramite link diretto
- Gli URL **non** appaiono nella ricerca pubblica di URLScan.io
- URLScan.io conserva i dati per scopi di ricerca (anonimizzati)

### Cosa viene analizzato

- Contenuto HTML e risorse caricate (JS, CSS, immagini)
- Collegamenti ipertestuali e redirect
- Certificati SSL/TLS
- Connessioni di rete in uscita
- Esecuzione di script JavaScript

### Nota Importante

URLScan.io **visita effettivamente il sito** in una sandbox. Se l'URL contiene:

- Parametri di sessione unici
- Token di autenticazione
- Link one-time use

Considera di disabilitare URLScan.io per tali messaggi.

## Alternative Self-Hosted

Per massima privacy:

- [PhishTank](https://www.phishtank.com/) - Database phishing gratuito
- [Google Safe Browsing API](https://developers.google.com/safe-browsing) - Rilevamento phishing e malware
- [Wappalyzer](https://www.wappalyzer.com/) - Analisi tecnologie web (no security focus)

## Contribuire

Possibili miglioramenti per l'integrazione URLScan.io:

- [ ] Cache Redis per evitare scansioni duplicate
- [ ] Polling più intelligente (intervalli dinamici)
- [ ] Analisi delle tecnologie rilevate (Wappalyzer-like)
- [ ] Estrazione IOC (Indicators of Compromise) dai report
- [ ] Dashboard con storico scansioni
- [ ] Whitelist di domini fidati (skip scanning)
- [ ] Integrazione con altre threat intelligence platforms

Per contribuire:

1. Fork il repository
2. Crea un branch per la tua feature
3. Implementa miglioramenti
4. Apri una Pull Request

## Supporto

- **Issues GitHub**: [clearurlsbot/issues](https://github.com/yourusername/clearurlsbot/issues)
- **URLScan.io Support**: [support@urlscan.io](mailto:support@urlscan.io)
- **Community**: Contatta [@BugMillennium](https://t.me/BugMillennium) su Telegram

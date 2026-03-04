# Integrazione VirusTotal

## Descrizione

L'integrazione VirusTotal permette al bot di controllare automaticamente tutti gli URL ricevuti per identificare potenziali minacce di sicurezza. Se un URL viene rilevato come dannoso o sospetto da uno o più motori di sicurezza, il bot invierà un avviso all'utente prima di pulire il link.

## Funzionalità

- ✅ **Controllo automatico** di tutti gli URL inviati al bot
- ✅ **Rilevamento malware** tramite 70+ motori antivirus
- ✅ **Avvisi in tempo reale** per link dannosi o sospetti
- ✅ **Statistiche dettagliate** sul numero di rilevamenti
- ✅ **Timeout configurabile** per non rallentare il bot
- ✅ **Completamente opzionale** - funziona anche senza API key

## Configurazione

### 1. Ottenere una API Key

1. Vai su [VirusTotal](https://www.virustotal.com/)
2. Crea un account gratuito o accedi
3. Vai su [My API Key](https://www.virustotal.com/gui/my-apikey)
4. Copia la tua API key

### 2. Configurare il bot

Apri il file `.env` nella root del progetto e aggiungi:

```bash
# --- VirusTotal Integration (Optional) ---
VIRUSTOTAL_API_KEY=your_api_key_here
```

Sostituisci `your_api_key_here` con la tua API key.

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
- **Richieste al minuto**: 4
- **Richieste al giorno**: 500
- **Richieste al mese**: 15.500

Per usage elevato, considera l'upgrade a un piano premium su VirusTotal.

## Come Funziona

1. L'utente invia un messaggio con uno o più URL
2. Il bot estrae tutti gli URL dal messaggio
3. Per ogni URL:
   - Codifica l'URL in formato base64 URL-safe
   - Invia una richiesta GET a VirusTotal API v3
   - Analizza la risposta per rilevamenti malicious/suspicious
4. Se vengono rilevate minacce:
   - Invia un messaggio di avviso all'utente
   - Mostra il numero di motori che hanno rilevato il link
5. Procede con la pulizia dell'URL dai parametri di tracciamento

## Esempi di Avvisi

### Link Dannoso

```
🚨 VirusTotal Allerta
⚠️ Questo link è stato rilevato come dannoso da 15/68 motori di sicurezza.
🔒 Si consiglia di NON aprire questo link.
```

### Link Sospetto

```
⚠️ VirusTotal Avviso
Questo link è stato rilevato come sospetto da 3/68 motori di sicurezza.
⚠️ Procedere con cautela.
```

## Soglie di Rilevamento

- **Dannoso**: `malicious > 0` - Almeno un motore ha rilevato malware
- **Sospetto**: `suspicious > 2` - Più di 2 motori hanno rilevato comportamenti sospetti
- **Sicuro**: Nessuno dei precedenti

## Performance

- **Timeout**: 10 secondi per richiesta
- **Caching**: Non implementato (ogni URL viene controllato)
- **Parallelizzazione**: Le richieste sono asincrone

## Troubleshooting

### Il bot non controlla gli URL

1. Verifica che `VIRUSTOTAL_API_KEY` sia configurato nel file `.env`
2. Controlla i log del bot per errori API:
   ```bash
   tail -f bot.log | grep -i virustotal
   ```

### Rate Limit Exceeded

Se vedi molti errori 429 nei log, hai superato il limite di richieste:

- **Soluzione breve termine**: Attendi qualche minuto
- **Soluzione lungo termine**: Upgrade a un piano premium o implementa il caching

### Falsi Positivi

VirusTotal può occasionalmente segnalare URL legittimi. I motivi includono:

- URL brevi (bit.ly, tinyurl) possono essere sospetti
- Siti con advertising aggressivo
- Siti recentemente hackerati
- Domini nuovi/poco conosciuti

## API Documentation

Documentazione ufficiale API v3:
- [VirusTotal API v3](https://developers.virustotal.com/reference/overview)
- [URLs Endpoint](https://developers.virustotal.com/reference/url)

## Disabilitare l'Integrazione

Semplicemente rimuovi o commenta la linea `VIRUSTOTAL_API_KEY` nel file `.env`:

```bash
# VIRUSTOTAL_API_KEY=your_api_key_here
```

Il bot continuerà a funzionare normalmente senza i controlli VirusTotal.

## Privacy

- Gli URL inviati a VirusTotal diventano **pubblici** nel loro database
- Se stai condividendo URL sensibili/privati, considera di disabilitare questa funzione
- Leggi la [Privacy Policy di VirusTotal](https://support.virustotal.com/hc/en-us/articles/115002168385-Privacy-Policy)

## Alternativa Self-Hosted

Per massima privacy, considera di usare:
- [YARA rules](https://github.com/Yara-Rules/rules) per rilevamento locale
- [ClamAV](https://www.clamav.net/) con database aggiornati
- [URLhaus](https://urlhaus.abuse.ch/) - database pubblico di URL malware

## Contribuire

Se vuoi migliorare l'integrazione VirusTotal:

1. Fork il repository
2. Crea un branch per la tua feature
3. Implementa miglioramenti (es: caching, retry logic)
4. Apri una Pull Request

Possibili miglioramenti:
- [ ] Cache Redis per evitare richieste duplicate
- [ ] Retry automatico con backoff esponenziale
- [ ] Supporto per submission di nuovi URL
- [ ] Dashboard con statistiche rilevamenti
- [ ] Whitelist di domini fidati

# Caching e Riutilizzo delle Scansioni

## Panoramica

Sia **VirusTotal** che **URLScan.io** implementano un sistema di cache intelligente per evitare scansioni duplicate e risparmiare sulla quota API.

Quando un URL viene inviato al bot, il sistema verifica prima se esiste una scansione precedente prima di inviare una nuova richiesta.

## VirusTotal

### Flusso di Ricerca

```
Usuario invia URL
    ↓
[1] GET /api/v3/urls/{encoded_url}
    ↓
    ├─→ 200 OK ✅ (URL trovato)
    │   └─→ Utilizza scansione precedente
    │       Estrae: malicious, suspicious stats
    │       Mostra risultato (avviso o conferma sicurezza)
    │
    └─→ 404 NOT_FOUND (URL non trovato)
        └─→ [2] POST /api/v3/urls (attesa 1.2s)
            └─→ [3] GET /api/v3/urls/{new_id} (riprova)
                └─→ Estrae verdetti e statistiche
                └─→ Mostra risultato
```

### Dettagli Tecnici VirusTotal

- **Metodo di ricerca**: Endpoint diretto `GET /api/v3/urls/{encoded_url}`
  - Base64 URL-safe encoding dell'URL
  - Instant lookup senza API aggiuntive

- **Vantaggi**:
  - ✅ Ricerca istantanea (una singola richiesta)
  - ✅ Nessun overhead di parsing
  - ✅ Riduce quota API significativamente

- **Log**:

  ```text
  VirusTotal: Scansione precedente trovata, utilizzo risultati
  ```

- **Limite API**: 4 richieste/minuto (account gratuito)

---

## URLScan.io

### Flusso di Ricerca URLScan

```text
Usuario invia URL
    ↓
[1] GET /api/v1/search?q=domain:example.com
    ↓
    ├─→ ✅ Risultati trovati (match esatto)
    │   └─→ [2] GET /api/v1/result/{uuid}/
    │       └─→ Estrae: malicious, score, verdict
    │       └─→ Mostra risultato
    │
    └─→ ❌ Nessun risultato trovato
        └─→ [2] POST /api/v1/scan/ (attesa 1.2s)
            └─→ [3] GET /api/v1/result/{uuid}/ × 4 retry
                └─→ Polling fino 6 secondi totali
                └─→ Estrae: malicious, score, verdict
                └─→ Mostra risultato
```

### Dettagli Tecnici URLScan

- **Metodo di ricerca**: Search API `GET /api/v1/search/`
  - Query: `domain:{domain_extracted_from_url}`
  - Matching esatto su URL completo

- **Vantaggi**:
  - ✅ Evita nuove scansioni (riutilizza precedenti)
  - ✅ Riduce latenza (niente polling se già scansionato)
  - ✅ Riduce quota API significativamente

- **Log**:

  ```text
  URLScan.io: Scansione precedente trovata
  URLScan.io: Utilizzando scansione precedente
  ```

- **Limite API**: ~100-200 scansioni/giorno (account gratuito)

---

## Confronto delle Due Strategie

| Aspetto         | VirusTotal                | URLScan                         |
| --------------- | ------------------------- | ------------------------------- |
| **Ricerca** | Endpoint diretto | Search API |
| **Latenza ricerca** | ~50-100ms | ~500ms-1s |
| **Se trovato** | Risultato immediato | GET result immediato |
| **Se non trovato** | POST + wait 1.2s + GET | POST + wait 1.2s + polling 6s |
| **Cache globale** | Sì (tutte le scansioni VT) | Sì (tutte le scansioni URLScan) |

---

## Logging e Monitoraggio

### VirusTotal Logging

**Scansione trovata (reusata)**:

```text
✓ VirusTotal: Scansione precedente trovata, utilizzo risultati
```

**Nuova scansione**:

```text
✓ VirusTotal: URL non presente, invio per analisi
```

### URLScan Logging

**Scansione trovata (reusata)**:

```text
✓ URLScan.io: Scansione precedente trovata
✓ URLScan.io: Utilizzando scansione precedente
```

**Nuova scansione**:

```text
✓ URLScan.io: Scansione in corso...
```

---

## Impatto sulla Quota API

### Scenario: 100 URL in un mese

**Senza Cache** (tutti nuovi):

- VirusTotal: 100 richieste (out of 500/mese)
- URLScan: 100 scansioni (out of ~150/mese) ⚠️ Possibile quota exceeded

**Con Cache** (70% reusati):

- VirusTotal: 30 richieste + 70 ricerche ✅ Ampio margine
- URLScan: 30 scansioni + 70 ricerche ✅ Ampio margine

**Risparmio**: ~70% delle richieste evitate

---

## Best Practices

1. **Non disabilitare il caching**
   - I log di ricerca sono trasparenti
   - Nessun comportamento nascosto

2. **Monitorare i log**

   ```bash
   grep "precedente trovata" /tmp/bot.log
   ```

   - Dovrebbe vedere ~70% di cache hits con uso normale

3. **Per alto volume**
   - Considera account premium su entrambe le piattaforme
   - Aumenta i limiti API quota

---

## Troubleshooting

### "VirusTotal non raggiungibile"

- Bot ha cercato l'URL ma non può raggiungere VirusTotal
- Il sistema ha fallito nel primo tentativo di GET O POST

### "VirusTotal: URL non presente"

- Questo è *normale*
- Significa che l'URL è nuovo per il database VirusTotal
- Viene inviato per l'analisi automaticamente

### "URLScan.io: Scansione in corso..." (ogni volta)

- Significa che la ricerca non ha trovato una scansione precedente
- L'URL è nuovo per URLScan, o la ricerca ha fallito
- Viene sottomesso di nuovo per l'analisi

---

## Architettura Futura

Possibili miglioramenti:

1. **Redis Cache** (locale al bot)
   - Cache di 24 ore per risultati già visti
   - Evita anche ricerche API per URL recenti

2. **Hash-based Caching**
   - Hash URL → risultato precedente
   - Instant fallback senza API

3. **Batch Caching**
   - Pre-scarica risultati noti da VT/URLScan all'avvio
   - Lookup locale rapido

4. **Statistics Dashboard**
   - Cache hit rate
   - Quota API usage
   - URL reputation distribution

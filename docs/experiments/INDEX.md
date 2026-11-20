# Indice degli esperimenti

Esito negativo = la modifica non entra, non = la modifica è dannosa.
Un esito negativo (H0 accettato, o criterio offline non superato) dice
solo che l'effetto misurato non raggiunge la soglia scelta — non
dimostra un peggioramento, e non esclude un effetto reale più piccolo
della soglia o nullo.

| # | Titolo | Tipo | Esito | Branch di archivio | Hash del commit del protocollo |
|---|---|---|---|---|---|
| 01 | SEE calcolata una sola volta in qsearch | Non funzionale (velocità) | Misura NON VALIDA (copia byte-identica fuori dalla soglia di rumore); la modifica non entra. Analisi esplorativa post-hoc: effetto stimato ~0. | `archive/exp-01-see-una-volta` | `7d5686726a36a48333fd835f93844506c1f17f49` |
| 02 | LMR calibrata sulla history delle mosse quiete (C1, K=250) | Funzionale (SPRT) | H0 accettato: nessuna evidenza di un guadagno >= 5 Elo (stima -1,8 ± 5,3, LOS 25,2%). Non dimostra un peggioramento. La modifica non entra. | `archive/exp-02-lmr-history-c1` | `82601d6cc1df9532b5a074d4c8ce6a06308aa7d3` |
| 03 | Mobilità sicura, persistenza del residuo e disaccordo fra reti (F1, F2, S1, S2, S1', S1'-R, 03C) | Analisi offline | F1 e F2: SCARTO; S1: SCARTO (difetto di definizione, corretto da S1', confermato da S1'-R con ICC(1)=0,14); S2: zona grigia; 03C: riduzione 5,27% [2,68%; 7,91%] (PROSEGUIMENTO marginale); nessuna modifica al motore | `archive/exp-03-mobilita-sicura` | `4d2c75e6722d3d3ba8ca3ecb6a895412a5a36e21` |
| 04 | Tempo inutilizzato a 1+1 (ipotesi: mediana dell'orologio ricostruito a mossa 40 >= 25s) | Analisi offline (nessuna partita oltre il lancio di controllo) | IPOTESI NON CONFERMATA: mediana ricostruita 22,10s < 25s | `archive/exp-04-tempo-1piu1` | `4f3f3c2e33106b6d31e0b96e06cd2f5d18e3d43b` |
| 05 | Pacchetto velocità: (i) prefetch TT, (ii) guardia di stop in qsearch, (iii) generatore mirato catture/promozioni | Non funzionale (velocità) | Pacchetto (i+ii+iii) ACCETTATO su Oracle aarch64 (G=1,0436 IC95%=[1,0421;1,0450], con i 3 limiti di 2026-11-18); guardia (ii) poi trovata instabile (criterio 3 di stabilità NON soddisfatto nel blocco 1+0,01: perdite a tempo pacchetto 2 vs riferimento 0) e **esclusa** — solo (i)+(iii) integrati in v4.2.1 (vedi esperimento 06 e `release/v4.2.1-prep`, mergiato in `main` come `13ae75a`); (ii) resta esperimento separato, non reintegrato | `archive/exp-05-pacchetto-velocita` | `6539a0e` |
| 06 | Limite morbido di tempo (soft limit) sempre al 60%, anche sotto i 500ms di allocazione | Funzionale (SPRT) | **AcceptH1**: LLR cumulativo 3,4244 (confine +2,9444), 6.500 partite, elo0=0/elo1=5/alpha=beta=0,05. Stima esplorativa POST HOC (non criterio): +9,3 Elo, IC95% trinomiale [+3,1;+15,5], IC95% bootstrap per coppie di aperture [+4,0;+14,4] — l'SPRT con arresto sul confine tende a sovrastimare l'effetto puntuale, queste stime non vanno usate come criterio. Vale solo per mosse con allocazione <=500ms (10+0,1, Threads=1, Hash=64, aarch64); guadagno reale in partita non misurato. In attesa del via di Daniele per merge/tag/release/bot. | `archive/exp-06-tempo-limite-morbido` | `1615143b2b2e176a833c383b32b86f988a599bab` |

I branch di archivio sono snapshot del momento dell'esperimento: possono contenere versioni di METHOD.md e TEMPLATE.md precedenti a quelle del branch `docs/metodo-e-template`.

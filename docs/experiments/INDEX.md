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

I branch di archivio sono snapshot del momento dell'esperimento: possono contenere versioni di METHOD.md e TEMPLATE.md precedenti a quelle del branch `docs/metodo-e-template`.

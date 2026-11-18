# 05 — Proposta di integrazione, solo (i)+(iii) (PREPARATA, NON unita, NON taggata, nessuna release)

Richiesta da Daniele: una seconda proposta di integrazione, SENZA la
guardia di stop (ii), che resta sul branch esistente
(`exp/05-pacchetto-velocita`) come esperimento separato da
riprogettare (il criterio di stabilità "zero perdite a tempo del
pacchetto oltre il riferimento" si è rivelato mal posto — vedi
docs/experiments/05-pacchetto-velocita.md — e la diagnosi non ha
isolato (ii) dalle altre due modifiche). Branch
`exp/05-proposta-i-iii`, creato da `v4.2.0` (= `80b1648`). **Non
toccato `main`. Nessun `--force`. Nessun merge. Nessun tag. Nessuna
release. Bot non toccato.**

## Composizione: due commit puliti

Cherry-pick dei commit delle ablazioni isolate (i) e (iii) (stessi
commit usati per `exp/05-proposta-integrazione`), **senza (ii)**:

1. `0ee89cb` — (i) solo prefetch della TT in `esegui_mossa`
2. `164f020` — (iii) solo generatore mirato catture/promozioni

Entrambi i cherry-pick si sono applicati **senza conflitti** (un solo
auto-merge testuale su `src/board.rs`/`src/search.rs` per (iii)).

## Verifiche sull'albero finale (dopo i due commit, prima di questo documento)

- `cargo build --release`: compila senza errori.
- `cargo test --release --workspace`: tutti i test passano
  (**43 + 46 = 89 test**, 0 falliti — stesso totale della build
  "pacchetto" su Oracle MENO i 4 test dedicati della guardia (ii),
  assenti qui perché (ii) non è in questo albero).
- Bench a nodi identici (`scripts/bench_suite.py`, depth 18, 1 thread):
  **590.820** (mediogioco) e **980.165** (finale pedoni).
- 100 posizioni a profondità 14 (`pgo_test_fens.txt`, sha256
  `dbf57bc9ce05d925a378013b90c4ba660e78aeae662c04bfcb9695962ddd69ea`)
  via `measure_speed_fase3.py` fra questo binario e il riferimento
  v4.2.0: **0 mismatch** (CSV `exp05_i_iii_gate100.csv`, sha256
  `fe452332296e864ef8914e2f6d034eb54dc244e0e97360f1e2900692dba13e5c`).
- Test di equivalenza del generatore mirato (`examples/check_genera_mosse_catture_promozioni.rs`,
  copiato qui solo per la verifica, NON committato -- il branch resta
  a due commit puliti): stesso seme **20261116** usato nell'esperimento
  05, **100 + 5.000 = 5.100 posizioni, 0 mismatch** di insieme o
  ordine (100 da `pgo_test_fens.txt` + 5.000 da 17 partite di
  auto-gioco).
- `cargo check --target aarch64-unknown-linux-gnu --release`:
  compila senza errori (**nessuna esecuzione, nessuna build nativa,
  nessun accesso a Oracle, bot non toccato** -- solo un controllo di
  compilazione cross locale).

## Hash del binario x86-64

sha256 `5936790b91f0381efc7bfa14abcf0ca15b5bf7632ca105b2b9dda8b92980e577`
(release, locale, Windows x86-64).

## Stato

Proposta pronta per revisione, **senza la guardia di stop (ii)**.
Non unita a `main`, nessun tag, nessuna release, bot non toccato. (ii)
resta un esperimento separato sul branch `exp/05-pacchetto-velocita`,
da riprogettare dopo la diagnosi delle perdite a tempo.

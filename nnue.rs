use std::fs::File;
use std::io::Read;
use crate::board::Scacchiera;

// ============================================================================
// FORMATO FILE: Stockfish NNUE classico, architettura HalfKP 256x2-32-32-1
// ============================================================================
//
// Questa NON è più la rete "giocattolo" a 768 feature piatte delle revisioni
// precedenti: è un parser per il formato binario REALE usato da Stockfish
// (era "nodchip/Stockfish", 2020, la stessa architettura di gran parte dei
// primi net .nnue ancora in circolazione: 41024 feature HalfKP, doppia
// prospettiva, 256 neuroni per prospettiva, poi 512->32->32->1). Le costanti
// sotto (hash, offset, scale bits, FV_SCALE) sono state verificate leggendo
// direttamente il codice sorgente di quella revisione di Stockfish
// (src/nnue/nnue_common.h, features/half_kp.{h,cpp}, nnue_feature_transformer.h,
// layers/{affine_transform,clipped_relu,input_slice}.h), non ricostruite a
// memoria: un errore anche di un solo offset qui non farebbe "fallire" in modo
// visibile il caricamento, produrrebbe semplicemente una valutazione basata su
// pesi disallineati, indistinguibile a occhio da una rete debole.
//
// Limite noto: supporta SOLO questa architettura (HalfKP classico). I file
// .nnue "moderni" di Stockfish (HalfKAv2_hm, layer-stack a bucket, pesi FT
// compressi LEB128) hanno un formato completamente diverso e vengono
// correttamente rifiutati al caricamento (non silenziosamente letti male).

/// Numero di caselle della scacchiera.
const SQUARE_NB: usize = 64;

/// PS_END dell'enum PieceSquareIndex originale (10 * SQUARE_NB + 1): numero
/// di "slot" piece-square per un singolo king-bucket, pedoni compresi ma RE
/// escluso (i re non sono mai feature tracciate in HalfKP, sono usati solo
/// per selezionare il king-bucket). I valori PS_W_PAWN=1, PS_B_PAWN=65, ...,
/// PS_W_QUEEN=513, PS_B_QUEEN=577 derivano dalla stessa formula (vedi
/// `ps_base` sotto) e non sono ripetuti qui singolarmente.
const PS_END: u32 = 10 * SQUARE_NB as u32 + 1; // 641

/// Dimensioni dello spazio di input HalfKP: un blocco di PS_END feature per
/// ciascuna delle 64 possibili caselle del re "associato" (king-bucket).
pub const HALFKP_INPUT_DIMENSIONS: usize = SQUARE_NB * PS_END as usize; // 41024

/// Numero di neuroni del feature transformer PER PROSPETTIVA (kTransformedFeatureDimensions
/// nel codice originale). L'input del primo layer nascosto è il doppio
/// (concatenazione [prospettiva "noi", prospettiva "loro"]).
const L1_SIZE: usize = 256;
/// Larghezza dei due layer nascosti (512->32->32->1).
const L2_SIZE: usize = 32;
const L3_SIZE: usize = 32;

/// Magic number di versione del formato file (kVersion in nnue_common.h).
const SF_NNUE_VERSION: u32 = 0x7AF3_2F16;

/// Numero di bit di shift tra un layer affine (int8) e il successivo
/// (kWeightScaleBits): i pesi dei layer nascosti sono quantizzati con un
/// fattore di scala 2^6=64, quindi la somma pesata va riportata alla scala
/// dell'input con uno shift aritmetico a destra di 6 PRIMA della ClippedReLU
/// successiva. Il feature transformer e il layer di output sono gli unici
/// due punti che NON applicano questo shift (vedi commenti più sotto).
const WEIGHT_SCALE_BITS: u32 = 6;

/// Fattore di conversione finale dall'uscita intera del layer di output ai
/// centipawn (FV_SCALE in nnue_common.h). Aggiornamento: nei file HalfKP
/// classici è l'UNICO punto di conversione in centipawn, non serve nessun
/// OUTPUT_SCALE aggiuntivo come nella vecchia rete "giocattolo".
const FV_SCALE: i32 = 16;

/// Limite di sicurezza per il punteggio finale, per non confondere una
/// valutazione estrema con un punteggio di matto codificato da search.rs
/// (±(MATE_SCORE - ply), MATE_SCORE = 49_000). Vedi ragionamento identico
/// nella revisione precedente di questo file.
pub const NNUE_EVAL_CLAMP: i32 = 15_000;

// ----------------------------------------------------------------------------
// Hash di compatibilità architetturale (informativi, NON usati come gate)
// ----------------------------------------------------------------------------
//
// Stockfish rifiuta un file .nnue se l'hash embedded non corrisponde
// esattamente a quello calcolato dall'architettura compilata. Abbiamo
// ricostruito la stessa catena di hash leggendo il codice sorgente (vedi
// commento in testa al modulo), ma un file .nnue REALE su cui verificarla non
// era disponibile al momento di scrivere questo parser. Per non rischiare di
// rifiutare un file altrimenti valido per un mio errore di trascrizione,
// l'hash viene calcolato e confrontato solo a scopo diagnostico (un
// messaggio "info string" in caso di mismatch): il vero gate di correttezza è
// il controllo strutturale sulla dimensione del file (`expected_len` in
// `parse`), che non dipende da nessuna di queste costanti.
const HALFKP_HASH: u32 = 0x5D69_D5B9 ^ 1; // AssociatedKing::kFriend -> true -> 1
const FT_OUTPUT_DIMS: u32 = (L1_SIZE * 2) as u32; // 512
const FT_HASH: u32 = HALFKP_HASH ^ FT_OUTPUT_DIMS;
const INPUT_SLICE_HASH: u32 = 0xEC42_E90D ^ FT_OUTPUT_DIMS; // Offset = 0

const fn affine_hash(out_dims: u32, prev_hash: u32) -> u32 {
    let mut h: u32 = 0xCC03_DAE4u32.wrapping_add(out_dims);
    h ^= prev_hash >> 1;
    h ^= prev_hash << 31;
    h
}
const fn clipped_relu_hash(prev_hash: u32) -> u32 {
    0x538D_24C7u32.wrapping_add(prev_hash)
}

const LAYER1_HASH: u32 = affine_hash(L2_SIZE as u32, INPUT_SLICE_HASH);
const RELU1_HASH: u32 = clipped_relu_hash(LAYER1_HASH);
const LAYER2_HASH: u32 = affine_hash(L3_SIZE as u32, RELU1_HASH);
const RELU2_HASH: u32 = clipped_relu_hash(LAYER2_HASH);
const OUTPUT_HASH: u32 = affine_hash(1, RELU2_HASH);
const TOP_HASH: u32 = FT_HASH ^ OUTPUT_HASH;

// ----------------------------------------------------------------------------
// Indicizzazione delle feature HalfKP
// ----------------------------------------------------------------------------

/// Orienta una casella secondo la prospettiva: per il nero la scacchiera
/// viene ruotata di 180° (XOR con 63 flippa contemporaneamente i 3 bit di
/// rank e i 3 bit di file, dato che casella = rank*8+file). Per il bianco è
/// un no-op. Corrisponde esattamente a `orient()` in half_kp.cpp.
#[inline(always)]
fn orient(perspective_black: bool, sq: usize) -> usize {
    sq ^ (if perspective_black { 63 } else { 0 })
}

/// Offset base PS_W_<pt>/PS_B_<pt> per un tipo di pezzo (1=Pedone..5=Regina,
/// il Re non è mai passato qui) visto da una data prospettiva: "amico" (stesso
/// colore della prospettiva) usa lo slot W, "nemico" lo slot B. Corrisponde
/// alla tabella `kpp_board_index` di evaluate_nnue.cpp, ristretta ai soli
/// pezzi non-Re (gli unici per cui HalfKP genera feature).
#[inline(always)]
fn ps_base(piece_type: usize, is_friend: bool) -> u32 {
    let slot = 2 * (piece_type as u32 - 1) + if is_friend { 0 } else { 1 };
    1 + slot * SQUARE_NB as u32
}

/// Indice di riga (0..HALFKP_INPUT_DIMENSIONS) nella matrice dei pesi del
/// feature transformer per la feature (prospettiva, casella, tipo di pezzo,
/// amico/nemico, casella del re associato). Corrisponde a `make_index()` in
/// half_kp.cpp: orient(perspective,s) + kpp_board_index[pc][perspective] +
/// PS_END * ksq, con `ksq` già orientato per la stessa prospettiva.
#[inline(always)]
fn feature_index(perspective_black: bool, sq: usize, piece_type: usize, is_friend: bool, ksq_oriented: usize) -> usize {
    orient(perspective_black, sq) + ps_base(piece_type, is_friend) as usize + PS_END as usize * ksq_oriented
}

// ----------------------------------------------------------------------------
// Accumulatore incrementale: DUE metà (una per prospettiva), pre-ClippedReLU
// ----------------------------------------------------------------------------
//
// A differenza della rete "piatta" precedente (una sola prospettiva, feature
// assolute per colore), HalfKP richiede un accumulatore per la prospettiva
// bianca E uno per la prospettiva nera, perché ciascuno usa un king-bucket
// diverso (il proprio re) per indicizzare le stesse feature. Ogni volta che
// il proprio re si muove, l'INTERO king-bucket cambia per quella prospettiva:
// tutte le feature attive vanno reindicizzate, quindi quella metà va
// ricalcolata da zero (vedi `refresh_one_perspective`), mentre l'altra metà
// (il cui re non si è mosso) resta aggiornabile in modo incrementale.
//
// Somma mantenuta in i32 (non i16 con saturating_add come in una prima bozza
// abbandonata): la saturazione non è invertibile, e qui serve che
// add_piece/remove_piece siano l'uno l'esatto inverso dell'altro in ogni
// caso, indipendentemente dall'ampiezza dei pesi caricati.
#[derive(Clone, Copy, Debug)]
pub struct Accumulator {
    pub white: [i32; L1_SIZE],
    pub black: [i32; L1_SIZE],
}

impl Accumulator {
    pub const fn zero() -> Self {
        Accumulator { white: [0i32; L1_SIZE], black: [0i32; L1_SIZE] }
    }
}

impl Default for Accumulator {
    fn default() -> Self { Self::zero() }
}

pub struct LunaNNUE {
    ft_bias: [i16; L1_SIZE],
    /// weights_[feature_index * L1_SIZE + j]: una riga di L1_SIZE pesi per
    /// ciascuna delle HALFKP_INPUT_DIMENSIONS feature. ~21 MB: deve vivere
    /// sull'heap (Vec), non sullo stack né come array embedded nel binario.
    ft_weight: Vec<i16>,
    l1_bias: [i32; L2_SIZE],
    l1_weight: [i8; L2_SIZE * L1_SIZE * 2],
    l2_bias: [i32; L3_SIZE],
    l2_weight: [i8; L3_SIZE * L2_SIZE],
    out_bias: i32,
    out_weight: [i8; L3_SIZE],
}

/// Cursore di lettura little-endian su un buffer in memoria: evita di tirare
/// dentro una dipendenza esterna (byteorder) solo per questo parser una
/// tantum, eseguito al più una volta per avvio del motore.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self { Reader { buf, pos: 0 } }
    fn remaining(&self) -> usize { self.buf.len() - self.pos }

    fn read_u32(&mut self) -> Option<u32> {
        let b = self.buf.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(u32::from_le_bytes(b.try_into().unwrap()))
    }
    fn read_i32(&mut self) -> Option<i32> {
        let b = self.buf.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(i32::from_le_bytes(b.try_into().unwrap()))
    }
    fn read_i16(&mut self) -> Option<i16> {
        let b = self.buf.get(self.pos..self.pos + 2)?;
        self.pos += 2;
        Some(i16::from_le_bytes(b.try_into().unwrap()))
    }
    fn read_i8(&mut self) -> Option<i8> {
        let b = *self.buf.get(self.pos)?;
        self.pos += 1;
        Some(b as i8)
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        if self.remaining() < n { return None; }
        self.pos += n;
        Some(())
    }
}

impl LunaNNUE {
    pub fn load(path: &str) -> Option<Self> {
        let mut file = File::open(path).ok()?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).ok()?;
        Self::parse(&buffer)
    }

    fn parse(buffer: &[u8]) -> Option<Self> {
        let mut r = Reader::new(buffer);

        let version = r.read_u32()?;
        if version != SF_NNUE_VERSION {
            println!(
                "⚠️ NNUE: versione file 0x{:08X} non riconosciuta (attesa 0x{:08X}, formato HalfKP classico 256x2-32-32-1). File ignorato.",
                version, SF_NNUE_VERSION
            );
            return None;
        }
        let top_hash = r.read_u32()?;
        let desc_len = r.read_u32()? as usize;
        r.skip(desc_len)?;

        // --- VALIDAZIONE STRUTTURALE (il vero gate) ---
        // Indipendentemente da qualunque hash: per l'architettura HalfKP
        // 256x2-32-32-1 il numero di byte rimanenti nel file è fissato
        // esattamente da queste dimensioni. Un file di un'architettura
        // diversa (es. le reti moderne HalfKAv2_hm, molto più grandi e con
        // pesi compressi) non potrà mai avere esattamente questa lunghezza:
        // viene rifiutato qui, PRIMA di allocare i ~21 MB della tabella pesi
        // del feature transformer.
        let ft_bytes = 4 + L1_SIZE * 2 + HALFKP_INPUT_DIMENSIONS * L1_SIZE * 2;
        let net_bytes = 4
            + (L2_SIZE * 4 + L2_SIZE * (L1_SIZE * 2))
            + (L3_SIZE * 4 + L3_SIZE * L2_SIZE)
            + (4 + L3_SIZE);
        let expected = ft_bytes + net_bytes;
        if r.remaining() != expected {
            println!(
                "⚠️ NNUE: dimensione inattesa ({} byte rimanenti, attesi {}): non è una rete HalfKP 256x2-32-32-1 compatibile. File ignorato.",
                r.remaining(), expected
            );
            return None;
        }

        if top_hash != TOP_HASH {
            println!(
                "info string NNUE: hash architettura 0x{:08X} diverso da quello calcolato 0x{:08X} (continuo comunque, il controllo di dimensione ha già validato il layout)",
                top_hash, TOP_HASH
            );
        }

        let ft_hash = r.read_u32()?;
        if ft_hash != FT_HASH {
            println!("info string NNUE: hash feature transformer inatteso (continuo comunque)");
        }

        let mut ft_bias = [0i16; L1_SIZE];
        for b in ft_bias.iter_mut() { *b = r.read_i16()?; }

        let mut ft_weight = vec![0i16; HALFKP_INPUT_DIMENSIONS * L1_SIZE];
        for w in ft_weight.iter_mut() { *w = r.read_i16()?; }

        let net_hash = r.read_u32()?;
        if net_hash != OUTPUT_HASH {
            println!("info string NNUE: hash rete inatteso (continuo comunque)");
        }

        let mut l1_bias = [0i32; L2_SIZE];
        for b in l1_bias.iter_mut() { *b = r.read_i32()?; }
        let mut l1_weight = [0i8; L2_SIZE * L1_SIZE * 2];
        for w in l1_weight.iter_mut() { *w = r.read_i8()?; }

        let mut l2_bias = [0i32; L3_SIZE];
        for b in l2_bias.iter_mut() { *b = r.read_i32()?; }
        let mut l2_weight = [0i8; L3_SIZE * L2_SIZE];
        for w in l2_weight.iter_mut() { *w = r.read_i8()?; }

        let out_bias = r.read_i32()?;
        let mut out_weight = [0i8; L3_SIZE];
        for w in out_weight.iter_mut() { *w = r.read_i8()?; }

        if r.remaining() != 0 {
            println!("⚠️ NNUE: {} byte non consumati a fine file (formato inatteso). File ignorato.", r.remaining());
            return None;
        }

        println!("✅ NNUE: rete HalfKP 256x2-32-32-1 caricata ({} feature di input).", HALFKP_INPUT_DIMENSIONS);

        Some(LunaNNUE { ft_bias, ft_weight, l1_bias, l1_weight, l2_bias, l2_weight, out_bias, out_weight })
    }

    #[inline(always)]
    fn add_row(&self, half: &mut [i32; L1_SIZE], row: usize) {
        let off = row * L1_SIZE;
        for i in 0..L1_SIZE {
            half[i] += self.ft_weight[off + i] as i32;
        }
    }
    #[inline(always)]
    fn sub_row(&self, half: &mut [i32; L1_SIZE], row: usize) {
        let off = row * L1_SIZE;
        for i in 0..L1_SIZE {
            half[i] -= self.ft_weight[off + i] as i32;
        }
    }

    /// Ricalcola da zero la metà dell'accumulatore relativa a UNA sola
    /// prospettiva (`perspective_white`), a partire dal bias e da tutti i
    /// pezzi non-Re presenti sulla scacchiera. Va usata quando il re di
    /// quella prospettiva si è appena mosso (l'intero king-bucket cambia,
    /// nessuna feature attiva resta valida) o per l'inizializzazione di una
    /// nuova posizione. `piece_of` deve restituire, per ogni casella
    /// occupata da un pezzo non-Re, la coppia (colore_bianco: bool,
    /// piece_type 1..=5).
    pub fn refresh_one_perspective(&self, half: &mut [i32; L1_SIZE], board: &Scacchiera, perspective_white: bool) {
        for i in 0..L1_SIZE { half[i] = self.ft_bias[i] as i32; }

        let perspective_black = !perspective_white;
        let own_king_sq = king_square(board, perspective_white);
        let ksq = orient(perspective_black, own_king_sq);

        // Pezzo = indice Luna 0..=4 (Pedone..Regina); il Re (indice 5) è
        // escluso dal range stesso del ciclo, non con un controllo esplicito:
        // in HalfKP i re non sono mai feature, solo selettori di king-bucket.
        for p_idx in 0..5 {
            let piece_type = p_idx + 1;
            let mut bb_w = board.pezzi[p_idx] & board.colori[0];
            while bb_w != 0 {
                let sq = bb_w.trailing_zeros() as usize;
                let is_friend = perspective_white; // pezzo bianco, prospettiva bianca => amico
                self.add_row(half, feature_index(perspective_black, sq, piece_type, is_friend, ksq));
                bb_w &= bb_w - 1;
            }
            let mut bb_b = board.pezzi[p_idx] & board.colori[1];
            while bb_b != 0 {
                let sq = bb_b.trailing_zeros() as usize;
                let is_friend = !perspective_white; // pezzo nero, prospettiva bianca => nemico
                self.add_row(half, feature_index(perspective_black, sq, piece_type, is_friend, ksq));
                bb_b &= bb_b - 1;
            }
        }
    }

    /// Ricalcola entrambe le metà da zero. Da usare una sola volta dopo aver
    /// impostato una nuova posizione (nuova partita, FEN, comando UCI
    /// "position"); da lì in avanti board.rs mantiene l'accumulatore con
    /// `add_piece`/`remove_piece` e `refresh_one_perspective` mirato.
    pub fn refresh(&self, board: &Scacchiera) -> Accumulator {
        let mut acc = Accumulator::zero();
        self.refresh_one_perspective(&mut acc.white, board, true);
        self.refresh_one_perspective(&mut acc.black, board, false);
        acc
    }

    /// Aggiorna INCREMENTALMENTE entrambe le prospettive per un pezzo che
    /// compare sulla casella `sq` (colore `color_white`, tipo Luna
    /// `piece_type_luna` 0..=5). Se `piece_type_luna == 5` (Re) la chiamata è
    /// un no-op: il Re non è mai una feature tracciata in HalfKP, il suo
    /// movimento va gestito da board.rs con `refresh_one_perspective` sulla
    /// propria prospettiva, non con questa funzione. `white_ksq`/`black_ksq`
    /// devono essere le caselle dei due re PRIMA delle mutazioni di questa
    /// mossa (per la prospettiva il cui re non si muove in questa mossa,
    /// sono anche le caselle corrette dopo; per l'altra, il valore passato è
    /// irrilevante perché quella metà verrà comunque ricalcolata da zero).
    #[inline]
    pub fn add_piece(&self, acc: &mut Accumulator, color_white: bool, piece_type_luna: usize, sq: usize, white_ksq: usize, black_ksq: usize) {
        if piece_type_luna >= 5 { return; }
        let piece_type = piece_type_luna + 1;
        self.add_row(&mut acc.white, feature_index(false, sq, piece_type, color_white, orient(false, white_ksq)));
        self.add_row(&mut acc.black, feature_index(true, sq, piece_type, !color_white, orient(true, black_ksq)));
    }

    /// Esatto inverso di `add_piece`: da chiamare quando un pezzo (non Re)
    /// SPARISCE dalla casella `sq`.
    #[inline]
    pub fn remove_piece(&self, acc: &mut Accumulator, color_white: bool, piece_type_luna: usize, sq: usize, white_ksq: usize, black_ksq: usize) {
        if piece_type_luna >= 5 { return; }
        let piece_type = piece_type_luna + 1;
        self.sub_row(&mut acc.white, feature_index(false, sq, piece_type, color_white, orient(false, white_ksq)));
        self.sub_row(&mut acc.black, feature_index(true, sq, piece_type, !color_white, orient(true, black_ksq)));
    }

    /// Layer 1+2 (affini, int8, con ClippedReLU e shift di scala) e layer di
    /// uscita (affine, senza ClippedReLU) a partire da un accumulatore già
    /// aggiornato. `side_to_move_white` determina l'ordine di concatenazione
    /// (prospettiva di chi deve muovere prima, dell'avversario dopo): è
    /// esattamente il modo in cui la rete è stata allenata, ed è anche ciò
    /// che rende il risultato già "firmato" correttamente per la convenzione
    /// negamax di search.rs, senza bisogno di un flip di segno finale come
    /// nella vecchia rete a singola prospettiva.
    pub fn evaluate_from_accumulator(&self, acc: &Accumulator, side_to_move_white: bool) -> i32 {
        let (us, them) = if side_to_move_white { (&acc.white, &acc.black) } else { (&acc.black, &acc.white) };

        // --- Trasformazione feature: clamp diretto a [0,127], NESSUNO shift ---
        // (la ClippedReLU del feature transformer opera sulla somma int16
        // grezza, a differenza di quella dei layer nascosti più sotto).
        let mut l1_in = [0u8; L1_SIZE * 2];
        for i in 0..L1_SIZE { l1_in[i] = us[i].clamp(0, 127) as u8; }
        for i in 0..L1_SIZE { l1_in[L1_SIZE + i] = them[i].clamp(0, 127) as u8; }

        // --- Hidden layer 1: 512 -> 32, poi ClippedReLU con shift di scala ---
        // Dot product vettorizzato (AVX2/SSE2, vedi `dot_i8u8` più sotto):
        // è qui che si concentra circa il 94% del costo del forward pass
        // (32 neuroni * 512 pesi ciascuno).
        let mut h1 = [0u8; L2_SIZE];
        for o in 0..L2_SIZE {
            let row = &self.l1_weight[o * L1_SIZE * 2..(o + 1) * L1_SIZE * 2];
            let sum = self.l1_bias[o] + dot_i8u8(&l1_in, row);
            h1[o] = (sum >> WEIGHT_SCALE_BITS).clamp(0, 127) as u8;
        }

        // --- Hidden layer 2: 32 -> 32, poi ClippedReLU con shift di scala ---
        let mut h2 = [0u8; L3_SIZE];
        for o in 0..L3_SIZE {
            let row = &self.l2_weight[o * L2_SIZE..(o + 1) * L2_SIZE];
            let sum = self.l2_bias[o] + dot_i8u8(&h1, row);
            h2[o] = (sum >> WEIGHT_SCALE_BITS).clamp(0, 127) as u8;
        }

        // --- Layer di uscita: 32 -> 1, NESSUNA ClippedReLU dopo ---
        let out = self.out_bias + dot_i8u8(&h2, &self.out_weight);

        // --- Conversione finale in centipawn (unico punto, FV_SCALE=16) ---
        (out / FV_SCALE).clamp(-NNUE_EVAL_CLAMP, NNUE_EVAL_CLAMP)
    }

    /// Valutazione "da zero" (refresh completo + forward pass), per usi
    /// occasionali fuori dal percorso caldo (comando UCI "eval" manuale). NON
    /// va chiamata nel loop di negamax/quiescence: lì è disponibile
    /// l'accumulatore incrementale mantenuto in `Scacchiera::nnue_acc`.
    pub fn evaluate(&self, board: &Scacchiera) -> i32 {
        let acc = self.refresh(board);
        let side_to_move_white = board.turno == crate::board::Colore::Bianco;
        self.evaluate_from_accumulator(&acc, side_to_move_white)
    }
}

/// Casella del re bianco (`perspective_white=true`) o nero.
#[inline(always)]
fn king_square(board: &Scacchiera, perspective_white: bool) -> usize {
    let color_idx = if perspective_white { 0 } else { 1 };
    (board.pezzi[5] & board.colori[color_idx]).trailing_zeros() as usize
}

// ============================================================================
// FORWARD PASS VETTORIZZATO (AVX2 / SSE2, con fallback scalare)
// ============================================================================
//
// Le tre trasformazioni affini del forward pass (512->32, 32->32, 32->1)
// sono tutte la STESSA operazione, ripetuta una volta per neurone di uscita:
// un dot product tra un vettore di attivazioni UNSIGNED (u8, uscita di una
// ClippedReLU, range [0,127]) e una riga di pesi SIGNED (i8), accumulato in
// i32. Il layer 512->32 domina il costo (32*512 = 16384 moltiplicazioni-somma
// contro le 32*32=1024 del secondo layer e le 32 dell'uscita: circa il 94%
// del lavoro totale), ed è quindi la parte su cui la vettorizzazione paga
// davvero, ma per uniformità e semplicità del codice tutte e tre le
// trasformazioni passano dallo stesso kernel `dot_i8u8`.
//
// Per la stessa ragione la funzione più chiamata è quella con la miglior
// resa: non tocchiamo invece il passo di clamp che costruisce `l1_in` (512
// elementi, semplice clamp+cast) né le ClippedReLU tra i layer, perché sono
// O(decine-centinaia) di operazioni scalari banali, non il collo di
// bottiglia misurato.
//
// Selezione dell'implementazione a runtime (non a compile-time): il binario
// deve girare correttamente anche su una CPU senza AVX2, quindi la scelta
// tra i tre percorsi avviene ad ogni chiamata tramite `is_x86_feature_detected!`
// (che internamente memorizza il risultato del CPUID, costo trascurabile) e
// non tramite un `cfg` di compilazione: un binario compilato su una macchina
// con AVX2 ma eseguito su una senza andrebbe altrimenti in crash (istruzione
// illegale) invece di limitarsi a essere più lento.

// Usata come fallback su target non-x86_64 e come riferimento nei test di
// correttezza qui sotto: su x86_64 in build normale (non test) il
// dispatcher non la richiama mai, da cui l'`allow(dead_code)`.
#[allow(dead_code)]
#[inline(always)]
fn dot_i8u8_scalar(input: &[u8], weight: &[i8]) -> i32 {
    debug_assert_eq!(input.len(), weight.len());
    let mut sum = 0i32;
    for i in 0..input.len() {
        sum += input[i] as i32 * weight[i] as i32;
    }
    sum
}

/// Dot product `input`(u8)·`weight`(i8) -> i32, con dispatch a runtime verso
/// AVX2, altrimenti SSE2 (sempre presente su x86_64); su aarch64 (Android)
/// verso NEON (sempre presente sulla baseline AArch64, a differenza di
/// AVX2); su qualunque altro target la versione scalare pura.
#[inline]
fn dot_i8u8(input: &[u8], weight: &[i8]) -> i32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: la feature AVX2 è stata appena verificata disponibile
            // sulla CPU corrente tramite `is_x86_feature_detected!`.
            return unsafe { simd::dot_i8u8_avx2(input, weight) };
        }
        // SSE2 fa parte della baseline dell'architettura x86_64 (garantita
        // dalla specifica stessa, a differenza di AVX2 che è opzionale):
        // nessun controllo a runtime necessario per usarla in sicurezza.
        // SAFETY: SSE2 è sempre disponibile su x86_64.
        return unsafe { simd::dot_i8u8_sse2(input, weight) };
    }
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON fa parte della baseline obbligatoria di AArch64 (a
        // differenza di AVX2 su x86_64): nessun controllo a runtime
        // necessario, esattamente come per SSE2 sopra.
        return unsafe { simd_aarch64::dot_i8u8_neon(input, weight) };
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        dot_i8u8_scalar(input, weight)
    }
}

#[cfg(target_arch = "x86_64")]
mod simd {
    use core::arch::x86_64::*;

    /// Dot product u8·i8 -> i32, 32 byte per iterazione (un registro YMM).
    ///
    /// `_mm256_maddubs_epi16` (VPMADDUBSW) moltiplica byte per byte
    /// unsigned*signed e somma già a coppie adiacenti, producendo 16 lane a
    /// 16 bit; `_mm256_madd_epi16` contro un registro di soli 1 somma a sua
    /// volta le coppie di lane i16 adiacenti in i32, completando la
    /// riduzione: ogni lane i32 finale è la somma di 4 prodotti byte
    /// consecutivi. È lo stesso schema (percorso non-VNNI) usato da
    /// Stockfish in nnue/layers/affine_transform.h per questo identico tipo
    /// di layer.
    ///
    /// La coda (elementi residui oltre l'ultimo blocco di 32) viene gestita
    /// scalarmente: per questa rete (dimensioni 512/32/32, tutte multiple di
    /// 32) non scatta mai, ma la funzione resta corretta per qualunque
    /// lunghezza, incluse quelle non multiple di 32.
    ///
    /// # Safety
    /// Il chiamante deve aver verificato `is_x86_feature_detected!("avx2")`:
    /// su una CPU senza AVX2 queste istruzioni causerebbero un'eccezione
    /// "illegal instruction". `input.len() == weight.len()` deve valere
    /// (non verificato in release da un `assert` per non pagarne il costo
    /// ad ogni chiamata sul percorso caldo, ma controllato via
    /// `debug_assert_eq!`).
    #[target_feature(enable = "avx2")]
    pub unsafe fn dot_i8u8_avx2(input: &[u8], weight: &[i8]) -> i32 {
        debug_assert_eq!(input.len(), weight.len());
        let len = input.len();
        let chunks = len / 32;

        let ones = _mm256_set1_epi16(1);
        let mut acc = _mm256_setzero_si256();

        for c in 0..chunks {
            let off = c * 32;
            // Caricamento non allineato: gli slice arrivano da array su
            // stack dimensionati per il layer (es. [u8; 512]), senza
            // garanzia di allineamento a 32 byte, quindi `loadu`.
            let a = _mm256_loadu_si256(input.as_ptr().add(off) as *const __m256i);
            let b = _mm256_loadu_si256(weight.as_ptr().add(off) as *const __m256i);
            let prod16 = _mm256_maddubs_epi16(a, b);
            let prod32 = _mm256_madd_epi16(prod16, ones);
            acc = _mm256_add_epi32(acc, prod32);
        }

        let mut sum = hsum_epi32_avx2(acc);
        for i in (chunks * 32)..len {
            sum += input[i] as i32 * weight[i] as i32;
        }
        sum
    }

    /// Riduzione orizzontale delle 8 lane i32 di un `__m256i` a un unico
    /// scalare: dimezza ripetutamente la larghezza (256->128->64->32 bit).
    #[target_feature(enable = "avx2")]
    unsafe fn hsum_epi32_avx2(v: __m256i) -> i32 {
        let lo = _mm256_castsi256_si128(v);
        let hi = _mm256_extracti128_si256(v, 1);
        let sum128 = _mm_add_epi32(lo, hi);
        let shuf1 = _mm_shuffle_epi32(sum128, 0b01_00_11_10); // metà alta/bassa a 64 bit scambiate
        let sum64 = _mm_add_epi32(sum128, shuf1);
        let shuf2 = _mm_shuffle_epi32(sum64, 0b00_00_00_01); // metà alta/bassa a 32 bit scambiate
        let sum32 = _mm_add_epi32(sum64, shuf2);
        _mm_cvtsi128_si32(sum32)
    }

    /// Dot product u8·i8 -> i32, 16 byte per iterazione (un registro XMM),
    /// per CPU senza AVX2 (SSE2 è garantita su qualunque x86_64).
    ///
    /// SSE2 non ha un equivalente di `maddubs` (arrivato solo con SSSE3):
    /// l'estensione a 16 bit va fatta a mano, separatamente per operando.
    /// `input` è unsigned: zero-extend (`unpack` con un registro di zeri
    /// come byte alto di ciascuna lane). `weight` è signed: serve invece
    /// un'estensione di SEGNO, ottenuta duplicando come byte alto il segno
    /// stesso (`_mm_cmpgt_epi8(zero, b)` produce 0xFF dove `b<0`, 0x00
    /// altrimenti). Una volta portati entrambi a 16 bit con il segno
    /// corretto, `_mm_madd_epi16` moltiplica e somma a coppie in un solo
    /// passo, esattamente come fa Stockfish nel proprio fallback SSE2 non-SSSE3.
    ///
    /// # Safety
    /// SSE2 è sempre disponibile su x86_64 per specifica di piattaforma:
    /// nessuna verifica di feature richiesta al chiamante. Resta `unsafe`
    /// solo perché invoca intrinsics SIMD dirette.
    #[target_feature(enable = "sse2")]
    pub unsafe fn dot_i8u8_sse2(input: &[u8], weight: &[i8]) -> i32 {
        debug_assert_eq!(input.len(), weight.len());
        let len = input.len();
        let chunks = len / 16;

        let zero = _mm_setzero_si128();
        let mut acc = _mm_setzero_si128();

        for c in 0..chunks {
            let off = c * 16;
            let a = _mm_loadu_si128(input.as_ptr().add(off) as *const __m128i);
            let b = _mm_loadu_si128(weight.as_ptr().add(off) as *const __m128i);

            let a_lo = _mm_unpacklo_epi8(a, zero);
            let a_hi = _mm_unpackhi_epi8(a, zero);

            let b_sign = _mm_cmpgt_epi8(zero, b);
            let b_lo = _mm_unpacklo_epi8(b, b_sign);
            let b_hi = _mm_unpackhi_epi8(b, b_sign);

            acc = _mm_add_epi32(acc, _mm_madd_epi16(a_lo, b_lo));
            acc = _mm_add_epi32(acc, _mm_madd_epi16(a_hi, b_hi));
        }

        let shuf1 = _mm_shuffle_epi32(acc, 0b01_00_11_10);
        let sum64 = _mm_add_epi32(acc, shuf1);
        let shuf2 = _mm_shuffle_epi32(sum64, 0b00_00_00_01);
        let sum32 = _mm_add_epi32(sum64, shuf2);
        let mut sum = _mm_cvtsi128_si32(sum32);

        for i in (chunks * 16)..len {
            sum += input[i] as i32 * weight[i] as i32;
        }
        sum
    }
}

#[cfg(target_arch = "aarch64")]
mod simd_aarch64 {
    use core::arch::aarch64::*;

    /// Dot product u8·i8 -> i32, 16 byte per iterazione (un registro NEON a
    /// 128 bit), per il target Android/ARM64 del torneo MCEC.
    ///
    /// Stesso schema del fallback SSE2 (nessun equivalente diretto di
    /// `maddubs`/VNNI disponibile senza le estensioni opzionali `dotprod`/
    /// `i8mm`, che non tutti i telefoni garantiscono): si estendono
    /// entrambi gli operandi a 16 bit prima di moltiplicare. `input` è
    /// unsigned in [0,127] (uscita di una ClippedReLU): l'estensione a
    /// 16 bit con zero-fill (`vmovl_u8`) non può mai impostare il bit di
    /// segno, quindi reinterpretare il risultato come `int16x8_t` è sempre
    /// corretto. `weight` è signed: `vmovl_s8` estende already col segno
    /// giusto. La moltiplicazione-accumulo widening (`vmlal_s16`, 4 lane
    /// per chiamata) fa il resto in un solo passo, come `_mm_madd_epi16`.
    ///
    /// La riduzione orizzontale finale usa `vaddvq_s32`, un'istruzione
    /// nativa AArch64 (non disponibile su ARMv7) che somma le 4 lane in un
    /// colpo, più semplice della sequenza di shuffle richiesta su x86.
    ///
    /// # Safety
    /// NEON è parte obbligatoria della baseline AArch64 (specifica ARM),
    /// quindi nessuna verifica di feature è richiesta al chiamante — a
    /// differenza di AVX2 su x86_64. Resta `unsafe` solo perché invoca
    /// intrinsics SIMD dirette. `input.len() == weight.len()` deve valere
    /// (controllato via `debug_assert_eq!`, non in release per non pagarne
    /// il costo sul percorso caldo).
    #[target_feature(enable = "neon")]
    pub unsafe fn dot_i8u8_neon(input: &[u8], weight: &[i8]) -> i32 {
        debug_assert_eq!(input.len(), weight.len());
        let len = input.len();
        let chunks = len / 16;

        let mut acc = vdupq_n_s32(0);

        for c in 0..chunks {
            let off = c * 16;
            let a = vld1q_u8(input.as_ptr().add(off));
            let b = vld1q_s8(weight.as_ptr().add(off));

            let a_lo = vreinterpretq_s16_u16(vmovl_u8(vget_low_u8(a)));
            let a_hi = vreinterpretq_s16_u16(vmovl_u8(vget_high_u8(a)));
            let b_lo = vmovl_s8(vget_low_s8(b));
            let b_hi = vmovl_s8(vget_high_s8(b));

            acc = vmlal_s16(acc, vget_low_s16(a_lo), vget_low_s16(b_lo));
            acc = vmlal_s16(acc, vget_high_s16(a_lo), vget_high_s16(b_lo));
            acc = vmlal_s16(acc, vget_low_s16(a_hi), vget_low_s16(b_hi));
            acc = vmlal_s16(acc, vget_high_s16(a_hi), vget_high_s16(b_hi));
        }

        let mut sum = vaddvq_s32(acc);
        for i in (chunks * 16)..len {
            sum += input[i] as i32 * weight[i] as i32;
        }
        sum
    }
}

#[cfg(test)]
mod simd_tests {
    use super::*;

    /// xorshift64 minimale per generare byte pseudo-random deterministici
    /// senza dipendere dal crate `rand` in questo test.
    fn xorshift_bytes_u8(seed: u64, n: usize) -> Vec<u8> {
        let mut s = seed ^ 0x9E3779B97F4A7C15;
        (0..n).map(|_| {
            s ^= s << 13; s ^= s >> 7; s ^= s << 17;
            (s % 128) as u8 // range [0,127], come le attivazioni post-ClippedReLU reali
        }).collect()
    }
    fn xorshift_bytes_i8(seed: u64, n: usize) -> Vec<i8> {
        let mut s = seed ^ 0xBF58476D1CE4E5B9;
        (0..n).map(|_| {
            s ^= s << 13; s ^= s >> 7; s ^= s << 17;
            (s % 256) as u8 as i8 // intera gamma i8, come pesi quantizzati reali
        }).collect()
    }

    /// Confronta scalare vs SIMD su varie lunghezze, incluse quelle NON
    /// multiple della larghezza del registro (32 per AVX2, 16 per SSE2):
    /// per questa rete (512/32/32) la coda non scatta mai, ma il kernel deve
    /// restare corretto in generale.
    #[test]
    fn dot_product_simd_matches_scalar() {
        for &len in &[0usize, 1, 3, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129, 512] {
            for seed in 0u64..5 {
                let input = xorshift_bytes_u8(seed * 1000 + len as u64, len);
                let weight = xorshift_bytes_i8(seed * 2000 + len as u64, len);

                let scalar = dot_i8u8_scalar(&input, &weight);

                if is_x86_feature_detected!("avx2") {
                    let avx2 = unsafe { simd::dot_i8u8_avx2(&input, &weight) };
                    assert_eq!(scalar, avx2, "AVX2 disallineato dallo scalare per len={len}, seed={seed}");
                }

                let sse2 = unsafe { simd::dot_i8u8_sse2(&input, &weight) };
                assert_eq!(scalar, sse2, "SSE2 disallineato dallo scalare per len={len}, seed={seed}");

                #[cfg(target_arch = "aarch64")]
                {
                    let neon = unsafe { simd_aarch64::dot_i8u8_neon(&input, &weight) };
                    assert_eq!(scalar, neon, "NEON disallineato dallo scalare per len={len}, seed={seed}");
                }

                let dispatched = dot_i8u8(&input, &weight);
                assert_eq!(scalar, dispatched, "dispatcher disallineato dallo scalare per len={len}, seed={seed}");
            }
        }
    }
}

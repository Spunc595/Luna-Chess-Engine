use std::fs::File;
use std::io::Read;
use crate::board::Scacchiera;

// ============================================================================
// FILE FORMAT: Akimbo-style king-bucketed HalfKA network
// ============================================================================
//
// Replaces the previous classic Stockfish HalfKP 256x2-32-32-1 parser
// entirely (that format, and every network for it, is gone from this
// codebase — see git history if it's ever needed again). Ported from
// jw1912/akimbo (Rust, MIT license, https://github.com/jw1912/akimbo),
// chosen after evaluating several modern open-source NNUE architectures:
// it's a real generational upgrade over classic HalfKP (adds king-bucket
// selection + horizontal mirroring, the core conceptual jump that defines
// the whole HalfKA family Stockfish itself moved to in 2021) while staying
// small and simple enough to port from scratch in one session — no threat
// features, no multiple hidden layers, no output buckets, no factorized
// training-only weight paths, unlike Reckless/Viridithas (both examined
// and rejected for this reason, on top of Reckless's own network having no
// stated license). Architecture, constants and feature-indexing formulas
// below are read directly from akimbo's `src/network.rs`/`src/position.rs`
// (not reconstructed from memory), and `resources/net.bin` in this repo is
// akimbo's own actual trained network file, used as-is under its MIT
// license (see the copyright notice this project's README carries for it).
//
// Architecture: (768 inputs x 4 king buckets, horizontally mirrored) x 2
// perspectives -> 1024 hidden (SCReLU) -> 1 output. 768 = 6 piece types x
// 2 colors x 64 squares (a flat, non-compound feature space, unlike
// HalfKP's PS_END-per-king-square scheme) — note the King IS a tracked
// feature here (pc=5), unlike HalfKP where it was excluded entirely and
// only ever used to select a king-bucket.

/// Hidden layer width per perspective (kHalfDimensions equivalent). The
/// accumulator holds one array of this size per perspective; the output
/// layer's two dot products each run over the full HIDDEN elements (no
/// concatenation into a single doubled-width buffer like the old
/// ClippedReLU int8 pipeline needed).
const HIDDEN: usize = 1024;

/// Number of king buckets PER PERSPECTIVE, after horizontal mirroring (own
/// king always mapped onto files a-d). Chosen by akimbo's own `BUCKETS`
/// table below, not a tunable here: changing this number without also
/// changing that table (and retraining/re-sourcing the network) would
/// silently misalign every feature index.
const NUM_BUCKETS: usize = 4;

/// Final centipawn scale factor, quantization factors for the
/// accumulator (QA) and the output layer (QB) — same names/values as
/// akimbo's own `consts`, since they're baked into how `resources/net.bin`
/// was trained/quantized and must match exactly, not independently
/// tunable constants of this codebase.
const SCALE: i32 = 400;
const QA: i32 = 255;
const QB: i32 = 64;
const QAB: i32 = QA * QB;

/// Safety limit for the final score, so as not to confuse an extreme
/// evaluation with a mate score encoded by search.rs (±(MATE_SCORE -
/// ply), MATE_SCORE = 49_000).
pub const NNUE_EVAL_CLAMP: i32 = 15_000;

/// King-bucket lookup table, indexed by (already horizontally-mirrored,
/// i.e. file forced into 0..=3) king square. Copied verbatim from
/// akimbo's `network.rs`: entries at file 4..=7 are dead/unreachable (the
/// mirroring step in `get_bucket` always maps the queried square's file
/// into 0..=3 first), kept only because that's how the source table is
/// written.
#[rustfmt::skip]
const BUCKETS: [usize; 64] = [
    0, 0, 1, 1, 5, 5, 4, 4,
    2, 2, 2, 2, 6, 6, 6, 6,
    3, 3, 3, 3, 7, 7, 7, 7,
    3, 3, 3, 3, 7, 7, 7, 7,
    3, 3, 3, 3, 7, 7, 7, 7,
    3, 3, 3, 3, 7, 7, 7, 7,
    3, 3, 3, 3, 7, 7, 7, 7,
    3, 3, 3, 3, 7, 7, 7, 7,
];

// ----------------------------------------------------------------------------
// Feature indexing
// ----------------------------------------------------------------------------

/// XOR mask that orients a square for one perspective: always mirrors the
/// FILE if that perspective's own king sits on files e-h (`own_ksq % 8 >
/// 3`, so the king ends up on a-d after mirroring — halves the number of
/// king-buckets actually needed), and for the BLACK perspective additionally
/// flips the RANK unconditionally (`^ 56`), the standard "view the board
/// from your own side" convention. Both bits are independent (file mirror
/// touches only the low 3 bits, rank flip only the high 3), so applying
/// this single combined mask to any square reference — the king's own
/// square for bucket selection, or any piece's square for its feature
/// index — is equivalent to applying them in either order.
#[inline(always)]
fn perspective_flip(perspective_black: bool, own_ksq: usize) -> usize {
    let file_flip = if own_ksq % 8 > 3 { 7 } else { 0 };
    let rank_flip = if perspective_black { 56 } else { 0 };
    file_flip ^ rank_flip
}

/// Which of the `NUM_BUCKETS` king-buckets this perspective's own king
/// (`own_ksq`, NOT yet oriented) selects.
#[inline(always)]
fn get_bucket(perspective_black: bool, own_ksq: usize) -> usize {
    BUCKETS[own_ksq ^ perspective_flip(perspective_black, own_ksq)]
}

/// Base row offset (before adding the piece's own oriented square) into
/// the `768 * NUM_BUCKETS`-row feature-weight table for a piece of color
/// `piece_white` and type `pc` (Luna's own 0=Pawn..5=King indexing,
/// unchanged from the old HalfKP module — matches akimbo's own convention
/// once its `Piece` enum's PAWN..KING range is enumerated from 0, verified
/// directly against akimbo's `fill_diff`, not assumed), as seen from
/// `perspective_black`'s point of view with its own king on `own_ksq`.
/// Each king-bucket occupies a block of 768 rows: the first 384 (6 piece
/// types x 64 squares) for the perspective's OWN pieces, the next 384 for
/// the opponent's — `is_own_piece` below picks which half.
#[inline(always)]
fn get_base_index(perspective_black: bool, piece_white: bool, pc: usize, own_ksq: usize) -> usize {
    let bucket = get_bucket(perspective_black, own_ksq);
    // Own piece iff its color matches which side this perspective belongs
    // to: white piece + white perspective, or black piece + black
    // perspective.
    let is_own_piece = piece_white != perspective_black;
    768 * bucket + if is_own_piece { 0 } else { 384 } + 64 * pc
}

/// Full feature-table row index for a piece of color `piece_white` and
/// type `pc` sitting on `piece_sq`, as seen from `perspective_black`'s
/// point of view with its own king on `own_ksq`.
#[inline(always)]
fn feature_index(perspective_black: bool, own_ksq: usize, piece_white: bool, pc: usize, piece_sq: usize) -> usize {
    let base = get_base_index(perspective_black, piece_white, pc, own_ksq);
    base + (piece_sq ^ perspective_flip(perspective_black, own_ksq))
}

// ----------------------------------------------------------------------------
// Incremental accumulator: TWO halves (one per perspective)
// ----------------------------------------------------------------------------
//
// i16 (not i32 like the old HalfKP module): matches akimbo's own
// accumulator type exactly, which matters here because we're using THEIR
// trained weights — their quantization was calibrated assuming this exact
// arithmetic width. `wrapping_add`/`wrapping_sub` (not plain `+=`/`-=`)
// make the update exactly invertible even in the (practically unreachable,
// but not analytically impossible) case of transient wraparound, and avoid
// any risk of a debug-build overflow panic — release builds wrap silently
// either way, so this only removes a footgun, it doesn't change behavior
// where it already worked.
#[derive(Clone, Copy, Debug)]
pub struct Accumulator {
    pub white: [i16; HIDDEN],
    pub black: [i16; HIDDEN],
}

impl Accumulator {
    pub const fn zero() -> Self {
        Accumulator { white: [0i16; HIDDEN], black: [0i16; HIDDEN] }
    }
}

impl Default for Accumulator {
    fn default() -> Self { Self::zero() }
}

pub struct LunaNNUE {
    /// `feature_weights[row * HIDDEN + j]`: one row of HIDDEN weights per
    /// feature, `768 * NUM_BUCKETS` rows total (~6MB): heap-allocated
    /// (Vec), not embedded as a stack/binary-resident array, for the same
    /// reason the old ~21MB HalfKP table was — far too large to be a
    /// struct field copied around or placed on the stack.
    feature_weights: Vec<i16>,
    feature_bias: [i16; HIDDEN],
    /// Output layer weights: `[0]` pairs with the SIDE-TO-MOVE's own
    /// accumulator half, `[1]` with the opponent's — matches akimbo's
    /// `Network::out(boys, opps)` convention exactly.
    output_weights: [[i16; HIDDEN]; 2],
    output_bias: i16,
}

/// Little-endian read cursor over an in-memory buffer (same minimal
/// pattern as the old HalfKP parser: no external dependency needed for a
/// one-off parse run at most once per engine startup).
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self { Reader { buf, pos: 0 } }

    fn read_i16(&mut self) -> Option<i16> {
        let b = self.buf.get(self.pos..self.pos + 2)?;
        self.pos += 2;
        Some(i16::from_le_bytes(b.try_into().unwrap()))
    }
}

/// Embedded by default (not behind a feature flag, unlike the old ~21MB
/// HalfKP net): at ~6MB this is small enough to always bundle directly
/// into the binary, permanently eliminating the "external NNUE file wasn't
/// placed correctly next to the executable" failure class — the very
/// likely cause of a previous MCEC tournament result with zero wins (see
/// `LunaNNUE::load_embedded` below and main.rs's loading order) — instead
/// of only fixing it for builds that opt in.
static EMBEDDED_NNUE: &[u8] = include_bytes!("../resources/net.bin");

impl LunaNNUE {
    pub fn load(path: &str) -> Option<Self> {
        let mut file = File::open(path).ok()?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).ok()?;
        Self::parse(&buffer)
    }

    /// Parses the network baked into the binary at compile time (see
    /// `EMBEDDED_NNUE`). Always available, no feature flag required.
    pub fn load_embedded() -> Option<Self> {
        Self::parse(EMBEDDED_NNUE)
    }

    fn parse(buffer: &[u8]) -> Option<Self> {
        // --- STRUCTURAL VALIDATION (the real gate) ---
        // Unlike the old HalfKP format, akimbo's net.bin has no self-
        // describing header (no magic number, no version field: it's a
        // raw dump of a `#[repr(C)]` struct) — so the file's exact byte
        // length is the ONLY available structural check, computed here
        // from the architecture's own dimensions rather than hardcoded
        // separately, so it can never silently drift out of sync with the
        // parsing logic below. The real file (and any correctly-shaped
        // one) is 6,297,664 bytes: the sum of every field below, PLUS 62
        // bytes of trailing alignment padding (the source struct carries
        // `#[repr(C, align(64))]` on its Accumulator type, which pads the
        // whole struct's size up to a multiple of 64) — intentionally
        // read past by the byte count below but never interpreted.
        let feature_weight_count = 768 * NUM_BUCKETS * HIDDEN;
        let expected_payload = feature_weight_count * 2 // feature_weights
            + HIDDEN * 2                                 // feature_bias
            + 2 * HIDDEN * 2                              // output_weights
            + 2;                                          // output_bias
        const TRAILING_PADDING: usize = 62;
        if buffer.len() != expected_payload + TRAILING_PADDING {
            println!(
                "⚠️ NNUE: unexpected size ({} bytes, expected {}): not a compatible network. File ignored.",
                buffer.len(), expected_payload + TRAILING_PADDING
            );
            return None;
        }

        let mut r = Reader::new(buffer);

        let mut feature_weights = vec![0i16; feature_weight_count];
        for w in feature_weights.iter_mut() { *w = r.read_i16()?; }

        let mut feature_bias = [0i16; HIDDEN];
        for b in feature_bias.iter_mut() { *b = r.read_i16()?; }

        let mut output_weights = [[0i16; HIDDEN]; 2];
        for half in output_weights.iter_mut() {
            for w in half.iter_mut() { *w = r.read_i16()?; }
        }

        let output_bias = r.read_i16()?;
        // The remaining 62 bytes of trailing padding are intentionally
        // left unread (`r.pos` simply stops here) — the length check
        // above already guarantees the file is exactly the right size.

        println!(
            "✅ NNUE: loaded ({} input features, {} king buckets, {} hidden neurons).",
            768 * NUM_BUCKETS, NUM_BUCKETS, HIDDEN
        );

        Some(LunaNNUE { feature_weights, feature_bias, output_weights, output_bias })
    }

    #[inline(always)]
    fn add_row(&self, half: &mut [i16; HIDDEN], row: usize) {
        let off = row * HIDDEN;
        for i in 0..HIDDEN {
            half[i] = half[i].wrapping_add(self.feature_weights[off + i]);
        }
    }
    #[inline(always)]
    fn sub_row(&self, half: &mut [i16; HIDDEN], row: usize) {
        let off = row * HIDDEN;
        for i in 0..HIDDEN {
            half[i] = half[i].wrapping_sub(self.feature_weights[off + i]);
        }
    }

    /// Recomputes from scratch the accumulator half relative to ONE single
    /// perspective (`perspective_white`), starting from the bias and every
    /// piece present on the board — INCLUDING the King now (pc=5), unlike
    /// the old HalfKP module: in this architecture the king is a regular
    /// tracked feature (it's only ALSO used, on top of that, to pick the
    /// king-bucket for the perspective it belongs to). Must be used
    /// whenever that perspective's own king moves (its entire king-bucket
    /// changes, invalidating every active feature at once) or to
    /// initialize a new position.
    pub fn refresh_one_perspective(&self, half: &mut [i16; HIDDEN], board: &Scacchiera, perspective_white: bool) {
        for i in 0..HIDDEN { half[i] = self.feature_bias[i]; }

        let perspective_black = !perspective_white;
        let own_ksq = king_square(board, perspective_white);

        for p_idx in 0..6 {
            let mut bb_w = board.pezzi[p_idx] & board.colori[0];
            while bb_w != 0 {
                let sq = bb_w.trailing_zeros() as usize;
                self.add_row(half, feature_index(perspective_black, own_ksq, true, p_idx, sq));
                bb_w &= bb_w - 1;
            }
            let mut bb_b = board.pezzi[p_idx] & board.colori[1];
            while bb_b != 0 {
                let sq = bb_b.trailing_zeros() as usize;
                self.add_row(half, feature_index(perspective_black, own_ksq, false, p_idx, sq));
                bb_b &= bb_b - 1;
            }
        }
    }

    /// Recomputes both halves from scratch. Used only once after setting
    /// up a new position (new game, FEN, UCI "position" command); from
    /// then on board.rs maintains the accumulator incrementally with
    /// `add_piece`/`remove_piece` and a targeted `refresh_one_perspective`
    /// on king moves.
    pub fn refresh(&self, board: &Scacchiera) -> Accumulator {
        let mut acc = Accumulator::zero();
        self.refresh_one_perspective(&mut acc.white, board, true);
        self.refresh_one_perspective(&mut acc.black, board, false);
        acc
    }

    /// INCREMENTALLY updates both perspectives for a piece that appears on
    /// square `sq` (color `color_white`, Luna type `piece_type_luna`
    /// 0..=5, King included). `white_ksq`/`black_ksq` must be the squares
    /// of the two kings BEFORE this move's mutations. Unlike the old
    /// HalfKP module there is no early-return for the King: it IS a
    /// regular tracked feature now. When the piece being moved is the
    /// King itself, board.rs still calls this (it doesn't special-case the
    /// call site) and then separately calls `refresh_nnue_perspective` for
    /// the moving side's OWN perspective right after — whatever this
    /// function wrote into that specific half is entirely overwritten by
    /// that full refresh, so it's harmless for it to use a now-stale
    /// king-bucket there; the OTHER perspective (whose own king didn't
    /// move) is unaffected and genuinely needs this incremental update, so
    /// skipping the call altogether for King pieces would be wrong here,
    /// unlike in HalfKP where the King carried no feature information for
    /// either perspective.
    #[inline]
    pub fn add_piece(&self, acc: &mut Accumulator, color_white: bool, piece_type_luna: usize, sq: usize, white_ksq: usize, black_ksq: usize) {
        self.add_row(&mut acc.white, feature_index(false, white_ksq, color_white, piece_type_luna, sq));
        self.add_row(&mut acc.black, feature_index(true, black_ksq, color_white, piece_type_luna, sq));
    }

    /// Exact inverse of `add_piece`: to be called when a piece DISAPPEARS
    /// from square `sq`.
    #[inline]
    pub fn remove_piece(&self, acc: &mut Accumulator, color_white: bool, piece_type_luna: usize, sq: usize, white_ksq: usize, black_ksq: usize) {
        self.sub_row(&mut acc.white, feature_index(false, white_ksq, color_white, piece_type_luna, sq));
        self.sub_row(&mut acc.black, feature_index(true, black_ksq, color_white, piece_type_luna, sq));
    }

    /// Output layer: SCReLU-activated dot product against each perspective
    /// half plus bias, scaled to centipawns. `side_to_move_white`
    /// determines which half is "us" (paired with `output_weights[0]`) and
    /// which is "them" (`output_weights[1]`) — exactly how the network was
    /// trained, and what makes the result already correctly signed for
    /// search.rs's negamax convention.
    pub fn evaluate_from_accumulator(&self, acc: &Accumulator, side_to_move_white: bool) -> i32 {
        let (us, them) = if side_to_move_white { (&acc.white, &acc.black) } else { (&acc.black, &acc.white) };

        let sum = flatten(us, &self.output_weights[0]) + flatten(them, &self.output_weights[1]);
        let out = sum / QA + self.output_bias as i32;

        (out * SCALE / QAB).clamp(-NNUE_EVAL_CLAMP, NNUE_EVAL_CLAMP)
    }
}

/// Square of the white king (`perspective_white=true`) or black.
#[inline(always)]
fn king_square(board: &Scacchiera, perspective_white: bool) -> usize {
    let color_idx = if perspective_white { 0 } else { 1 };
    (board.pezzi[5] & board.colori[color_idx]).trailing_zeros() as usize
}

// ============================================================================
// OUTPUT LAYER: SCReLU-activated dot product (AVX2 / NEON, scalar fallback)
// ============================================================================
//
// Squared Clipped ReLU: `screlu(x) = clamp(x, 0, QA)^2`, applied to each
// accumulator element before multiplying by its output weight and summing
// — the single nonlinearity of this architecture (no intermediate hidden
// layers, unlike the old HalfKP module's 512->32->32->1 chain). Same
// runtime-dispatch pattern already used by the old module for its dot
// product kernel: a compile-time `cfg(target_arch)` picks the instruction
// set family, and on x86_64 a runtime `is_x86_feature_detected!` check
// decides AVX2 vs the scalar fallback, so a binary built on an AVX2
// machine still runs correctly (just slower) on one without it.

#[allow(dead_code)]
#[inline(always)]
fn flatten_scalar(acc: &[i16; HIDDEN], weights: &[i16; HIDDEN]) -> i32 {
    let mut sum = 0i32;
    for i in 0..HIDDEN {
        let clamped = i32::from(acc[i].clamp(0, QA as i16));
        sum += clamped * clamped * i32::from(weights[i]);
    }
    sum
}

#[inline]
fn flatten(acc: &[i16; HIDDEN], weights: &[i16; HIDDEN]) -> i32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: the AVX2 feature was just verified as available.
            return unsafe { simd::flatten_avx2(acc, weights) };
        }
        return flatten_scalar(acc, weights);
    }
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is part of the mandatory AArch64 baseline: no
        // runtime check needed, same reasoning as the old module's
        // dot_i8u8_neon.
        return unsafe { simd_aarch64::flatten_neon(acc, weights) };
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        flatten_scalar(acc, weights)
    }
}

#[cfg(target_arch = "x86_64")]
mod simd {
    use super::{HIDDEN, QA};
    use core::arch::x86_64::*;

    /// Ported from akimbo's own AVX2 `flatten`: clamps 16 i16 lanes at a
    /// time to `[0, QA]`, then computes `clamped * (clamped * weight)` in
    /// two steps — `_mm256_mullo_epi16(v, w)` first (a TRUNCATING 16-bit
    /// multiply, i.e. `v*w` truncated to its low 16 bits, not widened),
    /// then `_mm256_madd_epi16(v, that)` multiplies `v` against it again
    /// (this time widening to i32) and horizontally sums adjacent lanes,
    /// completing `clamped^2 * weight` reduced to i32 in one instruction
    /// per 16-lane chunk. The intermediate truncation only stays lossless
    /// because real trained weight magnitudes are small (a consequence of
    /// the QB=64 quantization scale): see the precision note on the
    /// `flatten_simd_matches_scalar` test below for why synthetic test
    /// data must respect that same bound.
    ///
    /// # Safety
    /// Caller must have verified `is_x86_feature_detected!("avx2")`.
    #[target_feature(enable = "avx2")]
    pub unsafe fn flatten_avx2(acc: &[i16; HIDDEN], weights: &[i16; HIDDEN]) -> i32 {
        const CHUNK: usize = 16;

        let mut sum = _mm256_setzero_si256();
        let min = _mm256_setzero_si256();
        let max = _mm256_set1_epi16(QA as i16);

        for i in 0..HIDDEN / CHUNK {
            let off = i * CHUNK;
            let mut v = _mm256_loadu_si256(acc.as_ptr().add(off).cast());
            v = _mm256_min_epi16(_mm256_max_epi16(v, min), max);
            let w = _mm256_loadu_si256(weights.as_ptr().add(off).cast());
            let product = _mm256_madd_epi16(v, _mm256_mullo_epi16(v, w));
            sum = _mm256_add_epi32(sum, product);
        }

        horizontal_sum_i32(sum)
    }

    #[inline]
    unsafe fn horizontal_sum_i32(sum: __m256i) -> i32 {
        let upper_128 = _mm256_extracti128_si256::<1>(sum);
        let lower_128 = _mm256_castsi256_si128(sum);
        let sum_128 = _mm_add_epi32(upper_128, lower_128);
        let upper_64 = _mm_unpackhi_epi64(sum_128, sum_128);
        let sum_64 = _mm_add_epi32(upper_64, sum_128);
        let upper_32 = _mm_shuffle_epi32::<0b00_00_00_01>(sum_64);
        let sum_32 = _mm_add_epi32(upper_32, sum_64);
        _mm_cvtsi128_si32(sum_32)
    }
}

#[cfg(target_arch = "aarch64")]
mod simd_aarch64 {
    use super::{HIDDEN, QA};
    use core::arch::aarch64::*;

    /// NEON port of the same AVX2 scheme above, for the Android/ARM64
    /// target of the MCEC tournament: clamp 8 i16 lanes to `[0, QA]`,
    /// square via a self-multiply, widen-multiply-accumulate against the
    /// weight lane directly into i32 via `vmlal_s16` (4 lanes per call, so
    /// two calls per 8-lane chunk to cover the low/high halves).
    ///
    /// # Safety
    /// NEON is a mandatory part of the AArch64 baseline: no feature check
    /// required from the caller, same reasoning as the old module's
    /// dot_i8u8_neon.
    #[target_feature(enable = "neon")]
    pub unsafe fn flatten_neon(acc: &[i16; HIDDEN], weights: &[i16; HIDDEN]) -> i32 {
        const CHUNK: usize = 8;

        let min = vdupq_n_s16(0);
        let max = vdupq_n_s16(QA as i16);
        let mut sum = vdupq_n_s32(0);

        for i in 0..HIDDEN / CHUNK {
            let off = i * CHUNK;
            let v = vminq_s16(vmaxq_s16(vld1q_s16(acc.as_ptr().add(off)), min), max);
            let w = vld1q_s16(weights.as_ptr().add(off));
            // Truncating 16-bit multiply (v*w), matching AVX2's
            // `_mm256_mullo_epi16`: see the safety/precision note on
            // `flatten_avx2` above, same reasoning applies here.
            let vw = vmulq_s16(v, w);

            sum = vmlal_s16(sum, vget_low_s16(v), vget_low_s16(vw));
            sum = vmlal_s16(sum, vget_high_s16(v), vget_high_s16(vw));
        }

        vaddvq_s32(sum)
    }
}

#[cfg(test)]
mod simd_tests {
    use super::*;

    /// Minimal xorshift64 to generate deterministic pseudo-random i16
    /// values in `[-range, range)`, without depending on the `rand` crate
    /// for this test.
    fn xorshift_i16(seed: u64, range: i16) -> [i16; HIDDEN] {
        let mut out = [0i16; HIDDEN];
        let mut s = seed ^ 0x9E3779B97F4A7C15;
        for o in out.iter_mut() {
            s ^= s << 13; s ^= s >> 7; s ^= s << 17;
            *o = (s % (2 * range as u64)) as i16 - range;
        }
        out
    }

    /// Compares scalar vs SIMD flatten across a few seeds. Accumulator
    /// values are generated over a wide range (to meaningfully exercise
    /// both sides of the `[0, QA]` clamp), but weights are kept small
    /// (`|w| < 120`): both the AVX2 (`_mm256_mullo_epi16`) and NEON
    /// (`vmulq_s16`) kernels compute the intermediate `clamped * weight`
    /// product with a TRUNCATING 16-bit multiply before widening it via
    /// the final multiply-accumulate step — ported verbatim from akimbo's
    /// own implementation, which relies on real trained weight magnitudes
    /// staying well within this bound (a consequence of the QB=64
    /// quantization scale) so the truncation never actually loses
    /// information in practice. Large synthetic weights (close to i16::MAX)
    /// would make `clamped(<=255) * weight` overflow 16 bits and diverge
    /// from the scalar reference — a property of the SIMD kernel's design
    /// given realistic inputs, not a bug in either implementation, so the
    /// test must respect the same bound the real network's weights do
    /// instead of exercising a case that can't occur with actual data.
    #[test]
    fn flatten_simd_matches_scalar() {
        for seed in 0u64..5 {
            let acc = xorshift_i16(seed * 1000, 400);
            let weights = xorshift_i16(seed * 2000 + 1, 120);

            let scalar = flatten_scalar(&acc, &weights);

            #[cfg(target_arch = "x86_64")]
            if is_x86_feature_detected!("avx2") {
                let avx2 = unsafe { simd::flatten_avx2(&acc, &weights) };
                assert_eq!(scalar, avx2, "AVX2 mismatched from scalar for seed={seed}");
            }

            #[cfg(target_arch = "aarch64")]
            {
                let neon = unsafe { simd_aarch64::flatten_neon(&acc, &weights) };
                assert_eq!(scalar, neon, "NEON mismatched from scalar for seed={seed}");
            }

            let dispatched = flatten(&acc, &weights);
            assert_eq!(scalar, dispatched, "dispatcher mismatched from scalar for seed={seed}");
        }
    }
}

use std::fs::File;
use std::io::Read;
use crate::board::Scacchiera;

// ============================================================================
// FILE FORMAT: classic Stockfish NNUE, HalfKP 256x2-32-32-1 architecture
// ============================================================================
//
// This is NO LONGER the "toy" network with 768 flat features from previous
// revisions: it's a parser for the REAL binary format used by Stockfish
// (it was "nodchip/Stockfish", 2020, the same architecture used by most of
// the early .nnue nets still circulating: 41024 HalfKP features, dual
// perspective, 256 neurons per perspective, then 512->32->32->1). The
// constants below (hash, offsets, scale bits, FV_SCALE) were verified by
// reading directly the source code of that Stockfish revision
// (src/nnue/nnue_common.h, features/half_kp.{h,cpp}, nnue_feature_transformer.h,
// layers/{affine_transform,clipped_relu,input_slice}.h), not reconstructed
// from memory: an error even in a single offset here would not make loading
// "fail" visibly, it would simply produce an evaluation based on misaligned
// weights, indistinguishable by eye from a weak network.
//
// Known limitation: supports ONLY this architecture (classic HalfKP). The
// "modern" Stockfish .nnue files (HalfKAv2_hm, bucketed layer-stacks,
// LEB128-compressed FT weights) have a completely different format and are
// correctly rejected at load time (not silently misread).

/// Number of squares on the board.
const SQUARE_NB: usize = 64;

/// PS_END from the original PieceSquareIndex enum (10 * SQUARE_NB + 1):
/// number of piece-square "slots" for a single king-bucket, pawns included
/// but the KING excluded (kings are never tracked features in HalfKP, they
/// are only used to select the king-bucket). The values PS_W_PAWN=1,
/// PS_B_PAWN=65, ..., PS_W_QUEEN=513, PS_B_QUEEN=577 derive from the same
/// formula (see `ps_base` below) and are not repeated here individually.
const PS_END: u32 = 10 * SQUARE_NB as u32 + 1; // 641

/// Dimensions of the HalfKP input space: a block of PS_END features for
/// each of the 64 possible squares of the "associated" king (king-bucket).
pub const HALFKP_INPUT_DIMENSIONS: usize = SQUARE_NB * PS_END as usize; // 41024

/// Number of feature transformer neurons PER PERSPECTIVE (kTransformedFeatureDimensions
/// in the original code). The input of the first hidden layer is double
/// this (concatenation of ["us" perspective, "them" perspective]).
const L1_SIZE: usize = 256;
/// Width of the two hidden layers (512->32->32->1).
const L2_SIZE: usize = 32;
const L3_SIZE: usize = 32;

/// File format version magic number (kVersion in nnue_common.h).
const SF_NNUE_VERSION: u32 = 0x7AF3_2F16;

/// Number of shift bits between one affine (int8) layer and the next
/// (kWeightScaleBits): the hidden layer weights are quantized with a scale
/// factor of 2^6=64, so the weighted sum must be brought back to the input
/// scale with an arithmetic right shift of 6 BEFORE the following
/// ClippedReLU. The feature transformer and the output layer are the only
/// two points that do NOT apply this shift (see comments further below).
const WEIGHT_SCALE_BITS: u32 = 6;

/// Final conversion factor from the output layer's integer output to
/// centipawns (FV_SCALE in nnue_common.h). Update: in classic HalfKP files
/// this is the ONLY centipawn conversion point, no additional OUTPUT_SCALE
/// is needed as in the old "toy" network.
const FV_SCALE: i32 = 16;

/// Safety limit for the final score, so as not to confuse an extreme
/// evaluation with a mate score encoded by search.rs
/// (±(MATE_SCORE - ply), MATE_SCORE = 49_000). See identical reasoning
/// in the previous revision of this file.
pub const NNUE_EVAL_CLAMP: i32 = 15_000;

// ----------------------------------------------------------------------------
// Architectural compatibility hashes (informational, NOT used as a gate)
// ----------------------------------------------------------------------------
//
// Stockfish rejects an .nnue file if the embedded hash doesn't match
// exactly the one computed from the compiled architecture. We reconstructed
// the same hash chain by reading the source code (see the comment at the
// top of the module), but a REAL .nnue file to verify it against was not
// available at the time this parser was written. To avoid risking the
// rejection of an otherwise valid file due to a transcription error on my
// part, the hash is computed and compared only for diagnostic purposes (an
// "info string" message on mismatch): the real correctness gate is the
// structural check on the file size (`expected_len` in `parse`), which does
// not depend on any of these constants.
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
// HalfKP feature indexing
// ----------------------------------------------------------------------------

/// Orients a square according to perspective: for black the board is
/// rotated 180° (XOR with 63 flips the 3 rank bits and the 3 file bits at
/// the same time, since square = rank*8+file). For white it's a no-op.
/// Corresponds exactly to `orient()` in half_kp.cpp.
#[inline(always)]
fn orient(perspective_black: bool, sq: usize) -> usize {
    sq ^ (if perspective_black { 63 } else { 0 })
}

/// Base offset PS_W_<pt>/PS_B_<pt> for a piece type (1=Pawn..5=Queen, the
/// King is never passed here) as seen from a given perspective: "friend"
/// (same color as the perspective) uses the W slot, "enemy" uses the B
/// slot. Corresponds to the `kpp_board_index` table in evaluate_nnue.cpp,
/// restricted to non-King pieces only (the only ones for which HalfKP
/// generates features).
#[inline(always)]
fn ps_base(piece_type: usize, is_friend: bool) -> u32 {
    let slot = 2 * (piece_type as u32 - 1) + if is_friend { 0 } else { 1 };
    1 + slot * SQUARE_NB as u32
}

/// Row index (0..HALFKP_INPUT_DIMENSIONS) in the feature transformer's
/// weight matrix for the feature (perspective, square, piece type,
/// friend/enemy, associated king square). Corresponds to `make_index()` in
/// half_kp.cpp: orient(perspective,s) + kpp_board_index[pc][perspective] +
/// PS_END * ksq, with `ksq` already oriented for the same perspective.
#[inline(always)]
fn feature_index(perspective_black: bool, sq: usize, piece_type: usize, is_friend: bool, ksq_oriented: usize) -> usize {
    orient(perspective_black, sq) + ps_base(piece_type, is_friend) as usize + PS_END as usize * ksq_oriented
}

// ----------------------------------------------------------------------------
// Incremental accumulator: TWO halves (one per perspective), pre-ClippedReLU
// ----------------------------------------------------------------------------
//
// Unlike the previous "flat" network (a single perspective, absolute
// features per color), HalfKP requires an accumulator for the white
// perspective AND one for the black perspective, because each uses a
// different king-bucket (its own king) to index the same features. Every
// time one's own king moves, the ENTIRE king-bucket changes for that
// perspective: all active features must be reindexed, so that half must be
// recomputed from scratch (see `refresh_one_perspective`), while the other
// half (whose king hasn't moved) remains incrementally updatable.
//
// Sum kept as i32 (not i16 with saturating_add as in an early abandoned
// draft): saturation is not invertible, and here it's required that
// add_piece/remove_piece be each other's exact inverse in every case,
// regardless of the magnitude of the loaded weights.
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
    /// weights_[feature_index * L1_SIZE + j]: a row of L1_SIZE weights for
    /// each of the HALFKP_INPUT_DIMENSIONS features. ~21 MB: must live on
    /// the heap (Vec), not on the stack nor as an array embedded in the
    /// binary.
    ft_weight: Vec<i16>,
    l1_bias: [i32; L2_SIZE],
    l1_weight: [i8; L2_SIZE * L1_SIZE * 2],
    l2_bias: [i32; L3_SIZE],
    l2_weight: [i8; L3_SIZE * L2_SIZE],
    out_bias: i32,
    out_weight: [i8; L3_SIZE],
}

/// Little-endian read cursor over an in-memory buffer: avoids pulling in an
/// external dependency (byteorder) just for this one-off parser, run at
/// most once per engine startup.
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
                "⚠️ NNUE: unrecognized file version 0x{:08X} (expected 0x{:08X}, classic HalfKP 256x2-32-32-1 format). File ignored.",
                version, SF_NNUE_VERSION
            );
            return None;
        }
        let top_hash = r.read_u32()?;
        let desc_len = r.read_u32()? as usize;
        r.skip(desc_len)?;

        // --- STRUCTURAL VALIDATION (the real gate) ---
        // Regardless of any hash: for the HalfKP 256x2-32-32-1 architecture
        // the number of bytes remaining in the file is fixed exactly by
        // these dimensions. A file from a different architecture (e.g. the
        // modern HalfKAv2_hm networks, much larger and with compressed
        // weights) can never have exactly this length: it is rejected here,
        // BEFORE allocating the ~21 MB feature transformer weight table.
        let ft_bytes = 4 + L1_SIZE * 2 + HALFKP_INPUT_DIMENSIONS * L1_SIZE * 2;
        let net_bytes = 4
            + (L2_SIZE * 4 + L2_SIZE * (L1_SIZE * 2))
            + (L3_SIZE * 4 + L3_SIZE * L2_SIZE)
            + (4 + L3_SIZE);
        let expected = ft_bytes + net_bytes;
        if r.remaining() != expected {
            println!(
                "⚠️ NNUE: unexpected size ({} bytes remaining, expected {}): not a compatible HalfKP 256x2-32-32-1 network. File ignored.",
                r.remaining(), expected
            );
            return None;
        }

        if top_hash != TOP_HASH {
            println!(
                "info string NNUE: architecture hash 0x{:08X} differs from the computed one 0x{:08X} (continuing anyway, the size check has already validated the layout)",
                top_hash, TOP_HASH
            );
        }

        let ft_hash = r.read_u32()?;
        if ft_hash != FT_HASH {
            println!("info string NNUE: unexpected feature transformer hash (continuing anyway)");
        }

        let mut ft_bias = [0i16; L1_SIZE];
        for b in ft_bias.iter_mut() { *b = r.read_i16()?; }

        let mut ft_weight = vec![0i16; HALFKP_INPUT_DIMENSIONS * L1_SIZE];
        for w in ft_weight.iter_mut() { *w = r.read_i16()?; }

        let net_hash = r.read_u32()?;
        if net_hash != OUTPUT_HASH {
            println!("info string NNUE: unexpected network hash (continuing anyway)");
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
            println!("⚠️ NNUE: {} bytes unconsumed at end of file (unexpected format). File ignored.", r.remaining());
            return None;
        }

        println!("✅ NNUE: HalfKP 256x2-32-32-1 network loaded ({} input features).", HALFKP_INPUT_DIMENSIONS);

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

    /// Recomputes from scratch the accumulator half relative to ONE single
    /// perspective (`perspective_white`), starting from the bias and all
    /// non-King pieces present on the board. Must be used when that
    /// perspective's king has just moved (the entire king-bucket changes,
    /// no active feature remains valid) or to initialize a new position.
    /// `piece_of` must return, for each square occupied by a non-King
    /// piece, the pair (color_white: bool, piece_type 1..=5).
    pub fn refresh_one_perspective(&self, half: &mut [i32; L1_SIZE], board: &Scacchiera, perspective_white: bool) {
        for i in 0..L1_SIZE { half[i] = self.ft_bias[i] as i32; }

        let perspective_black = !perspective_white;
        let own_king_sq = king_square(board, perspective_white);
        let ksq = orient(perspective_black, own_king_sq);

        // Piece = Luna index 0..=4 (Pawn..Queen); the King (index 5) is
        // excluded by the loop's range itself, not by an explicit check: in
        // HalfKP kings are never features, only king-bucket selectors.
        for p_idx in 0..5 {
            let piece_type = p_idx + 1;
            let mut bb_w = board.pezzi[p_idx] & board.colori[0];
            while bb_w != 0 {
                let sq = bb_w.trailing_zeros() as usize;
                let is_friend = perspective_white; // white piece, white perspective => friend
                self.add_row(half, feature_index(perspective_black, sq, piece_type, is_friend, ksq));
                bb_w &= bb_w - 1;
            }
            let mut bb_b = board.pezzi[p_idx] & board.colori[1];
            while bb_b != 0 {
                let sq = bb_b.trailing_zeros() as usize;
                let is_friend = !perspective_white; // black piece, white perspective => enemy
                self.add_row(half, feature_index(perspective_black, sq, piece_type, is_friend, ksq));
                bb_b &= bb_b - 1;
            }
        }
    }

    /// Recomputes both halves from scratch. To be used only once after
    /// setting up a new position (new game, FEN, UCI "position" command);
    /// from then on board.rs maintains the accumulator with
    /// `add_piece`/`remove_piece` and a targeted `refresh_one_perspective`.
    pub fn refresh(&self, board: &Scacchiera) -> Accumulator {
        let mut acc = Accumulator::zero();
        self.refresh_one_perspective(&mut acc.white, board, true);
        self.refresh_one_perspective(&mut acc.black, board, false);
        acc
    }

    /// INCREMENTALLY updates both perspectives for a piece that appears on
    /// square `sq` (color `color_white`, Luna type `piece_type_luna`
    /// 0..=5). If `piece_type_luna == 5` (King) the call is a no-op: the
    /// King is never a tracked feature in HalfKP, its movement must be
    /// handled by board.rs with `refresh_one_perspective` on its own
    /// perspective, not with this function. `white_ksq`/`black_ksq` must be
    /// the squares of the two kings BEFORE this move's mutations (for the
    /// perspective whose king doesn't move on this move, they are also the
    /// correct squares afterwards; for the other one, the passed value is
    /// irrelevant because that half will be recomputed from scratch
    /// anyway).
    #[inline]
    pub fn add_piece(&self, acc: &mut Accumulator, color_white: bool, piece_type_luna: usize, sq: usize, white_ksq: usize, black_ksq: usize) {
        if piece_type_luna >= 5 { return; }
        let piece_type = piece_type_luna + 1;
        self.add_row(&mut acc.white, feature_index(false, sq, piece_type, color_white, orient(false, white_ksq)));
        self.add_row(&mut acc.black, feature_index(true, sq, piece_type, !color_white, orient(true, black_ksq)));
    }

    /// Exact inverse of `add_piece`: to be called when a (non-King) piece
    /// DISAPPEARS from square `sq`.
    #[inline]
    pub fn remove_piece(&self, acc: &mut Accumulator, color_white: bool, piece_type_luna: usize, sq: usize, white_ksq: usize, black_ksq: usize) {
        if piece_type_luna >= 5 { return; }
        let piece_type = piece_type_luna + 1;
        self.sub_row(&mut acc.white, feature_index(false, sq, piece_type, color_white, orient(false, white_ksq)));
        self.sub_row(&mut acc.black, feature_index(true, sq, piece_type, !color_white, orient(true, black_ksq)));
    }

    /// Layers 1+2 (affine, int8, with ClippedReLU and scale shift) and the
    /// output layer (affine, without ClippedReLU) starting from an already
    /// updated accumulator. `side_to_move_white` determines the
    /// concatenation order (perspective of the side to move first, the
    /// opponent's after): this is exactly how the network was trained, and
    /// it's also what makes the result already correctly "signed" for
    /// search.rs's negamax convention, without needing a final sign flip
    /// like in the old single-perspective network.
    pub fn evaluate_from_accumulator(&self, acc: &Accumulator, side_to_move_white: bool) -> i32 {
        let (us, them) = if side_to_move_white { (&acc.white, &acc.black) } else { (&acc.black, &acc.white) };

        // --- Feature transform: direct clamp to [0,127], NO shift ---
        // (the feature transformer's ClippedReLU operates on the raw int16
        // sum, unlike the hidden layers' below).
        let mut l1_in = [0u8; L1_SIZE * 2];
        for i in 0..L1_SIZE { l1_in[i] = us[i].clamp(0, 127) as u8; }
        for i in 0..L1_SIZE { l1_in[L1_SIZE + i] = them[i].clamp(0, 127) as u8; }

        // --- Hidden layer 1: 512 -> 32, then ClippedReLU with scale shift ---
        // Vectorized dot product (AVX2/SSE2, see `dot_i8u8` below): this is
        // where about 94% of the forward pass cost is concentrated
        // (32 neurons * 512 weights each).
        let mut h1 = [0u8; L2_SIZE];
        for o in 0..L2_SIZE {
            let row = &self.l1_weight[o * L1_SIZE * 2..(o + 1) * L1_SIZE * 2];
            let sum = self.l1_bias[o] + dot_i8u8(&l1_in, row);
            h1[o] = (sum >> WEIGHT_SCALE_BITS).clamp(0, 127) as u8;
        }

        // --- Hidden layer 2: 32 -> 32, then ClippedReLU with scale shift ---
        let mut h2 = [0u8; L3_SIZE];
        for o in 0..L3_SIZE {
            let row = &self.l2_weight[o * L2_SIZE..(o + 1) * L2_SIZE];
            let sum = self.l2_bias[o] + dot_i8u8(&h1, row);
            h2[o] = (sum >> WEIGHT_SCALE_BITS).clamp(0, 127) as u8;
        }

        // --- Output layer: 32 -> 1, NO ClippedReLU after ---
        let out = self.out_bias + dot_i8u8(&h2, &self.out_weight);

        // --- Final conversion to centipawns (single point, FV_SCALE=16) ---
        (out / FV_SCALE).clamp(-NNUE_EVAL_CLAMP, NNUE_EVAL_CLAMP)
    }
}

/// Square of the white king (`perspective_white=true`) or black.
#[inline(always)]
fn king_square(board: &Scacchiera, perspective_white: bool) -> usize {
    let color_idx = if perspective_white { 0 } else { 1 };
    (board.pezzi[5] & board.colori[color_idx]).trailing_zeros() as usize
}

// ============================================================================
// VECTORIZED FORWARD PASS (AVX2 / SSE2, with scalar fallback)
// ============================================================================
//
// The three affine transforms of the forward pass (512->32, 32->32, 32->1)
// are all the SAME operation, repeated once per output neuron: a dot
// product between a vector of UNSIGNED activations (u8, output of a
// ClippedReLU, range [0,127]) and a row of SIGNED weights (i8), accumulated
// in i32. The 512->32 layer dominates the cost (32*512 = 16384
// multiply-adds versus the second layer's 32*32=1024 and the output's 32:
// about 94% of the total work), and is therefore the part where
// vectorization really pays off, but for code uniformity and simplicity all
// three transforms go through the same `dot_i8u8` kernel.
//
// For the same reason, the most-called function is the one with the best
// payoff: we do NOT touch, on the other hand, the clamp step that builds
// `l1_in` (512 elements, simple clamp+cast) nor the ClippedReLUs between
// layers, because they are O(tens-hundreds) of trivial scalar operations,
// not the measured bottleneck.
//
// Runtime (not compile-time) implementation selection: the binary must run
// correctly even on a CPU without AVX2, so the choice among the three paths
// happens on every call via `is_x86_feature_detected!` (which internally
// caches the CPUID result, negligible cost) rather than via a compile-time
// `cfg`: a binary compiled on a machine with AVX2 but run on one without it
// would otherwise crash (illegal instruction) instead of simply being
// slower.

// Used as a fallback on non-x86_64 targets and as the reference in the
// correctness tests below: on x86_64 in a normal (non-test) build the
// dispatcher never calls it, hence the `allow(dead_code)`.
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

/// Dot product `input`(u8)·`weight`(i8) -> i32, with runtime dispatch to
/// AVX2, otherwise SSE2 (always present on x86_64); on aarch64 (Android) to
/// NEON (always present in the AArch64 baseline, unlike AVX2); on any other
/// target the pure scalar version.
#[inline]
fn dot_i8u8(input: &[u8], weight: &[i8]) -> i32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: the AVX2 feature was just verified as available on
            // the current CPU via `is_x86_feature_detected!`.
            return unsafe { simd::dot_i8u8_avx2(input, weight) };
        }
        // SSE2 is part of the x86_64 architecture baseline (guaranteed by
        // the specification itself, unlike AVX2 which is optional): no
        // runtime check is needed to use it safely.
        // SAFETY: SSE2 is always available on x86_64.
        return unsafe { simd::dot_i8u8_sse2(input, weight) };
    }
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is part of the mandatory AArch64 baseline (unlike
        // AVX2 on x86_64): no runtime check is needed, exactly as for SSE2
        // above.
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

    /// Dot product u8·i8 -> i32, 32 bytes per iteration (one YMM register).
    ///
    /// `_mm256_maddubs_epi16` (VPMADDUBSW) multiplies byte by byte
    /// unsigned*signed and already sums adjacent pairs, producing 16 lanes
    /// of 16 bits; `_mm256_madd_epi16` against a register of all 1s in turn
    /// sums adjacent pairs of i16 lanes into i32, completing the reduction:
    /// each final i32 lane is the sum of 4 consecutive byte products. This
    /// is the same scheme (non-VNNI path) used by Stockfish in
    /// nnue/layers/affine_transform.h for this identical type of layer.
    ///
    /// The tail (residual elements beyond the last block of 32) is handled
    /// scalarly: for this network (dimensions 512/32/32, all multiples of
    /// 32) it never triggers, but the function remains correct for any
    /// length, including ones not a multiple of 32.
    ///
    /// # Safety
    /// The caller must have verified `is_x86_feature_detected!("avx2")`: on
    /// a CPU without AVX2 these instructions would cause an "illegal
    /// instruction" exception. `input.len() == weight.len()` must hold
    /// (not checked in release via an `assert` so as not to pay its cost on
    /// every call on the hot path, but checked via `debug_assert_eq!`).
    #[target_feature(enable = "avx2")]
    pub unsafe fn dot_i8u8_avx2(input: &[u8], weight: &[i8]) -> i32 {
        debug_assert_eq!(input.len(), weight.len());
        let len = input.len();
        let chunks = len / 32;

        let ones = _mm256_set1_epi16(1);
        let mut acc = _mm256_setzero_si256();

        for c in 0..chunks {
            let off = c * 32;
            // Unaligned load: the slices come from stack arrays sized for
            // the layer (e.g. [u8; 512]), with no guarantee of 32-byte
            // alignment, hence `loadu`.
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

    /// Horizontal reduction of the 8 i32 lanes of a `__m256i` into a single
    /// scalar: repeatedly halves the width (256->128->64->32 bits).
    #[target_feature(enable = "avx2")]
    unsafe fn hsum_epi32_avx2(v: __m256i) -> i32 {
        let lo = _mm256_castsi256_si128(v);
        let hi = _mm256_extracti128_si256(v, 1);
        let sum128 = _mm_add_epi32(lo, hi);
        let shuf1 = _mm_shuffle_epi32(sum128, 0b01_00_11_10); // high/low 64-bit halves swapped
        let sum64 = _mm_add_epi32(sum128, shuf1);
        let shuf2 = _mm_shuffle_epi32(sum64, 0b00_00_00_01); // high/low 32-bit halves swapped
        let sum32 = _mm_add_epi32(sum64, shuf2);
        _mm_cvtsi128_si32(sum32)
    }

    /// Dot product u8·i8 -> i32, 16 bytes per iteration (one XMM register),
    /// for CPUs without AVX2 (SSE2 is guaranteed on any x86_64).
    ///
    /// SSE2 has no equivalent of `maddubs` (that arrived only with SSSE3):
    /// the extension to 16 bits must be done by hand, separately per
    /// operand. `input` is unsigned: zero-extend (`unpack` with a register
    /// of zeros as the high byte of each lane). `weight` is signed:
    /// instead a SIGN extension is needed, obtained by duplicating the sign
    /// itself as the high byte (`_mm_cmpgt_epi8(zero, b)` produces 0xFF
    /// where `b<0`, 0x00 otherwise). Once both are brought to 16 bits with
    /// the correct sign, `_mm_madd_epi16` multiplies and sums in pairs in a
    /// single step, exactly as Stockfish does in its own SSE2 non-SSSE3
    /// fallback.
    ///
    /// # Safety
    /// SSE2 is always available on x86_64 by platform specification: no
    /// feature check is required from the caller. It remains `unsafe` only
    /// because it invokes direct SIMD intrinsics.
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

    /// Dot product u8·i8 -> i32, 16 bytes per iteration (one 128-bit NEON
    /// register), for the Android/ARM64 target of the MCEC tournament.
    ///
    /// Same scheme as the SSE2 fallback (no direct equivalent of
    /// `maddubs`/VNNI available without the optional `dotprod`/`i8mm`
    /// extensions, which not all phones guarantee): both operands are
    /// extended to 16 bits before multiplying. `input` is unsigned in
    /// [0,127] (output of a ClippedReLU): extending to 16 bits with
    /// zero-fill (`vmovl_u8`) can never set the sign bit, so reinterpreting
    /// the result as `int16x8_t` is always correct. `weight` is signed:
    /// `vmovl_s8` already extends with the correct sign. The widening
    /// multiply-accumulate (`vmlal_s16`, 4 lanes per call) does the rest in
    /// a single step, like `_mm_madd_epi16`.
    ///
    /// The final horizontal reduction uses `vaddvq_s32`, a native AArch64
    /// instruction (not available on ARMv7) that sums the 4 lanes in one
    /// go, simpler than the shuffle sequence required on x86.
    ///
    /// # Safety
    /// NEON is a mandatory part of the AArch64 baseline (ARM
    /// specification), so no feature check is required from the caller —
    /// unlike AVX2 on x86_64. It remains `unsafe` only because it invokes
    /// direct SIMD intrinsics. `input.len() == weight.len()` must hold
    /// (checked via `debug_assert_eq!`, not in release so as not to pay its
    /// cost on the hot path).
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

    /// Minimal xorshift64 to generate deterministic pseudo-random bytes
    /// without depending on the `rand` crate for this test.
    fn xorshift_bytes_u8(seed: u64, n: usize) -> Vec<u8> {
        let mut s = seed ^ 0x9E3779B97F4A7C15;
        (0..n).map(|_| {
            s ^= s << 13; s ^= s >> 7; s ^= s << 17;
            (s % 128) as u8 // range [0,127], like real post-ClippedReLU activations
        }).collect()
    }
    fn xorshift_bytes_i8(seed: u64, n: usize) -> Vec<i8> {
        let mut s = seed ^ 0xBF58476D1CE4E5B9;
        (0..n).map(|_| {
            s ^= s << 13; s ^= s >> 7; s ^= s << 17;
            (s % 256) as u8 as i8 // full i8 range, like real quantized weights
        }).collect()
    }

    /// Compares scalar vs SIMD across various lengths, including ones NOT a
    /// multiple of the register width (32 for AVX2, 16 for SSE2): for this
    /// network (512/32/32) the tail never triggers, but the kernel must
    /// remain correct in general.
    #[test]
    fn dot_product_simd_matches_scalar() {
        for &len in &[0usize, 1, 3, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129, 512] {
            for seed in 0u64..5 {
                let input = xorshift_bytes_u8(seed * 1000 + len as u64, len);
                let weight = xorshift_bytes_i8(seed * 2000 + len as u64, len);

                let scalar = dot_i8u8_scalar(&input, &weight);

                if is_x86_feature_detected!("avx2") {
                    let avx2 = unsafe { simd::dot_i8u8_avx2(&input, &weight) };
                    assert_eq!(scalar, avx2, "AVX2 mismatched from scalar for len={len}, seed={seed}");
                }

                let sse2 = unsafe { simd::dot_i8u8_sse2(&input, &weight) };
                assert_eq!(scalar, sse2, "SSE2 mismatched from scalar for len={len}, seed={seed}");

                #[cfg(target_arch = "aarch64")]
                {
                    let neon = unsafe { simd_aarch64::dot_i8u8_neon(&input, &weight) };
                    assert_eq!(scalar, neon, "NEON mismatched from scalar for len={len}, seed={seed}");
                }

                let dispatched = dot_i8u8(&input, &weight);
                assert_eq!(scalar, dispatched, "dispatcher mismatched from scalar for len={len}, seed={seed}");
            }
        }
    }
}

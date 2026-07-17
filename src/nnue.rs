// Nnue.rs

use std::fs::File;
use std::io::Read;
use crate::board::{Scacchiera, Colore};

// --- NNUE ARCHITECTURE CONFIGURATION ---
// 768 input features (12 piece types * 64 squares).
const INPUT_SIZE: usize = 768;
// Size of the first hidden layer (Feature Transformer).
const L1_SIZE: usize = 256;
// Size of the second hidden layer.
const L2_SIZE: usize = 32;

// --- QUANTIZATION SCALES ---
// These MUST match `esporta_muscolosa.py`:
//   Layer 1 weights/bias are stored as round(real * QA).
//   Layer 2 and Layer 3 weights/bias are stored as round(real * QB).
const QA: i32 = 255;
const QB: i32 = 64;
// Product scale of the network's raw output (real_output = out_i32 / QAB).
const QAB: i32 = QA * QB;

// The network is trained on game results (WDL), so its raw output is roughly a
// white-win probability centered on 0.5. This linear factor converts that
// probability into centipawns (0.5 -> 0 cp). Tune it to taste / to match the
// `scaling` convention used in `simple_model.py`.
const EVAL_SCALE: i32 = 2400;

// ============================================================================
// SIMD BACKEND SELECTION
// ============================================================================
// The hot path of NNUE inference is L1 accumulation: for every occupied square
// we add a 256-wide i16 row from the weight matrix into the accumulator
// (saturating), then apply a CReLU clamp to the 256-wide accumulator. Both
// steps are data-parallel over contiguous i16 arrays, the textbook SIMD case.
// We provide:
//   - an AVX2 implementation (x86_64, runtime-detected via CPUID, cached),
//   - a NEON implementation (aarch64, always available on that target),
//   - a portable scalar fallback used anywhere else, or if AVX2 is missing.
//
// Every SIMD routine has a scalar twin computing the identical arithmetic, so
// switching backends never changes the evaluation output.
//
// L2 (256x32) and L3 (32) are tiny (8192 + 32 multiply-adds) and run as plain
// scalar code below; they are not a meaningful fraction of the runtime.
// ============================================================================

#[cfg(target_arch = "x86_64")]
fn has_avx2() -> bool {
    use std::sync::OnceLock;
    static AVX2: OnceLock<bool> = OnceLock::new();
    *AVX2.get_or_init(|| is_x86_feature_detected!("avx2"))
}

/// Main structure for the quantized LunaNNUE neural network.
/// Stores weights and biases as 16-bit integers (`i16`) to optimize performance on CPUs without an FPU.
pub struct LunaNNUE {
    // Feature transformer weights, transposed at load time into an
    // input-major layout: `l1_weights[feature * L1_SIZE + neuron]`, so the
    // L1_SIZE weights of a single feature are contiguous (SIMD-friendly).
    l1_weights: Vec<i16>,
    l1_bias: Vec<i16>,
    // Hidden layer weights, transposed at load time into an input-major
    // layout: `l2_weights[input * L2_SIZE + output]`.
    l2_weights: Vec<i16>,
    l2_bias: Vec<i16>,
    l3_weights: Vec<i16>,
    l3_bias: Vec<i16>,
}

impl LunaNNUE {
    /// Loads the network's binary weights from an external file in Little-Endian format.
    /// Returns `None` in case of I/O error or corrupted file.
    ///
    /// `esporta_muscolosa.py` writes each PyTorch `nn.Linear.weight` verbatim,
    /// i.e. in `[out_features, in_features]` (output-major) order. The inference
    /// hot path wants the opposite (input-major) layout, so L1 and L2 are
    /// transposed here, once, at load time.
    pub fn load(path: &str) -> Option<Self> {
        let mut file = File::open(path).ok()?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).ok()?;

        let mut offset = 0;
        // Helper closure to read paired bytes sequentially and convert them into i16.
        let mut read_i16 = |count: usize| -> Vec<i16> {
            let mut v = Vec::with_capacity(count);
            for _ in 0..count {
                if offset + 2 <= buffer.len() {
                    v.push(i16::from_le_bytes([buffer[offset], buffer[offset + 1]]));
                    offset += 2;
                }
            }
            v
        };

        // Read in file order (output-major for the weight matrices).
        let l1_w_raw = read_i16(L1_SIZE * INPUT_SIZE); // [out=256][in=768]
        let l1_bias = read_i16(L1_SIZE);
        let l2_w_raw = read_i16(L2_SIZE * L1_SIZE); // [out=32][in=256]
        let l2_bias = read_i16(L2_SIZE);
        let l3_weights = read_i16(L2_SIZE); // [out=1][in=32]
        let l3_bias = read_i16(1);

        if l1_w_raw.len() != L1_SIZE * INPUT_SIZE
            || l1_bias.len() != L1_SIZE
            || l2_w_raw.len() != L2_SIZE * L1_SIZE
            || l2_bias.len() != L2_SIZE
            || l3_weights.len() != L2_SIZE
            || l3_bias.len() != 1
        {
            println!("⚠️ WARNING: The NNUE file is truncated or the wrong size!");
            return None;
        }

        // Transpose L1: [out=256][in=768] -> [in=768][out=256].
        let mut l1_weights = vec![0i16; INPUT_SIZE * L1_SIZE];
        for n in 0..L1_SIZE {
            for f in 0..INPUT_SIZE {
                l1_weights[f * L1_SIZE + n] = l1_w_raw[n * INPUT_SIZE + f];
            }
        }

        // Transpose L2: [out=32][in=256] -> [in=256][out=32].
        let mut l2_weights = vec![0i16; L1_SIZE * L2_SIZE];
        for o in 0..L2_SIZE {
            for i in 0..L1_SIZE {
                l2_weights[i * L2_SIZE + o] = l2_w_raw[o * L1_SIZE + i];
            }
        }

        // Initial diagnostic check on the binary file status.
        if l1_weights.iter().take(100).all(|&x| x == 0) {
            println!("⚠️ WARNING: The NNUE file seems empty!");
        } else {
            #[cfg(target_arch = "x86_64")]
            let backend = if has_avx2() { "AVX2" } else { "scalar" };
            #[cfg(target_arch = "aarch64")]
            let backend = "NEON";
            #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
            let backend = "scalar";
            println!("✅ NNUE: Weights loaded (Safe mode, {} backend).", backend);
        }

        Some(LunaNNUE {
            l1_weights,
            l1_bias,
            l2_weights,
            l2_bias,
            l3_weights,
            l3_bias,
        })
    }

    /// Performs the feedforward (inference) step of the network for the current position.
    /// Extracts active features via bitboards and propagates values through quantized layers.
    ///
    /// Returns a centipawn score from the perspective of the side to move.
    pub fn evaluate(&self, s: &Scacchiera) -> i32 {
        // ==========================================
        // --- LAYER 1: FEATURE TRANSFORMER (scale QA)
        // ==========================================
        let mut l1_acc = [0i16; L1_SIZE];
        l1_acc.copy_from_slice(&self.l1_bias);

        // Iterate over the 6 piece types.
        for p_idx in 0..6 {
            // White (color index 0): feature = (p_idx * 64 + sq).
            let mut bb_w = s.pezzi[p_idx] & s.colori[0];
            while bb_w != 0 {
                let sq = bb_w.trailing_zeros() as usize;
                let offset = (p_idx * 64 + sq) * L1_SIZE;
                simd::accumulate(&mut l1_acc, &self.l1_weights[offset..offset + L1_SIZE]);
                bb_w &= bb_w - 1;
            }

            // Black (color index 1): feature = ((p_idx + 6) * 64 + sq).
            let mut bb_b = s.pezzi[p_idx] & s.colori[1];
            while bb_b != 0 {
                let sq = bb_b.trailing_zeros() as usize;
                let offset = ((p_idx + 6) * 64 + sq) * L1_SIZE;
                simd::accumulate(&mut l1_acc, &self.l1_weights[offset..offset + L1_SIZE]);
                bb_b &= bb_b - 1;
            }
        }

        // Clipped ReLU: real activations live in [0, 1], represented as [0, QA].
        let mut l1_out = [0i16; L1_SIZE];
        simd::crelu(&l1_acc, &mut l1_out, QA as i16);

        // ==========================================
        // --- LAYER 2: HIDDEN LAYER (scale QB)
        // ==========================================
        // acc (bias*QA + sum) is at scale QA*QB; dividing by QB brings it back
        // to scale QA. The model uses a plain `nn.ReLU()` here (no upper bound),
        // so we clamp only at 0 and keep the value in i32.
        let mut l2_out = [0i32; L2_SIZE];
        for o in 0..L2_SIZE {
            let mut sum = 0i32;
            for j in 0..L1_SIZE {
                let lj = l1_out[j] as i32;
                if lj != 0 {
                    sum += lj * self.l2_weights[j * L2_SIZE + o] as i32;
                }
            }
            let v = (self.l2_bias[o] as i32 * QA + sum) / QB;
            l2_out[o] = v.max(0);
        }

        // ==========================================
        // --- LAYER 3: LINEAR OUTPUT
        // ==========================================
        let mut sum = 0i32;
        for i in 0..L2_SIZE {
            sum += l2_out[i] * self.l3_weights[i] as i32;
        }
        // out_full is at scale QA*QB; real_output = out_full / QAB is ~a
        // white-win probability. Center on 0.5 and scale into centipawns.
        let out_full = self.l3_bias[0] as i32 * QA + sum;
        let score = (out_full - QAB / 2) * EVAL_SCALE / QAB;

        // Synchronize output with the Negamax search perspective.
        if s.turno == Colore::Bianco { score } else { -score }
    }
}

// ============================================================================
// SIMD implementations for the L1 hot path (accumulate + CReLU)
// ============================================================================
mod simd {
    use super::L1_SIZE;

    /// acc[i] = saturating_add(acc[i], weights[i]) for i in 0..L1_SIZE
    #[inline(always)]
    pub fn accumulate(acc: &mut [i16; L1_SIZE], weights: &[i16]) {
        debug_assert_eq!(weights.len(), L1_SIZE);

        #[cfg(target_arch = "x86_64")]
        {
            if super::has_avx2() {
                unsafe { x86::accumulate_avx2(acc, weights) };
                return;
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            unsafe { arm::accumulate_neon(acc, weights) };
            return;
        }
        #[allow(unreachable_code)]
        accumulate_scalar(acc, weights);
    }

    #[inline(always)]
    #[allow(dead_code)]
    fn accumulate_scalar(acc: &mut [i16; L1_SIZE], weights: &[i16]) {
        for i in 0..L1_SIZE {
            acc[i] = acc[i].saturating_add(weights[i]);
        }
    }

    /// out[i] = clamp(acc[i], 0, ceiling)
    #[inline(always)]
    pub fn crelu(acc: &[i16; L1_SIZE], out: &mut [i16; L1_SIZE], ceiling: i16) {
        #[cfg(target_arch = "x86_64")]
        {
            if super::has_avx2() {
                unsafe { x86::crelu_avx2(acc, out, ceiling) };
                return;
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            unsafe { arm::crelu_neon(acc, out, ceiling) };
            return;
        }
        #[allow(unreachable_code)]
        crelu_scalar(acc, out, ceiling);
    }

    #[inline(always)]
    #[allow(dead_code)]
    fn crelu_scalar(acc: &[i16; L1_SIZE], out: &mut [i16; L1_SIZE], ceiling: i16) {
        for i in 0..L1_SIZE {
            out[i] = acc[i].clamp(0, ceiling);
        }
    }

    // ------------------------------------------------------------------
    // x86_64 / AVX2 backend
    // ------------------------------------------------------------------
    #[cfg(target_arch = "x86_64")]
    mod x86 {
        use super::L1_SIZE;
        use std::arch::x86_64::*;

        #[target_feature(enable = "avx2")]
        pub unsafe fn accumulate_avx2(acc: &mut [i16; L1_SIZE], weights: &[i16]) {
            // 16 x i16 lanes per __m256i -> L1_SIZE / 16 iterations.
            let mut i = 0;
            while i < L1_SIZE {
                let a = _mm256_loadu_si256(acc.as_ptr().add(i) as *const __m256i);
                let w = _mm256_loadu_si256(weights.as_ptr().add(i) as *const __m256i);
                let sum = _mm256_adds_epi16(a, w); // saturating add, matches i16::saturating_add
                _mm256_storeu_si256(acc.as_mut_ptr().add(i) as *mut __m256i, sum);
                i += 16;
            }
        }

        #[target_feature(enable = "avx2")]
        pub unsafe fn crelu_avx2(acc: &[i16; L1_SIZE], out: &mut [i16; L1_SIZE], ceiling: i16) {
            let zero = _mm256_setzero_si256();
            let cap = _mm256_set1_epi16(ceiling);
            let mut i = 0;
            while i < L1_SIZE {
                let a = _mm256_loadu_si256(acc.as_ptr().add(i) as *const __m256i);
                let clamped = _mm256_min_epi16(_mm256_max_epi16(a, zero), cap);
                _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, clamped);
                i += 16;
            }
        }
    }

    // ------------------------------------------------------------------
    // aarch64 / NEON backend
    // ------------------------------------------------------------------
    #[cfg(target_arch = "aarch64")]
    mod arm {
        use super::L1_SIZE;
        use std::arch::aarch64::*;

        #[target_feature(enable = "neon")]
        pub unsafe fn accumulate_neon(acc: &mut [i16; L1_SIZE], weights: &[i16]) {
            // 8 x i16 lanes per 128-bit vector -> L1_SIZE / 8 iterations.
            let mut i = 0;
            while i < L1_SIZE {
                let a = vld1q_s16(acc.as_ptr().add(i));
                let w = vld1q_s16(weights.as_ptr().add(i));
                let sum = vqaddq_s16(a, w); // saturating add
                vst1q_s16(acc.as_mut_ptr().add(i), sum);
                i += 8;
            }
        }

        #[target_feature(enable = "neon")]
        pub unsafe fn crelu_neon(acc: &[i16; L1_SIZE], out: &mut [i16; L1_SIZE], ceiling: i16) {
            let zero = vdupq_n_s16(0);
            let cap = vdupq_n_s16(ceiling);
            let mut i = 0;
            while i < L1_SIZE {
                let a = vld1q_s16(acc.as_ptr().add(i));
                let clamped = vminq_s16(vmaxq_s16(a, zero), cap);
                vst1q_s16(out.as_mut_ptr().add(i), clamped);
                i += 8;
            }
        }
    }
}

// ============================================================================
// Tests: every SIMD backend must match the scalar reference exactly.
// ============================================================================
#[cfg(test)]
mod tests {
    use super::simd::*;
    use super::L1_SIZE;

    fn rand_i16_vec(len: usize, seed: &mut u64) -> Vec<i16> {
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            v.push(((*seed >> 48) as i16) % 2000);
        }
        v
    }

    #[test]
    fn accumulate_matches_scalar() {
        let mut seed = 12345u64;
        let base = rand_i16_vec(L1_SIZE, &mut seed);
        let weights = rand_i16_vec(L1_SIZE, &mut seed);

        let mut acc_simd = [0i16; L1_SIZE];
        acc_simd.copy_from_slice(&base);
        accumulate(&mut acc_simd, &weights);

        let mut acc_scalar = [0i16; L1_SIZE];
        acc_scalar.copy_from_slice(&base);
        for i in 0..L1_SIZE {
            acc_scalar[i] = acc_scalar[i].saturating_add(weights[i]);
        }

        assert_eq!(acc_simd, acc_scalar);
    }

    #[test]
    fn crelu_matches_scalar() {
        let mut seed = 999u64;
        let acc: Vec<i16> = rand_i16_vec(L1_SIZE, &mut seed)
            .iter()
            .map(|&x| x.wrapping_sub(1000))
            .collect();
        let mut acc_arr = [0i16; L1_SIZE];
        acc_arr.copy_from_slice(&acc);

        let mut out_simd = [0i16; L1_SIZE];
        crelu(&acc_arr, &mut out_simd, 255);

        let mut out_scalar = [0i16; L1_SIZE];
        for i in 0..L1_SIZE {
            out_scalar[i] = acc_arr[i].clamp(0, 255);
        }

        assert_eq!(out_simd, out_scalar);
    }

    #[test]
    fn stress_full_i16_range_many_trials() {
        // Real quantized weights can span the full i16 range and be negative;
        // hammer the SIMD primitives with 500 trials to catch rare lane bugs.
        let mut seed = 0xC0FFEEu64;
        let mut next = |lo: i32, hi: i32| -> i16 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r = (seed >> 33) as i64;
            (lo as i64 + (r % (hi - lo + 1) as i64)) as i16
        };

        for _ in 0..500 {
            // accumulate
            let base: Vec<i16> = (0..L1_SIZE).map(|_| next(-32000, 32000)).collect();
            let weights: Vec<i16> = (0..L1_SIZE).map(|_| next(i16::MIN as i32, i16::MAX as i32)).collect();
            let mut a1 = [0i16; L1_SIZE];
            a1.copy_from_slice(&base);
            accumulate(&mut a1, &weights);
            let mut a2 = [0i16; L1_SIZE];
            a2.copy_from_slice(&base);
            for i in 0..L1_SIZE { a2[i] = a2[i].saturating_add(weights[i]); }
            assert_eq!(a1, a2, "accumulate mismatch");

            // crelu
            let mut c1 = [0i16; L1_SIZE];
            crelu(&a1, &mut c1, 255);
            let mut c2 = [0i16; L1_SIZE];
            for i in 0..L1_SIZE { c2[i] = a1[i].clamp(0, 255); }
            assert_eq!(c1, c2, "crelu mismatch");
        }
    }
}

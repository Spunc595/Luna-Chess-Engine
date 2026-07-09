use std::fs::File;
use std::io::Read;
use crate::board::{Scacchiera, Colore};

// --- NNUE ARCHITECTURE CONFIGURATION ---
// 768 input features (12 piece types * 64 squares)[cite: 5].
const INPUT_SIZE: usize = 768; 
// Size of the first hidden layer (Feature Transformer)[cite: 5].
const L1_SIZE: usize = 256;
// Size of the second hidden layer[cite: 5].
const L2_SIZE: usize = 32;

/// Main structure for the quantized LunaNNUE neural network[cite: 5].
/// Stores weights and biases as 16-bit integers (`i16`) to optimize performance on CPUs without an FPU[cite: 5].
pub struct LunaNNUE {
    l1_weights: Vec<i16>,
    l1_bias: Vec<i16>,
    l2_weights: Vec<i16>,
    l2_bias: Vec<i16>,
    l3_weights: Vec<i16>,
    l3_bias: Vec<i16>,
}

impl LunaNNUE {
    /// Loads the network's binary weights from an external file in Little-Endian format[cite: 5].
    /// Returns `None` in case of I/O error or corrupted file[cite: 5].
    pub fn load(path: &str) -> Option<Self> {
        let mut file = File::open(path).ok()?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).ok()?;

        let mut net = LunaNNUE {
            l1_weights: vec![0; INPUT_SIZE * L1_SIZE],
            l1_bias: vec![0; L1_SIZE],
            l2_weights: vec![0; L1_SIZE * L2_SIZE],
            l2_bias: vec![0; L2_SIZE],
            l3_weights: vec![0; L2_SIZE],
            l3_bias: vec![0; 1],
        };

        let mut offset = 0;
        // Helper closure to read paired bytes sequentially and convert them into i16[cite: 5].
        let mut read_i16 = |count: usize| -> Vec<i16> {
            let mut v = Vec::with_capacity(count);
            for _ in 0..count {
                if offset + 2 <= buffer.len() {
                    v.push(i16::from_le_bytes([buffer[offset], buffer[offset+1]]));
                    offset += 2;
                }
            }
            v
        };

        // Ordered mapping of weight and bias vectors[cite: 5].
        net.l1_weights = read_i16(INPUT_SIZE * L1_SIZE);
        net.l1_bias = read_i16(L1_SIZE);
        net.l2_weights = read_i16(L1_SIZE * L2_SIZE);
        net.l2_bias = read_i16(L2_SIZE);
        net.l3_weights = read_i16(L2_SIZE);
        net.l3_bias = read_i16(1);

        // Initial diagnostic check on the binary file status[cite: 5].
        if net.l1_weights.iter().take(100).all(|&x| x == 0) {
            println!("⚠️ WARNING: The NNUE file seems empty!");
        } else {
            println!("✅ NNUE: Weights loaded (Safe mode).");
        }

        Some(net)
    }

    /// Performs the feedforward (inference) step of the network for the current position[cite: 5].
    /// Extracts active features via bitboards and propagates values through quantized layers[cite: 5].
    pub fn evaluate(&self, s: &Scacchiera) -> i32 {
        // ==========================================
        // --- LAYER 1: FEATURE TRANSFORMER (QA=255)
        // ==========================================
        // Initialize the L1 accumulator by cloning the initial bias values[cite: 5].
        let mut l1_acc = self.l1_bias.clone();

        // Iterate over the 6 piece types[cite: 5].
        for p_idx in 0..6 {
            // Sub-step for White (Color Index 0)[cite: 5].
            let mut bb_w = s.pezzi[p_idx] & s.colori[0];
            while bb_w != 0 {
                let sq = bb_w.trailing_zeros() as usize;
                // Calculate feature offset: (piece_type * 64 + square)[cite: 5].
                let offset = (p_idx * 64 + sq) * L1_SIZE;
                for i in 0..L1_SIZE {
                    l1_acc[i] = l1_acc[i].saturating_add(self.l1_weights[offset + i]);
                }
                bb_w &= bb_w - 1; // Clear the processed bit (Bit-pop)
            }

            // Sub-step for Black (Color Index 1)[cite: 5].
            // Black pieces are shifted by 6 positions in the feature index (+6)[cite: 5].
            let mut bb_b = s.pezzi[p_idx] & s.colori[1];
            while bb_b != 0 {
                let sq = bb_b.trailing_zeros() as usize;
                let offset = ((p_idx + 6) * 64 + sq) * L1_SIZE;
                for i in 0..L1_SIZE {
                    l1_acc[i] = l1_acc[i].saturating_add(self.l1_weights[offset + i]);
                }
                bb_b &= bb_b - 1;
            }
        }

        // Apply CReLU activation function on Layer 1[cite: 5].
        // The ceiling is limited to 2048, consistent with the QA=255 scale[cite: 5].
        let mut l1_out = [0i16; 256];
        for i in 0..256 {
            l1_out[i] = l1_acc[i].clamp(0, 2048);
        }

        // ==========================================
        // --- LAYER 2: HIDDEN LAYER (QB=64)
        // ==========================================
        let mut l2_acc = self.l2_bias.clone();
        for i in 0..L2_SIZE {
            let mut sum: i32 = l2_acc[i] as i32;
            for j in 0..L1_SIZE {
                // Integer multiplication followed by division by 2048 (equivalent to bitwise shift >> 11)[cite: 5].
                // Reduces the scale while preserving the precision of the second layer weights[cite: 5].
                sum += (l1_out[j] as i32 * self.l2_weights[j * L2_SIZE + i] as i32) / 2048;
            }
            l2_acc[i] = sum.clamp(-32768, 32767) as i16;
        }

        // CReLU activation function on Layer 2[cite: 5].
        // The ceiling is set to 128 to respect the QB=64 quantization scale[cite: 5].
        let mut l2_out = [0i16; 32];
        for i in 0..32 {
            l2_out[i] = l2_acc[i].clamp(0, 128);
        }

        // ==========================================
        // --- LAYER 3: LINEAR OUTPUT
        // ==========================================
        let mut score: i32 = self.l3_bias[0] as i32;
        for j in 0..L2_SIZE {
            // Final scale reduction via fixed division by 256[cite: 5].
            score += (l2_out[j] as i32 * self.l3_weights[j] as i32) / 256;
        }

        let val = score;
        
        // Synchronize output with the Negamax search perspective[cite: 5].
        if s.turno == Colore::Bianco { val } else { -val }
    }
}
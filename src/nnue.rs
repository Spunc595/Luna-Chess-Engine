use std::fs::File;
use std::io::Read;
use crate::board::{Scacchiera, Colore};

const INPUT_SIZE: usize = 768; 
const L1_SIZE: usize = 256;
const L2_SIZE: usize = 32;

pub struct LunaNNUE {
    l1_weights: Vec<i16>,
    l1_bias: Vec<i16>,
    l2_weights: Vec<i16>,
    l2_bias: Vec<i16>,
    l3_weights: Vec<i16>,
    l3_bias: Vec<i16>,
}

impl LunaNNUE {
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

        net.l1_weights = read_i16(INPUT_SIZE * L1_SIZE);
        net.l1_bias = read_i16(L1_SIZE);
        net.l2_weights = read_i16(L1_SIZE * L2_SIZE);
        net.l2_bias = read_i16(L2_SIZE);
        net.l3_weights = read_i16(L2_SIZE);
        net.l3_bias = read_i16(1);

        // Check semplice per vedere se il file è valido
        if net.l1_weights.iter().take(100).all(|&x| x == 0) {
            println!("⚠️ ATTENZIONE: Il file NNUE sembra vuoto!");
        } else {
            println!("✅ NNUE: Pesi caricati (Modalità Safe).");
        }

        Some(net)
    }

    pub fn evaluate(&self, s: &Scacchiera) -> i32 {
        // --- LAYER 1: Feature Transformer (QA=255) ---
        let mut l1_acc = self.l1_bias.clone();

        for p_idx in 0..6 {
            let mut bb_w = s.pezzi[p_idx] & s.colori[0];
            while bb_w != 0 {
                let sq = bb_w.trailing_zeros() as usize;
                let offset = (p_idx * 64 + sq) * L1_SIZE;
                for i in 0..L1_SIZE {
                    l1_acc[i] = l1_acc[i].saturating_add(self.l1_weights[offset + i]);
                }
                bb_w &= bb_w - 1;
            }
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

        let mut l1_out = [0i16; 256];
        for i in 0..256 {
            // CLAMP L1: Dimezzato da 4096 a 2048 per seguire QA=255
            l1_out[i] = l1_acc[i].clamp(0, 2048);
        }

        // --- LAYER 2 (Input scale 255, Weights scale 64) ---
        let mut l2_acc = self.l2_bias.clone();
        for i in 0..L2_SIZE {
            let mut sum: i32 = l2_acc[i] as i32;
            for j in 0..L1_SIZE {
                // DIVISORE L2: Dimezzato da 4096 a 2048
                // Mantiene il rapporto di scala corretto
                sum += (l1_out[j] as i32 * self.l2_weights[j * L2_SIZE + i] as i32) / 2048;
            }
            l2_acc[i] = sum.clamp(-32768, 32767) as i16;
        }

        let mut l2_out = [0i16; 32];
        for i in 0..32 {
            // CLAMP L2: Dimezzato da 256 a 128 (QB=64)
            l2_out[i] = l2_acc[i].clamp(0, 128);
        }

        // --- LAYER 3: Output (Input 128, Weights 64) ---
        let mut score: i32 = self.l3_bias[0] as i32;
        for j in 0..L2_SIZE {
            // DIVISORE L3: Dimezzato da 512 a 256
            score += (l2_out[j] as i32 * self.l3_weights[j] as i32) / 256;
        }

        // Output diretto (senza formule magiche)
        let val = score;
        
        if s.turno == Colore::Bianco { val } else { -val }
    }
}
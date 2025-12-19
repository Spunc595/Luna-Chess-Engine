use rand::Rng;

#[derive(Clone, Copy)]
pub struct Zobrist {
    pezzi: [[[u64; 2]; 6]; 64], // [casella][pezzo][colore]
    arrocco: [u64; 4],          // [bianco_lato_re, bianco_lato_regina, nero_lato_re, nero_lato_regina]
    en_passant: [u64; 8],       // [lettera]
    colore: u64,                // Valore se è il turno del nero
}

impl Zobrist {
    pub fn nuova() -> Self {
        let mut rng = rand::thread_rng();
        let mut z = Zobrist {
            pezzi: [[[0; 2]; 6]; 64],
            arrocco: [0; 4],
            en_passant: [0; 8],
            colore: 0,
        };
        
        // Inizializza valori casuali
        for i in 0..64 {
            for j in 0..6 {
                for k in 0..2 {
                    z.pezzi[i][j][k] = rng.gen();
                }
            }
        }
        
        for i in 0..4 {
            z.arrocco[i] = rng.gen();
        }
        
        for i in 0..8 {
            z.en_passant[i] = rng.gen();
        }
        
        z.colore = rng.gen();
        
        z
    }
    
    pub fn hash_pezzo(&self, casella: usize, pezzo: usize, colore: usize) -> u64 {
        self.pezzi[casella][pezzo][colore]
    }
    
    pub fn hash_arrocco(&self, indice: usize) -> u64 {
        self.arrocco[indice]
    }
    
    pub fn hash_en_passant(&self, lettera: usize) -> u64 {
        self.en_passant[lettera]
    }
    
    pub fn hash_colore(&self) -> u64 {
        self.colore
    }
}
use crate::board::Mossa;

/// Represents the type of confidence/bound associated with the stored score[cite: 6].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bound {
    None = 0,
    Exact = 1, // Exact score (value perfectly between Alpha and Beta)[cite: 6]
    Alpha = 2, // Upper Bound: the search failed low (score <= alpha)[cite: 6]
    Beta = 3,  // Lower Bound: the search generated a beta-cutoff (score >= beta)[cite: 6]
}

/// Single entry (slot) within the Transposition Table[cite: 6].
/// Optimized in size to maximize CPU cache efficiency[cite: 6].
#[derive(Clone, Copy, Debug)]
pub struct TTEntry {
    pub key: u64,        // Unique Zobrist key of the position[cite: 6]
    pub score: i32,      // Associated evaluation score[cite: 6]
    pub move_data: u16,  // Raw data of the best move found in this position[cite: 6]
    pub depth: u8,       // Remaining search depth at the time of saving[cite: 6]
    pub bound: u8,       // Bound type converted to u8 to save memory space[cite: 6]
    pub generation: u8,  // Identifier of the current search for entry aging[cite: 6]
}

/// Main transposition table of the engine[cite: 6].
/// Implements a direct addressing layout based on a bitwise mask[cite: 6].
pub struct TranspositionTable {
    entries: Vec<TTEntry>,
    mask: usize,
    generation: u8,
}

impl TranspositionTable {
    /// Allocates a new Transposition Table by specifying the desired size in Megabytes[cite: 6].
    /// The actual size of the vectors is forced to the nearest lower or equal power of 2[cite: 6].
    pub fn new(mb_size: usize) -> Self {
        // Calculates how many TTEntry instances can reside in the space expressed in MB[cite: 6].
        let size = (mb_size * 1024 * 1024) / std::mem::size_of::<TTEntry>();
        let mut real_size = 1;
        
        // Force the size to be a power of 2 to optimize the modulo operation[cite: 6].
        while real_size <= size { 
            real_size *= 2; 
        }
        
        TranspositionTable {
            entries: vec![TTEntry { key: 0, score: 0, move_data: 0, depth: 0, bound: 0, generation: 0 }; real_size],
            mask: real_size - 1, // Bitwise mask to calculate the index instantly without divisions[cite: 6]
            generation: 1,
        }
    }

    /// Completely clears the table by zeroing keys and resetting the initial generation[cite: 6].
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            entry.key = 0;
            entry.generation = 0;
        }
        self.generation = 1;
    }

    /// Increments the current generation identifier at the start of each new iterative search[cite: 6].
    /// Uses native wrapping (`wrapping_add`) to avoid 8-bit register overflow[cite: 6].
    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Inspects the table looking for a matching Zobrist key[cite: 6].
    /// If the depth criteria and Alpha/Beta bounds are met, returns the score for an immediate cutoff[cite: 6].
    pub fn probe(&self, key: u64, depth: i32, alpha: i32, beta: i32) -> Option<i32> {
        let idx = (key as usize) & self.mask;
        let entry = &self.entries[idx];

        // Verifies if the stored entry actually matches the requested position[cite: 6].
        if entry.key == key {
            // A cutoff is mathematically valid only if the saved depth is greater than or equal to the current one[cite: 6].
            if entry.depth as i32 >= depth {
                let score = entry.score; 
                let bound = entry.bound;

                // 1. Exact Value: Returns the score directly[cite: 6].
                if bound == Bound::Exact as u8 {
                    return Some(score);
                }
                // 2. Upper Bound (Alpha): Valid only if the score does not improve our minimum Alpha barrier[cite: 6].
                if bound == Bound::Alpha as u8 && score <= alpha {
                    return Some(score);
                }
                // 3. Lower Bound (Beta): Valid if the score causes a pruning above the enemy's Beta barrier[cite: 6].
                if bound == Bound::Beta as u8 && score >= beta {
                    return Some(score);
                }
            }
        }
        None
    }

    /// Retrieves the stored move associated with a given position[cite: 6].
    /// Used in the Move Ordering phase to analyze the previous best move first[cite: 6].
    pub fn get_move(&self, key: u64) -> Mossa {
        let idx = (key as usize) & self.mask;
        let entry = &self.entries[idx];
        
        if entry.key == key {
            Mossa::from_data(entry.move_data)
        } else {
            Mossa::null() // Returns an empty/invalid move if no match is found[cite: 6]
        }
    }

    /// Stores or updates an entry within the table based on a combined replacement policy[cite: 6].
    pub fn store(&mut self, key: u64, depth: i32, score: i32, bound: Bound, best_move: Mossa) {
        let idx = (key as usize) & self.mask;
        let entry = &mut self.entries[idx];

        // Replacement Policy: Overwrites if the slot is empty/different, 
        // if the new move comes from a deeper search, or if the entry belongs to a past search (aging)[cite: 6].
        if entry.key != key || depth as u8 >= entry.depth || entry.generation != self.generation {
            entry.key = key;
            entry.score = score;
            entry.depth = depth as u8;
            entry.bound = bound as u8;
            entry.generation = self.generation;
            
            // Saves compressed binary move data if it is not null[cite: 6].
            if !best_move.is_null() {
                entry.move_data = best_move.data;
            }
        }
    }
}
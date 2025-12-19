use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TipoNodo {
    Esatto,
    LimiteInferiore,
    LimiteSuperiore,
}

pub struct EntryTabellaTransposizione {
    pub hash: u64,
    pub profondita: i32,
    pub tipo_nodo: TipoNodo,
    pub valore: i32,
    pub mossa: Option<crate::movegen::Mossa>,
}

pub struct TabellaTransposizione {
    tabella: HashMap<u64, EntryTabellaTransposizione>,
    dimensione_massima: usize,
}

impl TabellaTransposizione {
    pub fn nuova(dimensione_mb: usize) -> Self {
        let dimensione_massima = dimensione_mb * 1024 * 1024 / std::mem::size_of::<EntryTabellaTransposizione>();
        
        TabellaTransposizione {
            tabella: HashMap::with_capacity(dimensione_massima),
            dimensione_massima,
        }
    }
    
    pub fn inserisci(&mut self, hash: u64, entry: EntryTabellaTransposizione) {
        if self.tabella.len() >= self.dimensione_massima {
            // Svuota parte della tabella se è piena
            self.tabella.clear();
        }
        
        self.tabella.insert(hash, entry);
    }
    
    pub fn cerca(&self, hash: u64) -> Option<&EntryTabellaTransposizione> {
        self.tabella.get(&hash)
    }
    
    pub fn pulisci(&mut self) {
        self.tabella.clear();
    }
}
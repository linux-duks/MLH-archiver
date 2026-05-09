use std::path::Path;

pub struct BRIN {
    /// each Epoch may have multiple blocks, ordered by block_number
    pub block_number: usize,
    /// number of entities in the block
    pub block_size: usize,
    /// the block start has the global order id = `global_block_offset`
    /// the block end has the block_start + block_size
    pub global_block_offset: usize,
    /// block_start: the "entity id" of the initial block in this range
    pub block_start: String,
    /// block_end: the "entity id" of the  block in this range
    pub block_end: String,
}

pub fn read_brin_index(_index_dir: &Path) -> crate::Result<Vec<BRIN>> {
    unimplemented!()
}

pub fn write_brin_index(_index_dir: &Path, _brin: Vec<BRIN>) -> crate::Result<usize> {
    unimplemented!()
}

pub fn init_brin_index(_index_dir: &Path, block_start: &str) -> crate::Result<usize> {
    _ = BRIN {
        block_number: 0,
        block_size: 1,
        global_block_offset: 0,
        block_start: block_start.to_string(),
        block_end: block_start.to_string(),
    };
    unimplemented!()
}

pub fn update_last_block(
    _index_dir: &Path,
    _block_end: &str,
    _block_size: usize,
) -> crate::Result<usize> {
    unimplemented!()
}

#![cfg(test)]

const EXAMPLE_SPV: &'static [u8] = include_bytes!(
    concat!(env!("CARGO_MANIFEST_DIR"), "/build/example.spv"));

crate fn example_spv() -> Vec<u32> {
    let word_size = std::mem::size_of::<u32>();
    assert_eq!(EXAMPLE_SPV.len() % word_size, 0);
    let word_count = EXAMPLE_SPV.len() / word_size;

    let mut words = Vec::with_capacity(word_count);
    unsafe {
        words.set_len(word_count);
        let dst = std::slice::from_raw_parts_mut(
            words.as_mut_ptr() as *mut u8,
            word_count * word_size,
        );
        dst.copy_from_slice(EXAMPLE_SPV);
    }
    words
}

use super::*;

#[cfg(test)]
fn do_test_patch(a: Vec<u8>, b: Vec<u8>, block_size: usize) {
    let b_blocks = compute_blocks(&b, block_size);
    let patch_commands = compute_diff(&a, &b_blocks, block_size);
    let c = if patch_commands.is_synchronized() {
        a
    } else {
        let patch = build_patch(&b, &patch_commands);
        apply_patch(&a, &patch)
    };
    if b.len() < 128 && c.len() < 128 {
        assert_eq!(b, c);
    } else {
        assert_eq!(compute_hash_strong(&b), compute_hash_strong(&c));
    }
}

#[test]
fn test_patch_aaa_bbb() {
    do_test_patch(b"aaa".to_vec(), b"bbb".to_vec(), 2);
}
#[test]
fn test_patch_abcd_cdab() {
    do_test_patch(b"abcd".to_vec(), b"cdab".to_vec(), 2);
}
#[test]
fn test_patch_abcd_abcd() {
    do_test_patch(b"abcd".to_vec(), b"abcd".to_vec(), 2);
}
#[test]
fn test_patch_abcd_abc() {
    do_test_patch(b"abcd".to_vec(), b"abc".to_vec(), 2);
}
#[test]
fn test_patch_abc_abcd() {
    do_test_patch(b"abc".to_vec(), b"abcd".to_vec(), 2);
}

#[test]
fn test_patch_a_b() {
    do_test_patch(b"a".to_vec(), b"b".to_vec(), 2);
}

#[test]
fn test_patch_ab_abc() {
    do_test_patch(b"ab".to_vec(), b"abc".to_vec(), 2);
}

#[test]
fn test_patch_abc_ab() {
    do_test_patch(b"abc".to_vec(), b"ab".to_vec(), 2);
}

#[test]
fn test_patch_1mb_equal() {
    let mut a: Vec<u8> = Vec::new();
    for i in 0..1 << 20 {
        a.push(i as u8);
    }
    let b: Vec<u8> = a.clone();
    do_test_patch(a, b, 2048);
}

#[test]
fn test_patch_1mb_diff_1block() {
    let mut a: Vec<u8> = Vec::new();
    for i in 0..1024*1024 {
        a.push(i as u8);
    }
    let mut b: Vec<u8> = a.clone();
    let difference_pos = 1000123;
    b[difference_pos] += 1;
    let block_size = 32;
    let b_blocks = compute_blocks(&b, block_size);
    let patch_commands = compute_diff(&a, &b_blocks, block_size);
    assert_eq!(patch_commands.other.len(), 1);
    assert_eq!(
        patch_commands.other[0].source as usize,
        (difference_pos / block_size) * block_size
    );
    let patch = build_patch(&b, &patch_commands);
    assert_eq!(patch.data.len(), block_size);
    let c = apply_patch(&a, &patch);
    assert_eq!(compute_hash_strong(&b), compute_hash_strong(&c));
}

#[test]
fn test_patch_128kb_u8_shifted() {
    let mut a: Vec<u8> = Vec::new();
    let mut b: Vec<u8> = Vec::new();    
    for i in 0..128*1024 {
        a.push(i as u8);
        b.push((i as u8).wrapping_add(1));
    }
    let block_size = 32;
    let b_blocks = compute_blocks(&b, block_size);
    let patch_commands = compute_diff(&a, &b_blocks, block_size);
    let patch = build_patch(&b, &patch_commands);
    assert_eq!(patch.data.len(), 0);
    let c = apply_patch(&a, &patch);
    assert_eq!(compute_hash_strong(&b), compute_hash_strong(&c));
}

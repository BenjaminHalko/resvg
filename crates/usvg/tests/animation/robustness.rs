// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Crafted animation: what would take forever, or overflow, is read, as SVG reads it.

use super::*;

/// The begins of the one animation on the rect `r`, which `chain`'s animations time.
fn begins_after(chain: &str, begin: &str) -> Vec<f32> {
    let tree = parse(&format!(
        "<rect width='4' height='4'>{chain}</rect><rect id='r' width='4' height='4'>\
            <animate attributeName='x' from='0' to='9' dur='1s' begin='{begin}'/></rect>"
    ));
    let Some(Node::Path(rect)) = tree.node_by_id("r") else {
        panic!("no rect");
    };
    rect.animations()[0]
        .timing()
        .intervals()
        .iter()
        .map(|interval| interval.interval().begin())
        .collect()
}

#[test]
fn a_begin_naming_two_earlier_animations_each_is_resolved_once() {
    // Each begins 0.1 s after the one before, and names the end of the one before that, which
    // has no duration and so no end: resolving one resolved both, and both theirs, a tree of
    // 1.6^n. 200 never finished.
    let n = 200;
    let anims: String = (0..n)
        .map(|i| {
            let begin = match i {
                0 => "0s".to_string(),
                1 => "a0.begin+0.1s".to_string(),
                _ => format!("a{}.begin+0.1s; a{}.end", i - 1, i - 2),
            };
            format!("<set id='a{i}' attributeName='opacity' to='0.5' begin='{begin}'/>")
        })
        .collect();
    let mut want = 0.0f32;
    for _ in 1..n {
        want += 0.1;
    }
    assert_eq!(begins_after(&anims, &format!("a{}.begin", n - 1)), [want]);
}

#[test]
fn a_chain_of_ends_resolves_each_once() {
    // Each begins as the one before ends: 2,000 took 8 s, each resolved again for every one
    // after it.
    let n = 2000;
    let anims: String = (0..n)
        .map(|i| {
            let begin = if i == 0 {
                "0s".to_string()
            } else {
                format!("a{}.end", i - 1)
            };
            format!(
                "<animate id='a{i}' attributeName='y' from='0' to='1' dur='0.5s' begin='{begin}'/>"
            )
        })
        .collect();
    assert_eq!(
        begins_after(&anims, &format!("a{}.end", n - 1)),
        [n as f32 * 0.5]
    );
}

#[test]
fn a_chain_of_ends_written_backwards_is_read() {
    // The first resolved needs every one after it: past MOST_SYNCBASE_DEPTH it's left
    // unresolved rather than recursing the whole chain.
    let n = 20_000;
    let anims: String = (0..n)
        .map(|i| {
            let begin = if i == n - 1 {
                "0s".to_string()
            } else {
                format!("a{}.end", i + 1)
            };
            format!(
                "<animate id='a{i}' attributeName='y' from='0' to='1' dur='0.5s' begin='{begin}'/>"
            )
        })
        .collect();
    // (The last is 0.5 s long from 0 s: the one before it begins at 0.5 s.)
    assert_eq!(begins_after(&anims, &format!("a{}.end", n - 2)), [1.0]);
}

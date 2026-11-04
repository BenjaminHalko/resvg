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

/// Every animation in `tree`, depth first.
fn animations_in(group: &usvg::Group) -> Vec<std::sync::Arc<usvg::Animation>> {
    let mut out = group.animations().to_vec();
    for child in group.children() {
        match child {
            Node::Group(g) => out.extend(animations_in(g)),
            Node::Path(p) => out.extend(p.animations().iter().cloned()),
            _ => {}
        }
    }
    out
}

#[test]
fn a_motion_turned_by_no_finite_angle_is_not_turned() {
    // `rotate="NaN"` was read as NaN, and `1e39` as infinity.
    for rotate in ["NaN", "inf", "1e39"] {
        let tree = parse(&format!(
            "<rect width='4' height='4'><animateMotion path='M0 0 L10 10' dur='1s' rotate='{rotate}'/></rect>"
        ));
        let animations = animations_in(tree.root());
        let AnimationKind::Motion(track) = animations[0].kind() else {
            panic!("a motion, for {rotate}");
        };
        assert!(
            matches!(track.rotate(), usvg::MotionRotate::Angle(a) if a == 0.0),
            "rotate='{rotate}': {:?}",
            track.rotate()
        );
    }
}

#[test]
fn a_spline_motion_of_splines_that_arent_from_0_to_1_is_in_error() {
    // As `<animate>`'s are: motion read its own splines, NaN and 4 and -3 among them.
    for splines in ["0 NaN 1 1", "0.5 -3 0.5 4", "0 inf 1 1"] {
        let tree = parse(&format!(
            "<rect width='4' height='4'><animateMotion path='M0 0 L50 0' calcMode='spline' keyTimes='0;1' keySplines='{splines}' dur='1s'/></rect>"
        ));
        assert!(
            animations_in(tree.root()).is_empty(),
            "keySplines='{splines}'"
        );
        assert!(
            tree.animation_warnings()
                .iter()
                .any(|w| w.code() == usvg::AnimationWarningCode::InvalidTiming),
            "keySplines='{splines}' is said"
        );
    }
    // (Under another calcMode they're ignored, as SMIL says.)
    let tree = parse(
        "<rect width='4' height='4'><animateMotion path='M0 0 L50 0' calcMode='linear' keyTimes='0;1' keySplines='0 NaN 1 1' dur='1s'/></rect>",
    );
    assert_eq!(animations_in(tree.root()).len(), 1);
}

#[test]
fn a_value_too_big_for_an_f32_is_read_as_inf_is() {
    // 1e39 is infinity as an f32: it was kept, a value of infinity, where `inf` itself is
    // refused.
    for animation in [
        "<animateTransform attributeName='transform' type='rotate' values='0;{v}' dur='1s'/>",
        "<animateTransform attributeName='transform' type='scale' values='1;{v}' dur='1s'/>",
        "<animate attributeName='stroke-width' values='1;{v}' dur='1s'/>",
        "<animate attributeName='stroke-dashoffset' values='0;{v}' dur='1s'/>",
        "<animate attributeName='stroke-dasharray' values='1 2;{v} 2' dur='1s'/>",
        "<animate attributeName='x' values='0;{v}' dur='1s'/>",
    ] {
        let read = |v: &str| {
            let tree = parse(&format!(
                "<rect width='4' height='4' stroke='red' stroke-dasharray='1 2'>{}</rect>",
                animation.replace("{v}", v)
            ));
            let warnings: Vec<_> = tree.animation_warnings().iter().map(|w| w.code()).collect();
            (format!("{:?}", animations_in(tree.root())), warnings)
        };
        let (huge, said) = read("1e39");
        assert_eq!((huge, said.clone()), read("inf"), "{animation}");
        assert!(
            said.iter().any(|c| matches!(
                c,
                usvg::AnimationWarningCode::InvalidValue
                    | usvg::AnimationWarningCode::InvalidGeometryValue
            )),
            "{animation} is said: {said:?}"
        );
    }
}

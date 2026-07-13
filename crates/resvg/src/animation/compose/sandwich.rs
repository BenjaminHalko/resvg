// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use usvg::{Animation, AnimationSource, NodeAnimation};

use super::super::timing::{interval_at, iteration_at, progress};
use super::apply::{fold, ImageState};
use super::SampledOverrides;

/// One animation that contributes to the sandwich at the query time.
pub(super) struct Contribution<'a> {
    pub(super) animation: &'a Animation,
    pub(super) progress: f32,
    pub(super) iteration: u32,
    css: bool,
    begin: f32,
    pub(super) order: usize,
}

/// Samples every animation on `node_anim` at time `t` and folds them into the
/// per-attribute sandwich.
pub(crate) fn sample_overrides(node_anim: &NodeAnimation, t: f32) -> SampledOverrides {
    let mut overrides = SampledOverrides::default();
    if node_anim.base_hidden() {
        overrides.hidden = Some(true);
    }
    let mut image = ImageState::new(node_anim);

    let mut contribution_iter = node_anim
        .animations()
        .iter()
        .enumerate()
        .filter(|(_, animation)| !animation.suppressed_by_important())
        .filter_map(|(order, animation)| build_contribution(animation, order, t));

    let Some(first) = contribution_iter.next() else {
        overrides.image_geometry = image.finish();
        return overrides;
    };

    let Some(second) = contribution_iter.next() else {
        fold(&mut overrides, &mut image, &first);
        overrides.image_geometry = image.finish();
        return overrides;
    };

    let mut contributions = vec![first, second];
    contributions.extend(contribution_iter);

    // SMIL sorts by interval begin (later wins) then document order; CSS sorts
    // after all SMIL contributions, in document order.
    contributions.sort_by(|a, b| {
        a.css
            .cmp(&b.css)
            .then(a.begin.total_cmp(&b.begin))
            .then(a.order.cmp(&b.order))
    });

    for contribution in &contributions {
        fold(&mut overrides, &mut image, contribution);
    }
    overrides.image_geometry = image.finish();
    overrides
}

/// Resolves an animation's progress and priority key at time `t`, or `None` when
/// it contributes nothing.
fn build_contribution(animation: &Animation, order: usize, t: f32) -> Option<Contribution<'_>> {
    let progress = progress(animation.timing(), t)?;
    let css = matches!(animation.source(), AnimationSource::Css);
    let (begin, iteration) = if css {
        (0.0, 0)
    } else {
        let interval = interval_at(animation.timing(), t)?;
        (
            interval.interval().begin(),
            iteration_at(animation.timing(), t),
        )
    };
    Some(Contribution {
        animation,
        progress,
        iteration,
        css,
        begin,
        order,
    })
}

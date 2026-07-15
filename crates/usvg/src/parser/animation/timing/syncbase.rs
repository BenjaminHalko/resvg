// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::intervals::{Begin, active_duration, parse_dur, parse_repeat_count, parse_repeat_dur};
use super::syntax::parse_begin_entry;
use crate::parser::svgtree::{AId, NodeId, SvgNode};

/// A partially-parsed `begin`/`end` value, before syncbase resolution.
#[derive(Clone, Debug)]
pub(crate) enum RawBegin {
    /// A resolved time offset in seconds.
    Offset(f32),
    /// The `indefinite` value.
    Indefinite,
    /// A syncbase reference (`id.begin`/`id.end`) with an offset.
    SyncBase {
        id: String,
        edge: SyncEdge,
        offset: f32,
    },
}

/// The referenced timing edge of a syncbase value.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SyncEdge {
    /// The `begin` edge.
    Begin,
    /// The `end` edge.
    End,
}

/// Resolves a `begin` or `end` attribute into a list of [`Begin`] values.
///
/// An absent attribute yields `[Begin::Offset(0.0)]` when `default_zero` is set
/// (the `begin` default) or an empty list otherwise (the `end` default). A
/// present-but-fully-invalid list yields an empty list.
pub(super) fn resolve_timing_list<'a, 'input>(
    node: SvgNode<'a, 'input>,
    aid: AId,
    default_zero: bool,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
    visiting: &mut Vec<SvgNode<'a, 'input>>,
) -> Vec<Begin> {
    let Some(value) = node.attribute::<&str>(aid) else {
        return if default_zero {
            vec![Begin::Offset(0.0)]
        } else {
            Vec::new()
        };
    };

    let mut out = Vec::new();
    for entry in value.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        let resolved =
            parse_begin_entry(entry).and_then(|raw| resolve_raw(&raw, all_animations, visiting));
        match resolved {
            Some(begin) => out.push(begin),
            None => log::warn!("Unsupported animation begin/end value: '{}'.", entry),
        }
    }
    out
}

/// Resolves a [`RawBegin`] into a concrete [`Begin`], or `None` when a syncbase
/// value cannot be statically resolved.
fn resolve_raw<'a, 'input>(
    raw: &RawBegin,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
    visiting: &mut Vec<SvgNode<'a, 'input>>,
) -> Option<Begin> {
    match raw {
        RawBegin::Offset(seconds) => Some(Begin::Offset(*seconds)),
        RawBegin::Indefinite => Some(Begin::Indefinite),
        RawBegin::SyncBase { id, edge, offset } => {
            resolve_syncbase(id, *edge, *offset, all_animations, visiting).map(Begin::Offset)
        }
    }
}

/// Resolves `id.begin`/`id.end` plus an offset to an absolute instant.
///
/// `x.begin` resolves when `x` has a single static begin. `x.end` resolves only
/// when `x` is statically determinate: a single begin, no explicit end list and
/// a finite active duration. Cyclic references return `None`.
fn resolve_syncbase<'a, 'input>(
    id: &str,
    edge: SyncEdge,
    offset: f32,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
    visiting: &mut Vec<SvgNode<'a, 'input>>,
) -> Option<f32> {
    let target = find_animation_node(id, all_animations)?;
    if visiting.contains(&target) {
        return None;
    }

    visiting.push(target);
    let base = match edge {
        SyncEdge::Begin => resolve_single_begin(target, all_animations, visiting),
        SyncEdge::End => resolve_end_instant(target, all_animations, visiting),
    };
    visiting.pop();

    base.map(|seconds| seconds + offset)
}

/// Finds the animation element with the given `id`.
fn find_animation_node<'a, 'input>(
    id: &str,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
) -> Option<SvgNode<'a, 'input>> {
    all_animations
        .iter()
        .find(|(_, node)| node.element_id() == id)
        .map(|(_, node)| *node)
}

/// Resolves a node's begins, returning the instant only when exactly one static
/// begin exists.
fn resolve_single_begin<'a, 'input>(
    node: SvgNode<'a, 'input>,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
    visiting: &mut Vec<SvgNode<'a, 'input>>,
) -> Option<f32> {
    let mut instants = Vec::new();
    for raw in raw_begin_list(node) {
        if let Some(Begin::Offset(seconds)) = resolve_raw(&raw, all_animations, visiting) {
            instants.push(seconds);
        }
    }
    (instants.len() == 1).then(|| instants[0])
}

/// Resolves a node's end instant when it is statically determinate.
fn resolve_end_instant<'a, 'input>(
    node: SvgNode<'a, 'input>,
    all_animations: &[(NodeId, SvgNode<'a, 'input>)],
    visiting: &mut Vec<SvgNode<'a, 'input>>,
) -> Option<f32> {
    if has_end_entries(node) {
        return None;
    }

    let begin = resolve_single_begin(node, all_animations, visiting)?;
    let active = active_duration(
        parse_dur(node),
        parse_repeat_count(node),
        parse_repeat_dur(node),
    )?;
    Some(begin + active)
}

/// Parses a node's `begin` list into [`RawBegin`] values without warnings.
///
/// An absent `begin` defaults to a single zero offset.
fn raw_begin_list<'a, 'input>(node: SvgNode<'a, 'input>) -> Vec<RawBegin> {
    let Some(value) = node.attribute::<&str>(AId::Begin) else {
        return vec![RawBegin::Offset(0.0)];
    };

    let mut out = Vec::new();
    for entry in value.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if let Some(raw) = parse_begin_entry(entry) {
            out.push(raw);
        }
    }
    out
}

/// Returns whether a node has a non-empty `end` list.
fn has_end_entries<'a, 'input>(node: SvgNode<'a, 'input>) -> bool {
    node.attribute::<&str>(AId::End)
        .is_some_and(|value| value.split(';').any(|entry| !entry.trim().is_empty()))
}

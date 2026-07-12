// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::tree::animation::{Accumulate, Additive, AnimationKind, AnimationVisibility, CalcMode};
use crate::{FillRule, LineCap, LineJoin, Opacity, StrokeMiterlimit};

/// The parsed result of a SMIL value animation.
#[derive(Clone, Debug)]
pub(crate) struct SmilValues {
    /// The typed keyframe data.
    pub(crate) kind: AnimationKind,
    /// The resolved additive behavior.
    pub(crate) additive: Additive,
    /// The resolved accumulate behavior.
    pub(crate) accumulate: Accumulate,
    /// The resolved calculation mode.
    pub(crate) calc_mode: CalcMode,
}

/// The static underlying value used to resolve `to`-only and `by` forms.
#[derive(Clone, Debug)]
pub(crate) enum BaseValue {
    /// No usable base value.
    None,
    /// An `opacity` or `stop-opacity` base.
    Opacity(Opacity),
    /// A `fill`, `stroke`, or `stop-color` base.
    Color(svgtypes::Color),
    /// A scalar base (stroke width, dash offset, geometry).
    Number(f32),
    /// A `stroke-dasharray` base.
    Numbers(Vec<f32>),
    /// A `stroke-miterlimit` base.
    Miterlimit(StrokeMiterlimit),
    /// A `display` base (`true` when shown).
    Boolean(bool),
    /// A `stroke-linecap` base.
    Linecap(LineCap),
    /// A `stroke-linejoin` base.
    Linejoin(LineJoin),
    /// A `fill-rule` base.
    FillRule(FillRule),
    /// A `visibility` base.
    Visibility(AnimationVisibility),
}

impl BaseValue {
    pub(super) fn opacity(&self) -> Option<Opacity> {
        match self {
            BaseValue::Opacity(v) => Some(*v),
            _ => None,
        }
    }

    pub(super) fn color(&self) -> Option<svgtypes::Color> {
        match self {
            BaseValue::Color(v) => Some(*v),
            _ => None,
        }
    }

    pub(super) fn number(&self) -> Option<f32> {
        match self {
            BaseValue::Number(v) => Some(*v),
            _ => None,
        }
    }

    pub(super) fn numbers(&self) -> Option<Vec<f32>> {
        match self {
            BaseValue::Numbers(v) => Some(v.clone()),
            _ => None,
        }
    }

    pub(super) fn miterlimit(&self) -> Option<StrokeMiterlimit> {
        match self {
            BaseValue::Miterlimit(v) => Some(*v),
            _ => None,
        }
    }

    pub(super) fn boolean(&self) -> Option<bool> {
        match self {
            BaseValue::Boolean(v) => Some(*v),
            _ => None,
        }
    }

    pub(super) fn linecap(&self) -> Option<LineCap> {
        match self {
            BaseValue::Linecap(v) => Some(*v),
            _ => None,
        }
    }

    pub(super) fn linejoin(&self) -> Option<LineJoin> {
        match self {
            BaseValue::Linejoin(v) => Some(*v),
            _ => None,
        }
    }

    pub(super) fn fill_rule(&self) -> Option<FillRule> {
        match self {
            BaseValue::FillRule(v) => Some(*v),
            _ => None,
        }
    }

    pub(super) fn visibility(&self) -> Option<AnimationVisibility> {
        match self {
            BaseValue::Visibility(v) => Some(*v),
            _ => None,
        }
    }
}

// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! CSS `@keyframes` extraction and conversion to the typed animation model.
//!
//! `simplecss` only understands selector-based rules, so `@keyframes` blocks are
//! pulled out here before the remaining text is handed to it. The parsed rules
//! are later matched against each element's `animation-*` properties and
//! converted into typed animations.

mod convert;
mod declarations;
mod keyframes;
mod scanner;

pub(crate) use convert::build_css_animations;
pub(crate) use keyframes::*;

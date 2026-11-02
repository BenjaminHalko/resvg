// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Collects animation warnings for the tree being parsed.
//!
//! Warnings are met deep in value and timing parsers that don't carry the
//! converter state, so they go to a per-thread stack of collections: one per
//! tree being parsed (an SVG image inside an SVG parses a tree of its own), and
//! the element being animated as context.

use std::cell::RefCell;

use crate::parser::svgtree::SvgNode;
use crate::tree::animation::{AnimationWarning, AnimationWarningCode};

#[derive(Default)]
struct Collection {
    warnings: Vec<AnimationWarning>,
    elements: Vec<Option<String>>,
}

thread_local! {
    static COLLECTIONS: RefCell<Vec<Collection>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f`, returning what it returns and the warnings reported meanwhile.
pub(crate) fn collect<T>(f: impl FnOnce() -> T) -> (T, Vec<AnimationWarning>) {
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            COLLECTIONS.with(|c| c.borrow_mut().pop());
        }
    }

    COLLECTIONS.with(|c| c.borrow_mut().push(Collection::default()));
    let pop = Pop;
    let value = f();
    let warnings = COLLECTIONS.with(|c| {
        c.borrow_mut()
            .last_mut()
            .map(|collection| std::mem::take(&mut collection.warnings))
            .unwrap_or_default()
    });
    drop(pop);
    (value, warnings)
}

/// Runs `f` with `node` as the animated element warnings are reported on.
pub(crate) fn on_element<T>(node: SvgNode, f: impl FnOnce() -> T) -> T {
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            COLLECTIONS.with(|c| {
                if let Some(collection) = c.borrow_mut().last_mut() {
                    collection.elements.pop();
                }
            });
        }
    }

    let id = node.element_id();
    let element = (!id.is_empty()).then(|| id.to_string());
    COLLECTIONS.with(|c| {
        if let Some(collection) = c.borrow_mut().last_mut() {
            collection.elements.push(element);
        }
    });
    let pop = Pop;
    let value = f();
    drop(pop);
    value
}

/// Reports a warning on the current element, logged and collected once.
pub(crate) fn report(code: AnimationWarningCode, detail: Option<&str>) {
    let element = COLLECTIONS.with(|c| {
        c.borrow()
            .last()
            .and_then(|collection| collection.elements.last().cloned().flatten())
    });
    push(code, element, detail);
}

/// Reports a warning on the element with the `id` given, logged and collected once.
pub(crate) fn report_on(code: AnimationWarningCode, element: Option<&str>, detail: Option<&str>) {
    let element = element.filter(|id| !id.is_empty()).map(str::to_string);
    push(code, element, detail);
}

fn push(code: AnimationWarningCode, element: Option<String>, detail: Option<&str>) {
    COLLECTIONS.with(|c| {
        let mut collections = c.borrow_mut();
        let warning = AnimationWarning {
            code,
            element,
            detail: detail.map(str::to_string),
        };
        match collections.last_mut() {
            Some(collection) if collection.warnings.contains(&warning) => {}
            Some(collection) => {
                log::warn!("{}", warning);
                collection.warnings.push(warning);
            }
            None => log::warn!("{}", warning),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_warning_is_collected_once_and_a_nested_collection_keeps_its_own() {
        let ((_, inner), outer) = collect(|| {
            report(AnimationWarningCode::ExternalCss, None);
            report(AnimationWarningCode::ExternalCss, None);
            collect(|| report(AnimationWarningCode::CssVariable, Some("--x")))
        });
        assert_eq!(outer.len(), 1);
        assert_eq!(outer[0].code(), AnimationWarningCode::ExternalCss);
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].detail(), Some("--x"));
    }
}

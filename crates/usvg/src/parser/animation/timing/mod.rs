// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod clock;
mod easing;
mod intervals;
mod syncbase;
mod syntax;

pub(crate) use easing::parse_easing;
pub(crate) use intervals::parse_smil_timing;
pub(crate) use syncbase::{RawBegin, SyncEdge};

#[cfg(test)]
use crate::parser::svgtree::{NodeId, SvgNode};
#[cfg(test)]
use crate::tree::animation::{CalcMode, Easing, Timing};
#[cfg(test)]
use clock::parse_clock_value;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::svgtree::Document;
    use std::cell::RefCell;
    use std::sync::Once;

    const NS: &str = "http://www.w3.org/2000/svg";

    thread_local! {
        static WARNINGS: RefCell<Option<Vec<String>>> = RefCell::new(None);
    }

    struct CaptureLogger;

    impl log::Log for CaptureLogger {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }

        fn log(&self, record: &log::Record) {
            WARNINGS.with(|slot| {
                if let Some(buffer) = slot.borrow_mut().as_mut() {
                    buffer.push(format!("{}", record.args()));
                }
            });
        }

        fn flush(&self) {}
    }

    /// Captures the warnings emitted on the current thread while `f` runs.
    fn capture<F: FnOnce()>(f: F) -> Vec<String> {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = log::set_boxed_logger(Box::new(CaptureLogger));
            log::set_max_level(log::LevelFilter::Warn);
        });
        WARNINGS.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
        f();
        WARNINGS.with(|slot| slot.borrow_mut().take().unwrap_or_default())
    }

    fn timing_of(svg: &str, id: &str) -> Timing {
        let xml = roxmltree::Document::parse(svg).unwrap();
        let doc = Document::parse_tree(&xml, None).unwrap();
        let all: Vec<(NodeId, SvgNode)> = doc
            .descendants()
            .filter(|node| node.tag_name().map(|t| t.is_animation()).unwrap_or(false))
            .enumerate()
            .map(|(i, node)| (NodeId::from(i), node))
            .collect();
        let node = all
            .iter()
            .find(|(_, node)| node.element_id() == id)
            .map(|(_, node)| *node)
            .expect("animation id not found");
        parse_smil_timing(node, &all)
    }

    fn easing_of(svg: &str, id: &str, values_count: usize) -> Option<Easing> {
        let xml = roxmltree::Document::parse(svg).unwrap();
        let doc = Document::parse_tree(&xml, None).unwrap();
        let node = doc
            .descendants()
            .find(|node| node.element_id() == id)
            .expect("id not found");
        parse_easing(node, values_count)
    }

    fn begins(timing: &Timing) -> Vec<f32> {
        timing
            .intervals()
            .iter()
            .map(|interval| interval.interval().begin())
            .collect()
    }

    #[test]
    fn clock_values() {
        assert_eq!(parse_clock_value("4"), Some(4.0));
        assert_eq!(parse_clock_value("3s"), Some(3.0));
        assert_eq!(parse_clock_value("1.5s"), Some(1.5));
        assert_eq!(parse_clock_value("02:30"), Some(150.0));
        assert_eq!(parse_clock_value("1min"), Some(60.0));
        assert_eq!(parse_clock_value("500ms"), Some(0.5));
        assert_eq!(parse_clock_value("01:00:00"), Some(3600.0));
        assert_eq!(parse_clock_value("bogus"), None);
    }

    #[test]
    fn multi_entry_begin_list() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s;2s;4s' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert_eq!(begins(&timing), vec![0.0, 2.0, 4.0]);
    }

    #[test]
    fn omitted_begin_and_dur() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect><animate id='a' attributeName='opacity'/></rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert_eq!(begins(&timing), vec![0.0]);
        assert_eq!(timing.iteration_dur(), None);
    }

    #[test]
    fn all_invalid_begin_is_empty() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='click;foo' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert!(timing.intervals().is_empty());
    }

    #[test]
    fn event_begin_is_dropped_with_warning() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='click' dur='1s'/>\
             </rect></svg>"
        );
        let warnings = capture(|| {
            let timing = timing_of(&svg, "a");
            assert!(timing.intervals().is_empty());
        });
        assert!(warnings.contains(&"Unsupported animation begin/end value: 'click'.".to_string()));
    }

    #[test]
    fn min_max_are_dropped_with_warning() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='1s' min='1s' max='5s'/>\
             </rect></svg>"
        );
        let warnings = capture(|| {
            let _ = timing_of(&svg, "a");
        });
        assert!(warnings.contains(&"Unsupported SMIL timing attribute: 'min'.".to_string()));
        assert!(warnings.contains(&"Unsupported SMIL timing attribute: 'max'.".to_string()));
    }

    #[test]
    fn syncbase_chain_resolves() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='1s' dur='1s'/>\
             <animate id='b' attributeName='opacity' begin='a.begin+2s' dur='1s'/>\
             <animate id='c' attributeName='opacity' begin='b.begin+3s' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "c");
        assert_eq!(begins(&timing), vec![6.0]);
    }

    #[test]
    fn end_reference_to_indefinite_repeat_drops() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='1s' repeatCount='indefinite'/>\
             <animate id='b' attributeName='opacity' begin='a.end+1s' dur='1s'/>\
             </rect></svg>"
        );
        let warnings = capture(|| {
            let timing = timing_of(&svg, "b");
            assert!(timing.intervals().is_empty());
        });
        assert!(
            warnings.contains(&"Unsupported animation begin/end value: 'a.end+1s'.".to_string())
        );
    }

    #[test]
    fn end_reference_to_multiple_begins_drops() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s;2s' dur='1s'/>\
             <animate id='b' attributeName='opacity' begin='a.end+1s' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "b");
        assert!(timing.intervals().is_empty());
    }

    #[test]
    fn end_reference_to_explicit_end_drops() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='1s' end='5s'/>\
             <animate id='b' attributeName='opacity' begin='a.end+1s' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "b");
        assert!(timing.intervals().is_empty());
    }

    #[test]
    fn end_list_selection() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s;3s' end='2s;5s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 2);
        assert_eq!(intervals[0].interval().begin(), 0.0);
        assert_eq!(intervals[0].interval().end(), Some(2.0));
        assert_eq!(intervals[1].interval().begin(), 3.0);
        assert_eq!(intervals[1].interval().end(), Some(5.0));
    }

    #[test]
    fn end_before_begin_is_skipped() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='5s' end='2s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert!(timing.intervals().is_empty());
    }

    #[test]
    fn identical_begins_collapse() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='1s;1s' dur='2s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert_eq!(timing.intervals().len(), 1);
        assert_eq!(timing.intervals()[0].interval().begin(), 1.0);
    }

    #[test]
    fn restart_never_keeps_first_interval() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s;2s' dur='1s' restart='never'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].interval().begin(), 0.0);
        assert_eq!(intervals[0].interval().end(), Some(1.0));
    }

    #[test]
    fn restart_when_not_active_accepts_after_end() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s;2s' dur='10s' end='1s' restart='whenNotActive'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 2);
        assert_eq!(intervals[0].interval().begin(), 0.0);
        assert_eq!(intervals[0].interval().end(), Some(1.0));
        assert_eq!(intervals[1].interval().begin(), 2.0);
        assert_eq!(intervals[1].interval().end(), Some(12.0));
    }

    #[test]
    fn zero_length_interval() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='1s' end='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].interval().begin(), 1.0);
        assert_eq!(intervals[0].interval().end(), Some(1.0));
    }

    #[test]
    fn zero_duration_set_has_a_zero_length_interval() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect><set id='a' attributeName='opacity' to='0.5' begin='1s' dur='0s' fill='freeze'/></rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert_eq!(timing.iteration_dur(), None);
        assert_eq!(timing.intervals().len(), 1);
        assert_eq!(timing.intervals()[0].interval().begin(), 1.0);
        assert_eq!(timing.intervals()[0].interval().end(), Some(1.0));
    }

    #[test]
    fn cyclic_syncbase_drops() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='b.begin' dur='1s'/>\
             <animate id='b' attributeName='opacity' begin='a.begin' dur='1s'/>\
             </rect></svg>"
        );
        let warnings = capture(|| {
            let timing = timing_of(&svg, "a");
            assert!(timing.intervals().is_empty());
        });
        assert!(
            warnings.contains(&"Unsupported animation begin/end value: 'b.begin'.".to_string())
        );
    }

    #[test]
    fn indefinite_duration_open_interval() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect><animate id='a' attributeName='opacity' begin='0s'/></rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].interval().begin(), 0.0);
        assert_eq!(intervals[0].interval().end(), None);
    }

    #[test]
    fn repeat_count_bounds_active_duration() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='2s' repeatCount='3'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        let intervals = timing.intervals();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].interval().begin(), 0.0);
        assert_eq!(intervals[0].interval().end(), Some(6.0));
    }

    #[test]
    fn fill_and_restart_defaults() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='1s'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert_eq!(timing.intervals()[0].held(), None);
    }

    #[test]
    fn fill_freeze_parsed() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' begin='0s' dur='1s' fill='freeze'/>\
             </rect></svg>"
        );
        let timing = timing_of(&svg, "a");
        assert_eq!(timing.intervals()[0].held(), Some(1.0));
    }

    #[test]
    fn easing_linear_key_times_valid() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='linear' keyTimes='0;0.5;1'/>\
             </rect></svg>"
        );
        let easing = easing_of(&svg, "a", 3).unwrap();
        assert!(matches!(easing.calc_mode(), CalcMode::Linear));
        assert_eq!(easing.key_times().unwrap().len(), 3);
    }

    #[test]
    fn easing_key_times_first_not_zero_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='linear' keyTimes='0.1;1'/>\
             </rect></svg>"
        );
        let warnings = capture(|| assert!(easing_of(&svg, "a", 2).is_none()));
        assert!(warnings.contains(&"Invalid animation timing: '0.1;1'.".to_string()));
    }

    #[test]
    fn easing_key_times_last_not_one_rejected_for_linear() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='linear' keyTimes='0;0.5'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 2).is_none());
    }

    #[test]
    fn easing_key_times_count_mismatch_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='linear' keyTimes='0;1'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 3).is_none());
    }

    #[test]
    fn easing_key_times_non_monotonic_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='linear' keyTimes='0;0.8;0.5;1'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 4).is_none());
    }

    #[test]
    fn easing_spline_valid() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='spline' keyTimes='0;1' keySplines='0 0 1 1'/>\
             </rect></svg>"
        );
        let easing = easing_of(&svg, "a", 2).unwrap();
        assert!(matches!(easing.calc_mode(), CalcMode::Spline));
        assert_eq!(easing.key_splines().unwrap().len(), 1);
    }

    #[test]
    fn easing_spline_out_of_range_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='spline' keyTimes='0;1' keySplines='1.2 0 0 1'/>\
             </rect></svg>"
        );
        let warnings = capture(|| assert!(easing_of(&svg, "a", 2).is_none()));
        assert!(warnings.contains(&"Invalid animation timing: '1.2 0 0 1'.".to_string()));
    }

    #[test]
    fn easing_spline_count_mismatch_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='spline' keyTimes='0;0.5;1' keySplines='0 0 1 1'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 3).is_none());
    }

    #[test]
    fn easing_spline_missing_splines_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='spline' keyTimes='0;1'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 2).is_none());
    }

    #[test]
    fn easing_discrete_waives_last_one() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='discrete' keyTimes='0;0.5'/>\
             </rect></svg>"
        );
        let easing = easing_of(&svg, "a", 2).unwrap();
        assert!(matches!(easing.calc_mode(), CalcMode::Discrete));
        assert_eq!(easing.key_times().unwrap().len(), 2);
    }

    #[test]
    fn easing_discrete_first_not_zero_rejected() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='discrete' keyTimes='0.2;0.5'/>\
             </rect></svg>"
        );
        assert!(easing_of(&svg, "a", 2).is_none());
    }

    #[test]
    fn easing_paced_ignores_key_times_and_splines() {
        let svg = format!(
            "<svg xmlns='{NS}'><rect>\
             <animate id='a' attributeName='opacity' calcMode='paced' keyTimes='bogus' keySplines='bogus'/>\
             </rect></svg>"
        );
        let easing = easing_of(&svg, "a", 2).unwrap();
        assert!(matches!(easing.calc_mode(), CalcMode::Paced));
        assert!(easing.key_times().is_none());
        assert!(easing.key_splines().is_none());
    }
}

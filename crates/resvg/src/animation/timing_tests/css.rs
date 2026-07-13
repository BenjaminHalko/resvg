use super::*;

#[test]
fn matrix_t_c1_delay_before_start_fill_values() {
    let empty = timeline(
        vec![relative(1.0, 1.0, None)],
        Some(1.0),
        Direction::Normal,
        None,
        Some(2.0),
    );
    let backwards = timeline(
        vec![relative(1.0, 1.0, None)],
        Some(1.0),
        Direction::Normal,
        Some(0.0),
        Some(2.0),
    );
    assert_eq!(progress(&empty, 0.5), None);
    assert_eq!(progress(&backwards, 0.5), Some(0.0));
}

#[test]
fn matrix_t_c2_c5_active_negative_delay_and_infinite_iteration() {
    let delayed = timeline(
        vec![relative(-0.5, 2.0, None)],
        Some(2.0),
        Direction::Normal,
        None,
        Some(2.0),
    );
    let infinite = timeline(
        vec![absolute(0.0, None, None)],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    approx(progress(&delayed, 0.0), Some(0.25));
    approx(progress(&infinite, 3.25), Some(0.25));
}

#[test]
fn matrix_t_c2_negative_delay_starts_mid_iteration() {
    let timing = timeline(
        vec![relative(-0.5, 2.0, None)],
        Some(2.0),
        Direction::Normal,
        None,
        Some(2.0),
    );
    assert_eq!(progress(&timing, 0.0), Some(0.25));
}

#[test]
fn matrix_t_c5_infinite_iteration_wraps() {
    let timing = timeline(
        vec![absolute(0.0, None, None)],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    assert_eq!(progress(&timing, 3.25), Some(0.25));
}

#[test]
fn matrix_t_c3_alternate_and_alternate_reverse() {
    let alternate = timeline(
        vec![relative(0.0, 3.0, None)],
        Some(1.0),
        Direction::Alternate,
        None,
        Some(1.0),
    );
    let reverse = timeline(
        vec![relative(0.0, 3.0, None)],
        Some(1.0),
        Direction::AlternateReverse,
        None,
        Some(1.0),
    );
    approx(progress(&alternate, 0.25), Some(0.25));
    approx(progress(&alternate, 1.25), Some(0.75));
    assert_eq!(progress(&reverse, 0.0), Some(1.0));
    assert_eq!(progress(&reverse, 1.0), Some(0.0));
    assert_eq!(progress(&reverse, 2.0), Some(1.0));
}

#[test]
fn matrix_t_c4_c6_terminal_values() {
    let integer = timeline(
        vec![relative(0.0, 2.0, Some(1.0))],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    let fractional = timeline(
        vec![relative(0.0, 2.5, Some(0.5))],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    let zero = timeline(
        vec![relative(0.0, 0.0, Some(0.5))],
        None,
        Direction::Normal,
        None,
        Some(0.0),
    );
    assert_eq!(progress(&integer, 10.0), Some(1.0));
    assert_eq!(progress(&fractional, 10.0), Some(0.5));
    assert_eq!(progress(&zero, 0.0), Some(0.5));
}

#[test]
fn matrix_t_c4_fractional_iterations_hold_the_terminal_progress() {
    let timing = timeline(
        vec![relative(0.0, 2.5, Some(0.5))],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    assert_eq!(progress(&timing, 10.0), Some(0.5));
}

#[test]
fn matrix_t_c6_zero_duration_keeps_fractional_terminal_progress() {
    let timing = timeline(
        vec![relative(0.0, 0.0, Some(0.5))],
        None,
        Direction::Normal,
        None,
        Some(0.0),
    );
    assert_eq!(progress(&timing, 0.0), Some(0.5));
}

#[test]
fn matrix_t_c7_c9_paused_values_are_constant() {
    let active = timeline(
        Vec::new(),
        Some(1.0),
        Direction::Normal,
        Some(0.0),
        Some(1.0),
    );
    let fractional = timeline(
        Vec::new(),
        Some(1.0),
        Direction::Normal,
        Some(0.5),
        Some(1.0),
    );
    for time in [-10.0, 0.0, 10.0] {
        assert_eq!(progress(&active, time), Some(0.0));
        assert_eq!(progress(&fractional, time), Some(0.5));
    }
}

#[test]
fn matrix_t_c7_paused_active_window_keeps_its_initial_progress() {
    let timing = timeline(
        Vec::new(),
        Some(1.0),
        Direction::Normal,
        Some(0.0),
        Some(1.0),
    );
    for time in [-10.0, 0.0, 10.0] {
        assert_eq!(progress(&timing, time), Some(0.0));
    }
}

#[test]
fn matrix_t_c8_c13_paused_fill_none_distinguishes_active_and_terminal() {
    let active = timeline(
        Vec::new(),
        Some(2.0),
        Direction::Normal,
        Some(0.25),
        Some(2.0),
    );
    let terminal = timeline(Vec::new(), Some(2.0), Direction::Normal, None, Some(2.0));
    for time in [-10.0, 0.0, 10.0] {
        assert_eq!(progress(&active, time), Some(0.25));
        assert_eq!(progress(&terminal, time), None);
    }
}

#[test]
fn matrix_t_c10_retains_large_delay_relative_duration() {
    let timing = timeline(
        vec![relative(1_000_000_000.0, 2.5, Some(0.5))],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1_000_000_000.0),
    );
    assert_eq!(progress(&timing, 1_000_000_128.0), Some(0.5));
}

#[test]
fn matrix_t_c11_c12_keep_independent_duration() {
    let running = timeline(
        vec![relative(-0.5, 2.0, Some(1.0))],
        Some(2.0),
        Direction::Normal,
        None,
        Some(2.0),
    );
    let paused = timeline(Vec::new(), Some(2.0), Direction::Normal, None, Some(7.0));
    assert_eq!(running.one_loop_end(), Some(2.0));
    assert_eq!(paused.one_loop_end(), Some(7.0));
}

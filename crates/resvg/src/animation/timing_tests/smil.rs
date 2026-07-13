use super::*;

#[test]
fn matrix_t_s1_active_wrap() {
    let timing = timeline(
        vec![absolute(0.0, Some(4.0), None)],
        Some(2.0),
        Direction::Normal,
        None,
        Some(2.0),
    );
    for (time, expected) in [
        (0.0, 0.0),
        (0.5, 0.25),
        (1.0, 0.5),
        (2.0, 0.0),
        (3.99, 0.995),
    ] {
        approx(progress(&timing, time), Some(expected));
    }
}

#[test]
fn matrix_t_s2_s4_frozen_values() {
    let integer = timeline(
        vec![absolute(0.0, Some(2.0), Some(1.0))],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    let fractional = timeline(
        vec![absolute(0.0, Some(2.5), Some(0.5))],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    let clipped = timeline(
        vec![absolute(0.0, Some(3.0), Some(0.3))],
        Some(10.0),
        Direction::Normal,
        None,
        Some(10.0),
    );
    approx(progress(&integer, 10.0), Some(1.0));
    approx(progress(&fractional, 10.0), Some(0.5));
    approx(progress(&clipped, 5.0), Some(0.3));
}

#[test]
fn matrix_t_s3_mid_iteration_freeze() {
    let timing = timeline(
        vec![absolute(0.0, Some(2.5), Some(0.5))],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    assert_eq!(progress(&timing, 10.0), Some(0.5));
}

#[test]
fn matrix_t_s4_clipped_freeze() {
    let timing = timeline(
        vec![absolute(0.0, Some(3.0), Some(0.3))],
        Some(10.0),
        Direction::Normal,
        None,
        Some(10.0),
    );
    assert_eq!(progress(&timing, 5.0), Some(0.3));
}

#[test]
fn matrix_t_s5_freeze_across_gap_and_takeover() {
    let timing = timeline(
        vec![
            absolute(0.0, Some(1.0), Some(1.0)),
            absolute(3.0, Some(4.0), Some(1.0)),
        ],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    assert_eq!(progress(&timing, 2.0), Some(1.0));
    assert_eq!(progress(&timing, 3.0), Some(0.0));
    assert_eq!(progress(&timing, 3.5), Some(0.5));
}

#[test]
fn matrix_t_s6_s9_absent_and_open_intervals() {
    let remove = timeline(
        vec![absolute(0.0, Some(1.0), None)],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    let before = timeline(
        vec![absolute(1.0, Some(2.0), Some(1.0))],
        Some(1.0),
        Direction::Normal,
        None,
        Some(2.0),
    );
    let open = timeline(
        vec![absolute(0.0, None, None)],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    let indefinite = timeline(
        vec![absolute(0.0, None, None)],
        None,
        Direction::Normal,
        None,
        None,
    );
    assert_eq!(progress(&remove, 1.0), None);
    assert_eq!(progress(&before, 0.5), None);
    approx(progress(&open, 5.5), Some(0.5));
    assert_eq!(progress(&indefinite, 1.0), Some(0.0));
}

#[test]
fn matrix_t_s6_remove_contributes_nothing_after_end() {
    let timing = timeline(
        vec![absolute(0.0, Some(1.0), None)],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    assert_eq!(progress(&timing, 1.0), None);
}

#[test]
fn matrix_t_s7_before_first_interval_contributes_nothing() {
    let timing = timeline(
        vec![absolute(1.0, Some(2.0), Some(1.0))],
        Some(1.0),
        Direction::Normal,
        None,
        Some(2.0),
    );
    assert_eq!(progress(&timing, 0.5), None);
}

#[test]
fn matrix_t_s8_open_interval_remains_active() {
    let timing = timeline(
        vec![absolute(0.0, None, None)],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    assert_eq!(progress(&timing, 5.5), Some(0.5));
}

#[test]
fn matrix_t_s9_indefinite_and_zero_iteration_duration_hold_start() {
    let indefinite = timeline(
        vec![absolute(0.0, None, None)],
        None,
        Direction::Normal,
        None,
        None,
    );
    let zero = timeline(
        vec![absolute(0.0, None, None)],
        None,
        Direction::Normal,
        None,
        None,
    );
    assert_eq!(progress(&indefinite, 1.0), Some(0.0));
    assert_eq!(progress(&zero, 1.0), Some(0.0));
}

#[test]
fn matrix_t_s10_keeps_just_below_integer_hold_exact() {
    let timing = timeline(
        vec![absolute(0.0, Some(0.999_999_94), Some(0.999_999_94))],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    assert_eq!(progress(&timing, 0.999_999_94), Some(0.999_999_94));
}

#[test]
fn matrix_t_s11_uses_each_interval_hold() {
    let timing = timeline(
        vec![
            absolute(0.0, Some(0.5), Some(0.5)),
            absolute(3.0, Some(4.0), Some(1.0)),
        ],
        Some(1.0),
        Direction::Normal,
        None,
        Some(1.0),
    );
    assert_eq!(progress(&timing, 2.0), Some(0.5));
    assert_eq!(progress(&timing, 3.0), Some(0.0));
    assert_eq!(progress(&timing, 3.5), Some(0.5));
}

#[test]
fn matrix_t_s12_keeps_duration_independent_from_runtime_interval() {
    let timing = timeline(
        vec![absolute(-0.5, Some(1.5), Some(1.0))],
        Some(2.0),
        Direction::Normal,
        None,
        Some(1.5),
    );
    assert_eq!(timing.one_loop_end(), Some(1.5));
    approx(progress(&timing, 0.0), Some(0.25));
}

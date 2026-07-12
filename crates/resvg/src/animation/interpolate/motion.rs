// Copyright 2026 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tiny_skia::{Path, PathSegment, Point, Transform};
use usvg::{CalcMode, Easing, MotionRotate, MotionTrack};

use super::super::easing::key_spline;
use super::locate::{bracket_offsets, lerp};

/// Flatness tolerance in pixels for curve subdivision.
const FLATNESS: f32 = 0.1;
/// Maximum recursion depth for adaptive curve subdivision.
const MAX_DEPTH: u8 = 16;

/// A cumulative arc-length table over a flattened motion path.
pub(super) struct ArcLength {
    points: Vec<Point>,
    cumulative: Vec<f32>,
    pub(super) total: f32,
}

impl ArcLength {
    /// Flattens `path` and builds its cumulative arc-length table.
    ///
    /// Returns `None` for a path with no drawable length.
    pub(super) fn build(path: &Path) -> Option<ArcLength> {
        let mut points: Vec<Point> = Vec::new();
        let mut cumulative: Vec<f32> = Vec::new();
        let mut total = 0.0;
        let mut current = Point::from_xy(0.0, 0.0);
        let mut subpath_start = Point::from_xy(0.0, 0.0);

        for segment in path.segments() {
            match segment {
                PathSegment::MoveTo(p) => {
                    // A move introduces a gap: the point advances with no length.
                    points.push(p);
                    cumulative.push(total);
                    current = p;
                    subpath_start = p;
                }
                PathSegment::LineTo(p) => {
                    total += distance(current, p);
                    points.push(p);
                    cumulative.push(total);
                    current = p;
                }
                PathSegment::QuadTo(c, p) => {
                    let mut flattened = Vec::new();
                    flatten_quad(current, c, p, 0, &mut flattened);
                    for point in flattened {
                        total += distance(current, point);
                        points.push(point);
                        cumulative.push(total);
                        current = point;
                    }
                }
                PathSegment::CubicTo(c1, c2, p) => {
                    let mut flattened = Vec::new();
                    flatten_cubic(current, c1, c2, p, 0, &mut flattened);
                    for point in flattened {
                        total += distance(current, point);
                        points.push(point);
                        cumulative.push(total);
                        current = point;
                    }
                }
                PathSegment::Close => {
                    total += distance(current, subpath_start);
                    points.push(subpath_start);
                    cumulative.push(total);
                    current = subpath_start;
                }
            }
        }

        if points.len() < 2 || total <= 0.0 {
            return None;
        }
        Some(ArcLength {
            points,
            cumulative,
            total,
        })
    }

    /// Returns the point and tangent angle (in degrees) at `distance_along`.
    pub(super) fn sample(&self, distance_along: f32) -> (Point, f32) {
        let target = distance_along.clamp(0.0, self.total);
        let segment = self.segment_index(target);
        let start = self.cumulative[segment];
        let span = self.cumulative[segment + 1] - start;
        let local = if span > 0.0 {
            (target - start) / span
        } else {
            0.0
        };
        let p0 = self.points[segment];
        let p1 = self.points[segment + 1];
        let point = Point::from_xy(lerp(p0.x, p1.x, local), lerp(p0.y, p1.y, local));
        (point, tangent_angle(p0, p1))
    }

    /// Finds the polyline segment containing `target`.
    fn segment_index(&self, target: f32) -> usize {
        let count = self.cumulative.len();
        let found = self.cumulative.partition_point(|&c| c <= target);
        found.saturating_sub(1).min(count - 2)
    }
}

/// Samples an `animateMotion` track into its local transform.
pub(super) fn sample_motion(
    track: &MotionTrack,
    easing: &Easing,
    progress: f32,
) -> Option<Transform> {
    let table = ArcLength::build(track.path())?;
    let fraction = motion_fraction(track, easing, progress.clamp(0.0, 1.0));
    let (point, tangent) = table.sample(fraction * table.total);
    let angle = match track.rotate() {
        MotionRotate::Auto => tangent,
        MotionRotate::AutoReverse => tangent + 180.0,
        MotionRotate::Angle(fixed) => fixed,
    };
    Some(Transform::from_translate(point.x, point.y).pre_concat(Transform::from_rotate(angle)))
}

/// Maps `progress` onto a path fraction in `0.0..=1.0`.
///
/// With `keyPoints` the fraction is looked up through the paired `keyTimes`
/// (spline-eased when requested); otherwise the fraction follows progress
/// directly, which under the default `paced` mode yields constant velocity.
fn motion_fraction(track: &MotionTrack, easing: &Easing, progress: f32) -> f32 {
    match track.key_points() {
        Some(key_points) if key_points.len() >= 2 => {
            let offsets: Vec<f32> = match easing.key_times() {
                Some(times) if times.len() == key_points.len() => {
                    times.iter().map(|t| t.get()).collect()
                }
                _ => uniform_offsets(key_points.len()),
            };
            let (lo, hi, local) = bracket_offsets(&offsets, progress);
            let eased = match easing.calc_mode() {
                CalcMode::Spline => easing
                    .key_splines()
                    .and_then(|splines| splines.get(lo))
                    .map(|spline| key_spline(*spline, local))
                    .unwrap_or(local),
                _ => local,
            };
            lerp(key_points[lo].get(), key_points[hi].get(), eased).clamp(0.0, 1.0)
        }
        _ => match easing.calc_mode() {
            CalcMode::Spline => easing
                .key_splines()
                .and_then(|splines| splines.first())
                .map(|spline| key_spline(*spline, progress))
                .unwrap_or(progress),
            _ => progress,
        },
    }
}

/// Evenly spaces `count` offsets across `0.0..=1.0`.
fn uniform_offsets(count: usize) -> Vec<f32> {
    if count <= 1 {
        return vec![0.0];
    }
    (0..count)
        .map(|i| i as f32 / (count as f32 - 1.0))
        .collect()
}

/// The Euclidean distance between two points.
pub(super) fn distance(a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

/// The tangent direction from `a` to `b`, in degrees.
fn tangent_angle(a: Point, b: Point) -> f32 {
    (b.y - a.y).atan2(b.x - a.x).to_degrees()
}

/// The midpoint of two points.
fn midpoint(a: Point, b: Point) -> Point {
    Point::from_xy((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
}

/// Adaptively flattens a quadratic curve, appending points up to `p2`.
fn flatten_quad(p0: Point, p1: Point, p2: Point, depth: u8, out: &mut Vec<Point>) {
    if depth >= MAX_DEPTH || perpendicular_distance(p0, p2, p1) <= FLATNESS {
        out.push(p2);
        return;
    }
    let p01 = midpoint(p0, p1);
    let p12 = midpoint(p1, p2);
    let p012 = midpoint(p01, p12);
    flatten_quad(p0, p01, p012, depth + 1, out);
    flatten_quad(p012, p12, p2, depth + 1, out);
}

/// Adaptively flattens a cubic curve, appending points up to `p3`.
fn flatten_cubic(p0: Point, p1: Point, p2: Point, p3: Point, depth: u8, out: &mut Vec<Point>) {
    let flat = perpendicular_distance(p0, p3, p1) <= FLATNESS
        && perpendicular_distance(p0, p3, p2) <= FLATNESS;
    if depth >= MAX_DEPTH || flat {
        out.push(p3);
        return;
    }
    let p01 = midpoint(p0, p1);
    let p12 = midpoint(p1, p2);
    let p23 = midpoint(p2, p3);
    let p012 = midpoint(p01, p12);
    let p123 = midpoint(p12, p23);
    let p0123 = midpoint(p012, p123);
    flatten_cubic(p0, p01, p012, p0123, depth + 1, out);
    flatten_cubic(p0123, p123, p23, p3, depth + 1, out);
}

/// The perpendicular distance of `p` from the line through `a` and `b`.
fn perpendicular_distance(a: Point, b: Point, p: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length < f32::EPSILON {
        return distance(a, p);
    }
    ((p.x - a.x) * dy - (p.y - a.y) * dx).abs() / length
}

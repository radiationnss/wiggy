use crate::window::geometry::Point;
use std::time::Duration;

#[cfg(target_os = "linux")]
pub(crate) mod service;

pub struct WiggleDetector {
    previous: Option<(Point, Duration)>,
    start: Duration,
    anchor: Point,
    extreme: Point,
    direction: Option<Point>,
    completed: u8,
    last_trigger: Option<Duration>,
}

impl Default for WiggleDetector {
    fn default() -> Self {
        Self {
            previous: None,
            start: Duration::ZERO,
            anchor: Point { x: 0.0, y: 0.0 },
            extreme: Point { x: 0.0, y: 0.0 },
            direction: None,
            completed: 0,
            last_trigger: None,
        }
    }
}

impl WiggleDetector {
    pub fn reset(&mut self) {
        let last_trigger = self.last_trigger;
        *self = Self {
            last_trigger,
            ..Self::default()
        };
    }

    fn seed(&mut self, point: Point, time: Duration) {
        self.reset();
        self.previous = Some((point, time));
        self.start = time;
        self.anchor = point;
        self.extreme = point;
    }

    pub fn sample(
        &mut self,
        point: Point,
        time: Duration,
        sensitivity: f64,
        cooldown: Duration,
    ) -> bool {
        if !point.x.is_finite()
            || !point.y.is_finite()
            || !sensitivity.is_finite()
            || !(0.5..=2.0).contains(&sensitivity)
        {
            self.reset();
            return false;
        }
        let Some((previous, previous_time)) = self.previous else {
            self.seed(point, time);
            return false;
        };
        let Some(elapsed) = time.checked_sub(previous_time) else {
            self.seed(point, time);
            return false;
        };
        let blocked = self.last_trigger.is_some_and(|trigger| {
            time.checked_sub(trigger)
                .is_none_or(|elapsed| elapsed < cooldown)
        });
        let delta_x = point.x - previous.x;
        let delta_y = point.y - previous.y;
        if blocked
            || elapsed > Duration::from_millis(160)
            || (self.direction.is_some()
                && time.saturating_sub(self.start) > Duration::from_millis(650))
            || delta_x.hypot(delta_y) > 300.0
        {
            self.seed(point, time);
            return false;
        }
        if elapsed.is_zero() {
            return false;
        }
        self.previous = Some((point, time));
        let threshold = 48.0 / sensitivity;
        if self.direction.is_none() {
            let offset = displacement(self.anchor, point);
            let length = offset.x.hypot(offset.y);
            if length < 3.0 {
                return false;
            }
            self.direction = Some(Point {
                x: offset.x / length,
                y: offset.y / length,
            });
            self.start = time;
            self.extreme = point;
        }
        let direction = self.direction.unwrap();
        let offset = displacement(self.extreme, point);
        let distance = offset.x.hypot(offset.y);
        let forward = offset.x * direction.x + offset.y * direction.y;
        if forward > 0.0 {
            self.extreme = point;
            let stroke = displacement(self.anchor, self.extreme);
            let length = stroke.x.hypot(stroke.y);
            self.direction = Some(Point {
                x: stroke.x / length,
                y: stroke.y / length,
            });
        } else if distance >= threshold && -forward >= distance * 0.5 {
            let stroke = displacement(self.anchor, self.extreme);
            if stroke.x.hypot(stroke.y) >= threshold {
                self.completed += 1;
            } else {
                self.completed = 0;
            }
            self.anchor = self.extreme;
            self.extreme = point;
            self.direction = Some(Point {
                x: offset.x / distance,
                y: offset.y / distance,
            });
        }
        let stroke = displacement(self.anchor, self.extreme);
        if self.completed >= 3 && stroke.x.hypot(stroke.y) >= threshold {
            self.last_trigger = Some(time);
            self.seed(point, time);
            return true;
        }
        false
    }
}

fn displacement(from: Point, to: Point) -> Point {
    Point {
        x: to.x - from.x,
        y: to.y - from.y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn replay(points: &[(f64, f64)], interval: u64, sensitivity: f64) -> usize {
        let mut detector = WiggleDetector::default();
        points
            .iter()
            .enumerate()
            .filter(|(index, (x, y))| {
                detector.sample(
                    Point { x: *x, y: *y },
                    Duration::from_millis(*index as u64 * interval),
                    sensitivity,
                    Duration::from_millis(2500),
                )
            })
            .count()
    }
    #[test]
    fn idle_polling_does_not_consume_the_gesture_time_window() {
        let mut detector = WiggleDetector::default();
        for index in 0..20 {
            assert!(!detector.sample(
                Point { x: 0.0, y: 0.0 },
                Duration::from_millis(index * 33),
                1.0,
                Duration::from_millis(2500),
            ));
        }
        let mut triggers = 0;
        for (index, horizontal) in [60.0, 0.0, 60.0, 0.0].into_iter().enumerate() {
            triggers += usize::from(detector.sample(
                Point {
                    x: horizontal,
                    y: 0.0,
                },
                Duration::from_millis(660 + index as u64 * 50),
                1.0,
                Duration::from_millis(2500),
            ));
        }
        assert_eq!(triggers, 1);
    }

    #[test]
    fn deliberate_four_strokes_fire_once() {
        let points = [
            (0.0, 0.0),
            (60.0, 1.0),
            (0.0, 0.0),
            (60.0, 1.0),
            (0.0, 0.0),
            (60.0, 0.0),
            (0.0, 0.0),
        ];
        assert_eq!(replay(&points, 60, 1.0), 1);
    }
    #[test]
    fn rejects_normal_motion_jitter_drift_and_slow_sweeps() {
        assert_eq!(
            replay(
                &[(0.0, 0.0), (100.0, 0.0), (200.0, 0.0), (250.0, 0.0)],
                33,
                1.0
            ),
            0
        );
        assert_eq!(
            replay(
                &[(0.0, 0.0), (4.0, 0.0), (0.0, 0.0), (4.0, 0.0), (0.0, 0.0)],
                33,
                2.0
            ),
            0
        );
        assert_eq!(
            replay(
                &[
                    (0.0, 0.0),
                    (60.0, 100.0),
                    (0.0, 200.0),
                    (60.0, 300.0),
                    (0.0, 400.0)
                ],
                60,
                1.0
            ),
            0
        );
        assert_eq!(
            replay(
                &[(0.0, 0.0), (60.0, 0.0), (0.0, 0.0), (60.0, 0.0), (0.0, 0.0)],
                250,
                1.0
            ),
            0
        );
    }
    #[test]
    fn detects_wiggles_at_every_angle_with_the_same_sensitivity() {
        for degrees in (0..360).step_by(15) {
            let angle = f64::from(degrees).to_radians();
            for (length, sensitivity, expected) in [(60.0, 1.0, 1), (30.0, 1.0, 0), (30.0, 2.0, 1)]
            {
                let points: Vec<_> = [0.0, length, 0.0, length, 0.0]
                    .into_iter()
                    .map(|distance| {
                        (
                            400.0 + distance * angle.cos(),
                            300.0 + distance * angle.sin(),
                        )
                    })
                    .collect();
                assert_eq!(
                    replay(&points, 50, sensitivity),
                    expected,
                    "angle {degrees}, sensitivity {sensitivity}"
                );
            }
        }
    }

    #[test]
    fn accumulates_small_vertical_and_diagonal_samples() {
        for degrees in [45, 90, 135, 225, 270, 315] {
            let angle = f64::from(degrees).to_radians();
            let mut points = vec![(0.0, 0.0)];
            for direction in [1.0, -1.0, 1.0, -1.0] {
                for _sample in 0..6 {
                    let previous = *points.last().unwrap();
                    points.push((
                        previous.0 + direction * 10.0 * angle.cos(),
                        previous.1 + direction * 10.0 * angle.sin(),
                    ));
                }
            }
            assert_eq!(replay(&points, 20, 1.0), 1, "angle {degrees}");
        }
    }

    #[test]
    fn accepts_wiggles_that_change_direction_between_strokes() {
        let points = [
            (0.0, 0.0),
            (60.0, 0.0),
            (0.0, 30.0),
            (60.0, 60.0),
            (0.0, 90.0),
        ];
        assert_eq!(replay(&points, 60, 1.0), 1);
    }

    #[test]
    fn rejects_vertical_and_diagonal_jitter_straight_motion_and_jumps() {
        for degrees in [45, 90, 135, 270] {
            let angle = f64::from(degrees).to_radians();
            for distances in [
                vec![0.0, 4.0, 0.0, 4.0, 0.0],
                vec![0.0, 60.0, 120.0, 180.0, 240.0],
                vec![0.0, 600.0, 0.0, 600.0, 0.0],
            ] {
                let points: Vec<_> = distances
                    .into_iter()
                    .map(|distance| (distance * angle.cos(), distance * angle.sin()))
                    .collect();
                assert_eq!(replay(&points, 33, 2.0), 0, "angle {degrees}");
            }
        }
    }

    #[test]
    fn sensitivity_changes_required_stroke_length() {
        let points = [(0.0, 0.0), (30.0, 0.0), (0.0, 0.0), (30.0, 0.0), (0.0, 0.0)];
        assert_eq!(replay(&points, 50, 1.0), 0);
        assert_eq!(replay(&points, 50, 2.0), 1);
    }
    #[test]
    fn accumulates_small_samples_into_strokes() {
        let mut points = vec![(0.0, 0.0)];
        for direction in [1.0, -1.0, 1.0, -1.0] {
            for _sample in 0..6 {
                points.push((points.last().unwrap().0 + direction * 10.0, 0.0));
            }
        }
        assert_eq!(replay(&points, 20, 1.0), 1);
    }
    #[test]
    fn resumes_after_cooldown_and_rejects_jumps() {
        let mut detector = WiggleDetector::default();
        let mut triggers = 0;
        for base in [0, 500, 3000] {
            for (index, horizontal) in [0.0, 60.0, 0.0, 60.0, 0.0].iter().enumerate() {
                if detector.sample(
                    Point {
                        x: *horizontal,
                        y: 0.0,
                    },
                    Duration::from_millis(base + index as u64 * 50),
                    1.0,
                    Duration::from_millis(2500),
                ) {
                    triggers += 1;
                }
            }
        }
        assert_eq!(triggers, 2);
        assert_eq!(
            replay(
                &[
                    (0.0, 0.0),
                    (600.0, 0.0),
                    (0.0, 0.0),
                    (600.0, 0.0),
                    (0.0, 0.0)
                ],
                33,
                1.0
            ),
            0
        );
    }
    #[test]
    fn resets_on_invalid_input_and_time_regression() {
        let mut detector = WiggleDetector::default();
        for (point, time) in [
            (Point { x: 0.0, y: 0.0 }, 200),
            (Point { x: 60.0, y: 0.0 }, 250),
            (Point { x: 0.0, y: 0.0 }, 100),
            (
                Point {
                    x: f64::NAN,
                    y: 0.0,
                },
                150,
            ),
        ] {
            assert!(!detector.sample(
                point,
                Duration::from_millis(time),
                1.0,
                Duration::from_millis(2500)
            ));
        }
    }
}

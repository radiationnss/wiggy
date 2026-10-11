use crate::config::PopupPosition;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Monitor {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    #[serde(default)]
    pub transform: u8,
    #[serde(default)]
    pub reserved: [f64; 4],
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default, rename = "mirrorOf")]
    pub mirror_of: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Monitor {
    fn extent(&self) -> Option<(f64, f64)> {
        if self.disabled
            || (!self.mirror_of.is_empty() && self.mirror_of != "none")
            || !self.scale.is_finite()
            || self.scale <= 0.0
            || self.width <= 0.0
            || self.height <= 0.0
        {
            return None;
        }
        let (width, height) = if self.transform % 2 == 1 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        };
        Some((width / self.scale, height / self.scale))
    }

    pub fn contains(&self, cursor: Point) -> bool {
        self.extent().is_some_and(|(width, height)| {
            cursor.x >= self.x
                && cursor.x < self.x + width
                && cursor.y >= self.y
                && cursor.y < self.y + height
        })
    }

    pub fn place(
        &self,
        cursor: Point,
        position: PopupPosition,
        requested_height: u32,
    ) -> Result<Placement, String> {
        let (width, height) = self.extent().ok_or("Invalid monitor geometry")?;
        let [left, top, right, bottom] = self.reserved;
        let minimum_x = (self.x + left.max(0.0) + 8.0).ceil();
        let minimum_y = (self.y + top.max(0.0) + 8.0).ceil();
        let available_width = (self.x + width - right.max(0.0) - 8.0 - minimum_x).floor();
        let available_height = (self.y + height - bottom.max(0.0) - 8.0 - minimum_y).floor();
        if available_width < 1.0 || available_height < 1.0 {
            return Err("Monitor work area is empty".into());
        }
        let popup_width = available_width.min(f64::from(super::WIDTH));
        let popup_height = available_height.min(f64::from(requested_height));
        let maximum_x = minimum_x + available_width - popup_width;
        let maximum_y = minimum_y + available_height - popup_height;
        let (target_x, target_y) = match position {
            PopupPosition::NearCursor => (cursor.x + 18.0, cursor.y + 18.0),
            PopupPosition::TopLeft => (minimum_x, minimum_y),
            PopupPosition::TopRight => (maximum_x, minimum_y),
            PopupPosition::BottomLeft => (minimum_x, maximum_y),
            PopupPosition::BottomRight => (maximum_x, maximum_y),
        };
        Ok(Placement {
            x: target_x.clamp(minimum_x, maximum_x).round() as i32,
            y: target_y.clamp(minimum_y, maximum_y).round() as i32,
            width: popup_width as u32,
            height: popup_height as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn monitor() -> Monitor {
        Monitor {
            name: "test".into(),
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
            scale: 1.0,
            transform: 0,
            reserved: [0.0, 39.0, 0.0, 0.0],
            focused: true,
            disabled: false,
            mirror_of: "none".into(),
        }
    }
    #[test]
    fn clamps_bottom_right_and_reserved_top() {
        let placement = monitor()
            .place(
                Point {
                    x: 1919.0,
                    y: 1079.0,
                },
                PopupPosition::NearCursor,
                260,
            )
            .unwrap();
        assert_eq!((placement.x, placement.y), (1432, 812));
        assert_eq!(
            monitor()
                .place(Point { x: 0.0, y: 0.0 }, PopupPosition::NearCursor, 260)
                .unwrap()
                .y,
            47
        );
    }
    #[test]
    fn handles_rotated_scaled_negative_monitor() {
        let display = Monitor {
            x: -720.0,
            scale: 1.5,
            transform: 1,
            ..monitor()
        };
        assert!(display.contains(Point { x: -1.0, y: 1200.0 }));
        assert!(!display.contains(Point { x: 1.0, y: 1200.0 }));
        let placement = display
            .place(Point { x: -1.0, y: 1279.0 }, PopupPosition::NearCursor, 260)
            .unwrap();
        assert_eq!(placement.x, -488);
        assert_eq!(placement.y, 1012);
    }
    #[test]
    fn shrinks_to_small_work_area() {
        let display = Monitor {
            width: 400.0,
            height: 300.0,
            ..monitor()
        };
        let placement = display
            .place(Point { x: 399.0, y: 299.0 }, PopupPosition::NearCursor, 620)
            .unwrap();
        assert_eq!((placement.width, placement.height), (384, 245));
        assert_eq!((placement.x, placement.y), (8, 47));
    }

    #[test]
    fn pins_all_corners_with_expanded_height() {
        for (position, expected) in [
            (PopupPosition::TopLeft, (8, 47)),
            (PopupPosition::TopRight, (1432, 47)),
            (PopupPosition::BottomLeft, (8, 452)),
            (PopupPosition::BottomRight, (1432, 452)),
        ] {
            let placement = monitor()
                .place(Point { x: 400.0, y: 400.0 }, position, 620)
                .unwrap();
            assert_eq!((placement.x, placement.y), expected);
            assert_eq!(placement.height, 620);
        }
    }
}

//! Tray placement owns all physical-coordinate calculations.

#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    fn right(self) -> f64 {
        self.x + self.width
    }
    fn bottom(self) -> f64 {
        self.y + self.height
    }
}

pub(super) fn place(work: Bounds, anchor: Bounds, scale: f64, desired_height: f64) -> Bounds {
    let margin = 8.0 * scale;
    let width = (360.0 * scale).min((work.width - margin * 2.0).max(1.0));
    let height = (desired_height * scale).min((work.height - margin * 2.0).max(1.0));
    let cx = anchor.x + anchor.width / 2.0;
    let cy = anchor.y + anchor.height / 2.0;
    let distances = [
        (anchor.y - work.y).abs(),
        (work.bottom() - anchor.bottom()).abs(),
        (anchor.x - work.x).abs(),
        (work.right() - anchor.right()).abs(),
    ];
    let edge = distances
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .unwrap()
        .0;
    let (x, y) = match edge {
        0 => (cx - width / 2.0, anchor.bottom() + margin),
        1 => (cx - width / 2.0, anchor.y - margin - height),
        2 => (anchor.right() + margin, cy - height / 2.0),
        _ => (anchor.x - margin - width, cy - height / 2.0),
    };
    let min_x = work.x + margin.min((work.width - width).max(0.0));
    let min_y = work.y + margin.min((work.height - height).max(0.0));
    Bounds {
        x: x.clamp(min_x, (work.right() - width - margin).max(min_x)),
        y: y.clamp(min_y, (work.bottom() - height - margin).max(min_y)),
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bounds(x: f64, y: f64, width: f64, height: f64) -> Bounds {
        Bounds {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn bottom_tray_opens_above_and_stays_in_negative_coordinate_monitor() {
        let popup = place(
            bounds(-1920.0, 0.0, 1920.0, 1040.0),
            bounds(-60.0, 1040.0, 24.0, 24.0),
            1.0,
            500.0,
        );
        assert_eq!(popup.y, 532.0);
        assert_eq!(popup.x, -368.0);
    }

    #[test]
    fn top_and_side_panels_expand_towards_workspace() {
        let work = bounds(0.0, 30.0, 1920.0, 1050.0);
        assert_eq!(
            place(work, bounds(900.0, 0.0, 24.0, 24.0), 1.0, 500.0).y,
            38.0
        );
        assert_eq!(
            place(work, bounds(0.0, 500.0, 24.0, 24.0), 1.0, 500.0).x,
            32.0
        );
        assert_eq!(
            place(work, bounds(1896.0, 500.0, 24.0, 24.0), 1.0, 500.0).x,
            1528.0
        );
    }

    #[test]
    fn mixed_dpi_and_small_workspaces_bound_the_entire_popup() {
        let popup = place(
            bounds(2560.0, -900.0, 800.0, 600.0),
            bounds(3300.0, -300.0, 48.0, 48.0),
            2.0,
            700.0,
        );
        assert_eq!(popup.width, 720.0);
        assert_eq!(popup.height, 568.0);
        assert!(popup.x >= 2576.0 && popup.x + popup.width <= 3344.0);
        assert_eq!(popup.y, -884.0);
    }
}

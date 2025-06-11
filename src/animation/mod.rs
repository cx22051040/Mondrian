use std::time::Duration;

use smithay::utils::{Coordinate, Logical, Point, Rectangle, Size};

pub enum AnimationType {
    #[allow(dead_code)]
    Linear,
    EaseInOutQuad,
    OvershootBounce,
}

impl AnimationType {
    pub fn _default() -> AnimationType {
        AnimationType::Linear
    }

    pub fn get_progress(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);

        match self {
            AnimationType::Linear => t,

            AnimationType::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }

            AnimationType::OvershootBounce => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;

                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
        }
    }
}

pub enum AnimationState {
    NotStarted,
    Running,
    Completed,
}

impl AnimationState {
    pub fn new() -> Self {
        AnimationState::NotStarted
    }
}

pub struct Animation {
    from: Rectangle<i32, Logical>,
    to: Rectangle<i32, Logical>,
    elapsed: Duration,
    duration: Duration,
    animation_type: AnimationType,
    pub state: AnimationState,
}

impl Animation {
    pub fn new(
        from: Rectangle<i32, Logical>,
        to: Rectangle<i32, Logical>,
        duration: Duration,
        animation_type: AnimationType,
    ) -> Self {
        Self {
            from,
            to,
            elapsed: Duration::ZERO,
            duration,
            animation_type,
            state: AnimationState::new(),
        }
    }

    pub fn start(&mut self) -> Rectangle<i32, Logical> {
        self.elapsed = Duration::ZERO;
        self.state = AnimationState::Running;
        self.from
    }

    pub fn tick(&mut self) {
        self.elapsed += Duration::from_millis(1);
        if self.elapsed >= self.duration {
            self.state = AnimationState::Completed;
        }
    }

    pub fn current_value(&self) -> Rectangle<i32, Logical> {
        let progress = (self.elapsed.as_secs_f64() / self.duration.as_secs_f64()).clamp(0.0, 1.0);
        process_rec(
            self.from,
            self.to,
            self.animation_type.get_progress(progress),
        )
    }
}

fn process_rec(
    from: Rectangle<i32, Logical>,
    to: Rectangle<i32, Logical>,
    progress: f64,
) -> Rectangle<i32, Logical> {
    let size: Size<f64, Logical> = (
        from.size.w.to_f64() + (to.size.w - from.size.w).to_f64() * progress,
        from.size.h.to_f64() + (to.size.h - from.size.h).to_f64() * progress,
    )
        .into();

    let loc: Point<f64, Logical> = (
        from.loc.x.to_f64() + (to.loc.x - from.loc.x).to_f64() * progress,
        from.loc.y.to_f64() + (to.loc.y - from.loc.y).to_f64() * progress,
    )
        .into();

    Rectangle {
        loc: loc.to_i32_round(),
        size: size.to_i32_round(),
    }
}


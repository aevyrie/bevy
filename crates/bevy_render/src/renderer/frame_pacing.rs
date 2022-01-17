use bevy_ecs::change_detection::Mut;
use bevy_utils::tracing::info_span;
use std::time::{Duration, Instant};

/// Frame pacing and frame limiting configuration resource.
#[derive(Debug, Clone)]
pub struct FramePacing {
    /// The minimum frametime limit. When a frame is rendered before this time limit is reached, the
    /// renderer will wait until it has just enough time to render the next frame before starting to
    /// render again.
    ///
    /// __This should not be set lower than the display's refresh rate__
    ///
    /// This is used to achieve consistent frame pacing (renderer produces frames at the same rate
    /// as the monitor can display them), and reduce input (motion-to-photon) latency.
    frametime_limit: Duration,
    /// How early should we cut the predicted sleep time by, to ensure we have enough time to render
    /// our frame if it takes longer than expected?
    ///
    /// Increasing this number makes dropped frames less likely, but also increases motion-to-photon
    /// latency of user input rendered to screen. The more frametime variance your application
    /// experiences, the higher this number must be to prevent dropped frames.
    frame_pacing_safety_margin: Duration,
}
impl FramePacing {
    pub fn new(fps: f32) -> Self {
        FramePacing {
            frametime_limit: Duration::from_micros((1.0 / fps) as u64 * 1_000),
            frame_pacing_safety_margin: Duration::from_micros(500),
        }
    }
}

/// A renderer-internal resource for tracking frame time for the purposes of frame pacing and frame
/// limiting.
#[derive(Debug, Clone)]
pub struct FrameTimer {
    /// The instant this frame started
    frame_start: Instant,
    /// The duration the frame limiter has slept this frame
    frame_limiter_sleep: Duration,
    /// The instant before frames are presented to the GPU
    render_start: Instant,
}
impl Default for FrameTimer {
    fn default() -> Self {
        FrameTimer {
            frame_start: Instant::now(),
            frame_limiter_sleep: Duration::ZERO,
            render_start: Instant::now(),
        }
    }
}

/// Limits framerate by sleeping until the desired frametime has elapsed
pub fn limit_framerate(settings: &FramePacing, mut timer: Mut<FrameTimer>) {
    let span = info_span!("frame_limiter");
    let _guard = span.enter();
    let FramePacing {
        frametime_limit,
        frame_pacing_safety_margin: _,
    } = *settings;
    let function_start = Instant::now();
    let last_frametime = function_start.duration_since(timer.render_start);
    // Need to cap this; the subsequent subtraction will panic if the result is negative
    let last_frametime_capped = frametime_limit.min(last_frametime);
    let sleep_needed = frametime_limit - last_frametime_capped;
    spin_sleep::sleep(sleep_needed); // The spin_sleep crate provides precise sleep times
    print!("lol");
    timer.render_start = Instant::now();
    timer.frame_limiter_sleep = timer.render_start.duration_since(function_start);
}

/// Provides frame pacing by sleeping after rendering is finished, until the next frame can start
/// rendering. The sleep time is estimated based on how long rendering took in the last frame.
fn pace_framerate(settings: &FramePacing, mut timer: Mut<FrameTimer>) {
    let FramePacing {
        frametime_limit,
        frame_pacing_safety_margin,
    } = *settings;
    let render_end = Instant::now();
    let last_frametime = render_end.duration_since(timer.frame_start);
    let last_actual_frametime = last_frametime - timer.frame_limiter_sleep;
    let estimated_frametime_needed = last_actual_frametime + frame_pacing_safety_margin;
    let estimated_frametime_needed_capped = frametime_limit.min(estimated_frametime_needed);
    let estimated_sleep_needed = frametime_limit - estimated_frametime_needed_capped;
    spin_sleep::sleep(estimated_sleep_needed);
    timer.frame_start = Instant::now();
}

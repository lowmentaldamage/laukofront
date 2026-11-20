//! Frame-time recorder for `--perf-run SECONDS`: records every frame after a warm-up, then
//! prints a summary and exits. Used to fill the frame-time tables in `docs/PERFORMANCE.md`.

use bevy::prelude::*;

/// Frames in the first `WARMUP_SECS` are not recorded (shader compilation, first uploads).
pub const WARMUP_SECS: f32 = 2.0;

#[derive(Resource, Debug)]
pub struct PerfRun {
    pub seconds: f32,
    elapsed: f32,
    frames_ms: Vec<f32>,
}

impl PerfRun {
    pub fn new(seconds: f32) -> Self {
        Self {
            seconds,
            elapsed: 0.0,
            frames_ms: Vec::with_capacity((seconds * 240.0) as usize),
        }
    }
}

/// Summary of recorded frame times.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSummary {
    pub frames: usize,
    pub avg_ms: f32,
    pub p99_ms: f32,
    pub max_ms: f32,
    pub over_33ms: usize,
}

pub fn summarize(frames_ms: &[f32]) -> Option<FrameSummary> {
    if frames_ms.is_empty() {
        return None;
    }
    let mut sorted = frames_ms.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let n = sorted.len();
    let p99_index = ((n as f32 * 0.99).ceil() as usize).clamp(1, n) - 1;
    Some(FrameSummary {
        frames: n,
        avg_ms: sorted.iter().sum::<f32>() / n as f32,
        p99_ms: sorted[p99_index],
        max_ms: sorted[n - 1],
        over_33ms: sorted.iter().filter(|&&f| f > 33.0).count(),
    })
}

pub fn record_frames(
    mut run: ResMut<PerfRun>,
    time: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    run.elapsed += dt;
    if run.elapsed < WARMUP_SECS {
        return;
    }
    run.frames_ms.push(dt * 1000.0);
    if dt * 1000.0 > 33.0 {
        println!("perf-run: slow frame {:.1} ms at {:.1} s", dt * 1000.0, run.elapsed);
    }
    if run.elapsed >= WARMUP_SECS + run.seconds {
        match summarize(&run.frames_ms) {
            Some(s) => println!(
                "perf-run: {} frames over {:.0} s | avg {:.2} ms ({:.0} FPS) | p99 {:.2} ms | max {:.2} ms | frames > 33 ms: {}",
                s.frames,
                run.seconds,
                s.avg_ms,
                1000.0 / s.avg_ms,
                s.p99_ms,
                s.max_ms,
                s.over_33ms
            ),
            None => println!("perf-run: no frames recorded"),
        }
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_numbers() {
        let mut frames = vec![10.0; 99];
        frames.push(50.0);
        let s = summarize(&frames).unwrap();
        assert_eq!(s.frames, 100);
        assert_eq!(s.max_ms, 50.0);
        assert_eq!(s.p99_ms, 10.0);
        assert_eq!(s.over_33ms, 1);
        assert!((s.avg_ms - 10.4).abs() < 1e-4);
    }

    #[test]
    fn empty_is_none() {
        assert!(summarize(&[]).is_none());
    }
}

/// `--screenshot PATH`: saves a screenshot after `at_secs`, then exits one second later.
#[derive(Resource, Debug)]
pub struct ScreenshotRun {
    pub path: String,
    pub at_secs: f32,
    elapsed: f32,
    taken: bool,
}

impl ScreenshotRun {
    pub fn new(path: String, at_secs: f32) -> Self {
        Self {
            path,
            at_secs,
            elapsed: 0.0,
            taken: false,
        }
    }
}

pub fn take_screenshot(
    mut commands: Commands,
    mut run: ResMut<ScreenshotRun>,
    time: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    run.elapsed += time.delta_secs();
    if !run.taken && run.elapsed >= run.at_secs {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(run.path.clone()));
        run.taken = true;
    }
    if run.taken && run.elapsed >= run.at_secs + 1.0 {
        exit.write(AppExit::Success);
    }
}

//! Sound effects (port milestone P13), synthesised in code at startup: no sound files, nothing
//! downloaded (D44). Each effect is a short mono buffer played through Bevy's audio with a custom
//! `Decodable` source; the master volume is the "Sound volume" setting.

use std::num::NonZero;
use std::sync::Arc;
use std::time::Duration;

use bevy::audio::{
    AddAudioSource, ChannelCount, Decodable, PlaybackSettings, Sample, SampleRate, Source,
};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

const RATE: u32 = 44_100;

/// The effects the game plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sfx {
    Click,
    Attack,
    Build,
    Alliance,
    Warning,
    Launch,
    Explosion,
    Victory,
    Defeat,
}

impl Sfx {
    pub const ALL: [Sfx; 9] = [
        Sfx::Click,
        Sfx::Attack,
        Sfx::Build,
        Sfx::Alliance,
        Sfx::Warning,
        Sfx::Launch,
        Sfx::Explosion,
        Sfx::Victory,
        Sfx::Defeat,
    ];
}

/// Effects to play this frame (filled by the game systems).
#[derive(Resource, Default)]
pub struct SoundQueue(pub Vec<Sfx>);

/// A synthesised sound.
#[derive(Asset, TypePath, Clone)]
pub struct Synth {
    samples: Arc<[f32]>,
}

pub struct SynthDecoder {
    samples: Arc<[f32]>,
    at: usize,
}

impl Iterator for SynthDecoder {
    type Item = Sample;
    fn next(&mut self) -> Option<Sample> {
        let v = *self.samples.get(self.at)?;
        self.at += 1;
        Some(v as Sample)
    }
}

impl Source for SynthDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        NonZero::new(1).unwrap_or(NonZero::<u16>::MIN)
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(RATE).unwrap_or(NonZero::<u32>::MIN)
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.samples.len() as f64 / RATE as f64,
        ))
    }
}

impl Decodable for Synth {
    type Decoder = SynthDecoder;
    fn decoder(&self) -> SynthDecoder {
        SynthDecoder {
            samples: self.samples.clone(),
            at: 0,
        }
    }
}

/// Deterministic white noise in -1..1.
struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
}

#[allow(clippy::too_many_arguments)]
fn tone(
    out: &mut Vec<f32>,
    start: f32,
    len: f32,
    f0: f32,
    f1: f32,
    amp: f32,
    decay: f32,
    square: bool,
) {
    let n0 = (start * RATE as f32) as usize;
    let n = (len * RATE as f32) as usize;
    if out.len() < n0 + n {
        out.resize(n0 + n, 0.0);
    }
    let mut phase = 0f32;
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let f = f0 + (f1 - f0) * (i as f32 / n as f32);
        phase += f / RATE as f32;
        let s = (phase * std::f32::consts::TAU).sin();
        let s = if square { s.signum() * 0.6 } else { s };
        // 5 ms attack, exponential decay.
        let env = (t / 0.005).min(1.0) * (-t * decay).exp();
        out[n0 + i] += s * amp * env;
    }
}

fn noise(out: &mut Vec<f32>, start: f32, len: f32, amp: f32, decay: f32, smooth: f32, seed: u32) {
    let n0 = (start * RATE as f32) as usize;
    let n = (len * RATE as f32) as usize;
    if out.len() < n0 + n {
        out.resize(n0 + n, 0.0);
    }
    let mut rng = Noise(seed);
    let mut lp = 0f32;
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        lp += (rng.next() - lp) * smooth;
        let env = (t / 0.01).min(1.0) * (-t * decay).exp();
        out[n0 + i] += lp * amp * env;
    }
}

/// The samples of an effect (mono, 44.1 kHz, within -1..1). Pure, for tests.
pub fn synth(s: Sfx) -> Vec<f32> {
    let mut o = Vec::new();
    match s {
        Sfx::Click => tone(&mut o, 0.0, 0.04, 1400.0, 1100.0, 0.25, 60.0, false),
        Sfx::Attack => {
            tone(&mut o, 0.0, 0.18, 140.0, 70.0, 0.55, 18.0, false);
            noise(&mut o, 0.0, 0.12, 0.25, 30.0, 0.3, 7);
        }
        Sfx::Build => {
            tone(&mut o, 0.0, 0.09, 660.0, 660.0, 0.3, 25.0, false);
            tone(&mut o, 0.08, 0.14, 880.0, 880.0, 0.3, 20.0, false);
        }
        Sfx::Alliance => {
            for (i, f) in [523.25, 659.25, 783.99, 1046.5].iter().enumerate() {
                tone(&mut o, i as f32 * 0.09, 0.5, *f, *f, 0.22, 6.0, false);
            }
        }
        Sfx::Warning => {
            tone(&mut o, 0.0, 0.22, 233.0, 233.0, 0.28, 4.0, true);
            tone(&mut o, 0.25, 0.3, 196.0, 196.0, 0.28, 4.0, true);
        }
        Sfx::Launch => {
            noise(&mut o, 0.0, 0.9, 0.5, 2.5, 0.08, 11);
            tone(&mut o, 0.0, 0.9, 180.0, 900.0, 0.15, 2.0, false);
        }
        Sfx::Explosion => {
            noise(&mut o, 0.0, 1.4, 0.9, 3.0, 0.05, 3);
            tone(&mut o, 0.0, 0.8, 60.0, 35.0, 0.6, 4.0, false);
        }
        Sfx::Victory => {
            for (i, f) in [523.25, 659.25, 783.99, 1046.5, 1318.5].iter().enumerate() {
                tone(&mut o, i as f32 * 0.14, 0.6, *f, *f, 0.25, 3.5, false);
            }
        }
        Sfx::Defeat => {
            for (i, f) in [392.0, 349.23, 311.13, 261.63].iter().enumerate() {
                tone(&mut o, i as f32 * 0.22, 0.6, *f, *f, 0.25, 3.0, false);
            }
        }
    }
    for v in o.iter_mut() {
        *v = v.clamp(-1.0, 1.0);
    }
    o
}

#[derive(Resource)]
struct SoundBank(Vec<(Sfx, Handle<Synth>)>);

/// When each effect last played (real seconds), to avoid stacking the same sound.
#[derive(Resource, Default)]
struct LastPlayed(Vec<(Sfx, f64)>);

pub struct AudioFxPlugin;

impl Plugin for AudioFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Synth>()
            .init_resource::<SoundQueue>()
            .init_resource::<LastPlayed>()
            .add_systems(Startup, make_sounds)
            .add_systems(Update, play_queue)
            // Every button click.
            .add_observer(|_: On<Activate>, mut q: ResMut<SoundQueue>| q.0.push(Sfx::Click));
    }
}

fn make_sounds(mut commands: Commands, mut assets: ResMut<Assets<Synth>>) {
    let bank = Sfx::ALL
        .iter()
        .map(|&s| {
            (
                s,
                assets.add(Synth {
                    samples: synth(s).into(),
                }),
            )
        })
        .collect();
    commands.insert_resource(SoundBank(bank));
}

fn play_queue(
    mut commands: Commands,
    mut queue: ResMut<SoundQueue>,
    bank: Option<Res<SoundBank>>,
    mut last: ResMut<LastPlayed>,
    time: Res<Time<Real>>,
) {
    let Some(bank) = bank else {
        queue.0.clear();
        return;
    };
    let now = time.elapsed_secs_f64();
    let mut played: Vec<Sfx> = Vec::new();
    for s in queue.0.drain(..) {
        if played.contains(&s) {
            continue;
        }
        let gap = if s == Sfx::Click { 0.05 } else { 0.25 };
        if last.0.iter().any(|&(k, t)| k == s && now - t < gap) {
            continue;
        }
        played.push(s);
        last.0.retain(|&(k, _)| k != s);
        last.0.push((s, now));
        if let Some((_, h)) = bank.0.iter().find(|(k, _)| *k == s) {
            commands.spawn((AudioPlayer::<Synth>(h.clone()), PlaybackSettings::DESPAWN));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_is_short_audible_and_in_range() {
        for s in Sfx::ALL {
            let v = synth(s);
            let secs = v.len() as f32 / RATE as f32;
            assert!(secs > 0.02 && secs < 2.0, "{s:?}: {secs} s");
            let peak = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.1 && peak <= 1.0, "{s:?}: peak {peak}");
        }
    }
}

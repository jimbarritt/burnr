use std::ops::Range;

use rand::RngExt;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

use super::particle::{Particle, random_glyph};

#[derive(Default)]
pub struct ParticleSystem {
    particles: Vec<Particle>,
}

impl ParticleSystem {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.particles.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.particles.is_empty()
    }

    /// The background smoulder, spawned at `embers_per_second` (the caller
    /// picks the rate — fuller while the session is active, a gentle flicker
    /// once it goes idle). Fractional expected counts are resolved
    /// probabilistically so low rates still flicker rather than stopping.
    pub fn spawn_ambient(&mut self, width: u16, embers_per_second: f32, dt: f32) {
        let expected = (embers_per_second * dt).max(0.0);
        let mut count = expected.floor() as usize;
        if rand::rng().random_range(0.0..1.0) < expected.fract() {
            count += 1;
        }
        self.spawn_particles(width, count, 4.0..9.0);
    }

    /// A burst of embers sized by tokens burned in one turn. Real sessions
    /// range from hundreds of tokens to hundreds of thousands (cache reads
    /// dominate), so intensity is compressed with a log scale rather than
    /// scaled linearly — otherwise a single big cache-read turn would swamp
    /// the screen while everything else looked identical.
    pub fn burst(&mut self, width: u16, tokens: u64) {
        let intensity = intensity_for_tokens(tokens);
        let count = 3 + (intensity * 60.0).round() as usize;
        let vy_min = 4.0 + intensity * 4.0;
        let vy_max = 9.0 + intensity * 10.0;
        self.spawn_particles(width, count, vy_min..vy_max);
    }

    fn spawn_particles(&mut self, width: u16, count: usize, vy_range: Range<f32>) {
        if width == 0 {
            return;
        }
        let mut rng = rand::rng();
        for _ in 0..count {
            self.particles.push(Particle {
                x: rng.random_range(0..width) as f32,
                y: 0.0,
                vx: rng.random_range(-0.4..0.4),
                vy: rng.random_range(vy_range.clone()),
                glyph: random_glyph(&mut rng),
                age: 0.0,
                lifetime: rng.random_range(1.2..2.5),
            });
        }
    }

    pub fn update(&mut self, dt: f32) {
        for particle in &mut self.particles {
            particle.update(dt);
        }
        self.particles.retain(Particle::is_alive);
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        for particle in &self.particles {
            let Some((x, y)) = self.screen_position(particle, area) else {
                continue;
            };
            if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                cell.set_char(particle.glyph);
                cell.set_fg(particle.colour());
            }
        }
    }

    fn screen_position(&self, particle: &Particle, area: Rect) -> Option<(u16, u16)> {
        let col = particle.x.round();
        let row_from_bottom = particle.y.round();
        if col < 0.0 || col >= area.width as f32 {
            return None;
        }
        if row_from_bottom < 0.0 || row_from_bottom >= area.height as f32 {
            return None;
        }
        let x = area.x + col as u16;
        let y = area.y + area.height - 1 - row_from_bottom as u16;
        Some((x, y))
    }
}

/// Tokens below this read as intensity 0 (a flicker); tokens above
/// `TOKENS_CEILING` saturate at intensity 1 (a full blaze). Calibrated
/// against ~5,000 real turns across many sessions: totals run ~20k–285k
/// with a median around 75k, because cache reads dominate.
const TOKENS_FLOOR: f32 = 10_000.0;
const TOKENS_CEILING: f32 = 300_000.0;

/// Maps a token count to a 0..1 intensity, log-scaled across the observed
/// real-world range. A plain `ln(t)/k` curve compressed all real turns into
/// the top fifth of the scale (everything looked like a maximal burst);
/// anchoring the scale between an observed floor and ceiling spreads the
/// median turn to ~0.6 and keeps small and huge turns visually distinct.
fn intensity_for_tokens(tokens: u64) -> f32 {
    let t = (tokens as f32).max(1.0);
    ((t.ln() - TOKENS_FLOOR.ln()) / (TOKENS_CEILING.ln() - TOKENS_FLOOR.ln())).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_ambient_spawns_whole_expected_count() {
        let mut system = ParticleSystem::new();
        // 30 embers/sec over a 0.1s tick = exactly 3, no fractional part.
        system.spawn_ambient(40, 30.0, 0.1);
        assert_eq!(system.len(), 3);
    }

    #[test]
    fn spawn_ambient_low_rate_spawns_at_most_one_per_tick() {
        let mut system = ParticleSystem::new();
        // 3 embers/sec over a 33ms tick ≈ 0.1 expected — probabilistically
        // zero or one, never more.
        system.spawn_ambient(40, 3.0, 0.033);
        assert!(system.len() <= 1);
    }

    #[test]
    fn spawn_ambient_zero_rate_spawns_nothing() {
        let mut system = ParticleSystem::new();
        system.spawn_ambient(40, 0.0, 0.033);
        assert!(system.is_empty());
    }

    #[test]
    fn spawn_ambient_with_zero_width_adds_nothing() {
        let mut system = ParticleSystem::new();
        system.spawn_ambient(0, 30.0, 1.0);
        assert!(system.is_empty());
    }

    #[test]
    fn update_removes_particles_past_their_lifetime() {
        let mut system = ParticleSystem::new();
        system.spawn_ambient(40, 30.0, 0.1);
        // All spawned lifetimes are well under 100 seconds.
        system.update(100.0);
        assert!(system.is_empty());
    }

    #[test]
    fn bigger_bursts_spawn_more_particles() {
        let mut small = ParticleSystem::new();
        small.burst(40, 100);
        let mut large = ParticleSystem::new();
        large.burst(40, 150_000);
        assert!(large.len() > small.len());
    }

    #[test]
    fn burst_with_zero_width_adds_nothing() {
        let mut system = ParticleSystem::new();
        system.burst(0, 50_000);
        assert!(system.is_empty());
    }

    #[test]
    fn intensity_is_clamped_and_monotonic() {
        assert_eq!(intensity_for_tokens(0), 0.0);
        assert!(intensity_for_tokens(20_000) < intensity_for_tokens(100_000));
        assert!(intensity_for_tokens(10_000_000_000) <= 1.0);
    }

    #[test]
    fn real_world_turn_sizes_spread_across_the_scale() {
        // Observed distribution: p5 ~26k, median ~75k, p99 ~246k. These
        // should land visibly apart, not all crushed against 1.0.
        let small = intensity_for_tokens(26_000);
        let median = intensity_for_tokens(75_000);
        let huge = intensity_for_tokens(246_000);
        assert!(small < 0.4, "small turn should smoulder, got {small}");
        assert!(
            (0.4..0.8).contains(&median),
            "median turn should sit mid-scale, got {median}"
        );
        assert!(huge > 0.8, "huge turn should blaze, got {huge}");
    }

    #[test]
    fn intensity_saturates_at_floor_and_ceiling() {
        assert_eq!(intensity_for_tokens(500), 0.0);
        assert_eq!(intensity_for_tokens(1_000_000), 1.0);
    }
}

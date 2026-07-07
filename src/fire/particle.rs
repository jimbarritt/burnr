use rand::RngExt;
use ratatui::style::Color;

const GLYPHS: &[char] = &[
    'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ', 'ﾀ', 'ﾁ', 'ﾂ', 'ﾃ',
    'ﾄ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾉ', 'ﾊ', 'ﾋ', 'ﾌ', 'ﾍ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ', 'ﾔ', 'ﾕ', 'ﾖ',
    'ﾗ', 'ﾘ', 'ﾙ', 'ﾚ', 'ﾛ', 'ﾜ', 'ﾝ', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
];

pub fn random_glyph(rng: &mut impl rand::Rng) -> char {
    GLYPHS[rng.random_range(0..GLYPHS.len())]
}

/// A single rising ember. Coordinates are area-local: `x` is columns from
/// the left edge, `y` is rows *above the bottom* of the area (so positive
/// `vy` means rising).
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub glyph: char,
    pub age: f32,
    pub lifetime: f32,
}

impl Particle {
    pub fn is_alive(&self) -> bool {
        self.age < self.lifetime
    }

    pub fn update(&mut self, dt: f32) {
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        self.age += dt;
    }

    /// Fraction of the particle's life elapsed, clamped to [0, 1].
    fn age_fraction(&self) -> f32 {
        (self.age / self.lifetime).clamp(0.0, 1.0)
    }

    /// Colour ramps white -> yellow -> orange -> dim red as the particle ages.
    pub fn colour(&self) -> Color {
        let t = self.age_fraction();
        let stops: [(f32, (u8, u8, u8)); 4] = [
            (0.0, (255, 255, 255)),
            (0.3, (255, 255, 120)),
            (0.65, (255, 140, 30)),
            (1.0, (120, 20, 10)),
        ];
        let mut lo = stops[0];
        let mut hi = stops[stops.len() - 1];
        for window in stops.windows(2) {
            let (a, b) = (window[0], window[1]);
            if t >= a.0 && t <= b.0 {
                lo = a;
                hi = b;
                break;
            }
        }
        let span = (hi.0 - lo.0).max(f32::EPSILON);
        let local_t = ((t - lo.0) / span).clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * local_t).round() as u8;
        Color::Rgb(
            lerp(lo.1.0, hi.1.0),
            lerp(lo.1.1, hi.1.1),
            lerp(lo.1.2, hi.1.2),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_alive_before_lifetime_elapses() {
        let particle = Particle {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 1.0,
            glyph: '0',
            age: 0.0,
            lifetime: 1.0,
        };
        assert!(particle.is_alive());
    }

    #[test]
    fn dies_once_age_reaches_lifetime() {
        let mut particle = Particle {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 1.0,
            glyph: '0',
            age: 0.0,
            lifetime: 1.0,
        };
        particle.update(1.0);
        assert!(!particle.is_alive());
    }

    #[test]
    fn rises_and_drifts_over_time() {
        let mut particle = Particle {
            x: 5.0,
            y: 0.0,
            vx: 1.0,
            vy: 2.0,
            glyph: '0',
            age: 0.0,
            lifetime: 10.0,
        };
        particle.update(0.5);
        assert_eq!(particle.x, 5.5);
        assert_eq!(particle.y, 1.0);
    }

    #[test]
    fn colour_starts_white_and_ends_dim_red() {
        let fresh = Particle {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            glyph: '0',
            age: 0.0,
            lifetime: 1.0,
        };
        assert_eq!(fresh.colour(), Color::Rgb(255, 255, 255));

        let dying = Particle { age: 1.0, ..fresh };
        assert_eq!(dying.colour(), Color::Rgb(120, 20, 10));
    }
}

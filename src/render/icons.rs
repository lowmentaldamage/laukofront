//! Our own 2D icons for buildings and ships (not OpenFront's art, D20), drawn into small textures
//! at startup from simple shapes: white glyphs with a dark outline, and a round badge that is
//! tinted with the owner's colour behind building glyphs. Pure pixel code, so it is testable.

use crate::engine::units::UnitKind;

/// Icon texture size in pixels.
pub const ICON_PX: u32 = 48;
/// Supersamples per axis for smooth edges.
const SS: u32 = 4;
/// Outline width in pixels.
const OUTLINE_PX: f32 = 2.2;
/// Outline colour (stays dark when tinted).
const DARK: [u8; 3] = [16, 18, 24];

/// Which picture an icon shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Glyph {
    Badge,
    City,
    Port,
    Factory,
    DefensePost,
    MissileSilo,
    SamLauncher,
    Warship,
    TransportShip,
    TradeShip,
}

impl Glyph {
    pub const ALL: [Glyph; 10] = [
        Glyph::Badge,
        Glyph::City,
        Glyph::Port,
        Glyph::Factory,
        Glyph::DefensePost,
        Glyph::MissileSilo,
        Glyph::SamLauncher,
        Glyph::Warship,
        Glyph::TransportShip,
        Glyph::TradeShip,
    ];

    /// The glyph of a unit kind (nukes are drawn as effects, not icons).
    pub fn of(kind: UnitKind) -> Option<Glyph> {
        Some(match kind {
            UnitKind::City => Glyph::City,
            UnitKind::Port => Glyph::Port,
            UnitKind::Factory => Glyph::Factory,
            UnitKind::DefensePost => Glyph::DefensePost,
            UnitKind::MissileSilo => Glyph::MissileSilo,
            UnitKind::SamLauncher => Glyph::SamLauncher,
            UnitKind::Warship => Glyph::Warship,
            UnitKind::TransportShip => Glyph::TransportShip,
            UnitKind::TradeShip => Glyph::TradeShip,
            _ => return None,
        })
    }

    /// Is a point (u, v in -1..1, v up) inside the shape?
    fn inside(self, u: f32, v: f32) -> bool {
        let rect = |x0: f32, y0: f32, x1: f32, y1: f32| u >= x0 && u <= x1 && v >= y0 && v <= y1;
        let circle = |cx: f32, cy: f32, r: f32| (u - cx).powi(2) + (v - cy).powi(2) <= r * r;
        let tri = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
            let s =
                |p: (f32, f32), q: (f32, f32)| (u - q.0) * (p.1 - q.1) - (p.0 - q.0) * (v - q.1);
            let (d1, d2, d3) = (s(a, b), s(b, c), s(c, a));
            let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
            let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
            !(neg && pos)
        };
        let quad = |a, b, c, d| tri(a, b, c) || tri(a, c, d);
        match self {
            Glyph::Badge => circle(0.0, 0.0, 0.95),
            Glyph::City => {
                rect(-0.8, -0.72, -0.3, 0.15)
                    || rect(-0.32, -0.72, 0.18, 0.7)
                    || rect(0.16, -0.72, 0.76, 0.0)
                    || rect(-0.86, -0.82, 0.86, -0.66)
            }
            Glyph::Port => {
                let ring = circle(0.0, 0.58, 0.2) && !circle(0.0, 0.58, 0.09);
                let hook = circle(0.0, -0.05, 0.62) && !circle(0.0, -0.05, 0.48) && v < -0.05;
                ring || rect(-0.08, -0.62, 0.08, 0.4)
                    || rect(-0.42, 0.14, 0.42, 0.28)
                    || hook
                    || tri((-0.75, 0.05), (-0.42, 0.05), (-0.58, -0.25))
                    || tri((0.75, 0.05), (0.42, 0.05), (0.58, -0.25))
            }
            Glyph::Factory => {
                rect(-0.82, -0.72, 0.82, 0.02)
                    || rect(-0.78, 0.0, -0.56, 0.82)
                    || tri((-0.52, 0.0), (-0.07, 0.0), (-0.07, 0.42))
                    || tri((-0.07, 0.0), (0.38, 0.0), (0.38, 0.42))
                    || tri((0.38, 0.0), (0.82, 0.0), (0.82, 0.42))
            }
            Glyph::DefensePost => {
                rect(-0.62, -0.08, 0.62, 0.72) || tri((-0.62, -0.08), (0.62, -0.08), (0.0, -0.88))
            }
            Glyph::MissileSilo => {
                rect(-0.17, -0.55, 0.17, 0.42)
                    || tri((-0.17, 0.42), (0.17, 0.42), (0.0, 0.86))
                    || tri((-0.17, -0.12), (-0.17, -0.62), (-0.48, -0.66))
                    || tri((0.17, -0.12), (0.17, -0.62), (0.48, -0.66))
                    || rect(-0.72, -0.86, 0.72, -0.72)
            }
            Glyph::SamLauncher => {
                let dish = circle(-0.05, 0.2, 0.62) && !circle(0.25, 0.48, 0.55);
                dish || circle(0.38, 0.52, 0.11)
                    || rect(-0.08, -0.55, 0.08, 0.05)
                    || tri((-0.5, -0.85), (0.5, -0.85), (0.0, -0.45))
            }
            Glyph::Warship => {
                quad((-0.92, -0.08), (0.92, -0.08), (0.62, -0.5), (-0.72, -0.5))
                    || rect(-0.38, -0.1, 0.22, 0.26)
                    || rect(0.28, -0.1, 0.52, 0.12)
                    || rect(0.46, 0.02, 0.9, 0.08)
                    || rect(-0.14, 0.24, -0.04, 0.62)
            }
            Glyph::TransportShip => {
                quad((-0.78, -0.2), (0.78, -0.2), (0.52, -0.56), (-0.52, -0.56))
                    || rect(-0.05, -0.2, 0.05, 0.78)
                    || tri((0.1, 0.72), (0.1, -0.08), (0.62, -0.08))
                    || tri((-0.1, 0.6), (-0.1, -0.08), (-0.48, -0.08))
            }
            Glyph::TradeShip => {
                quad((-0.82, -0.2), (0.82, -0.2), (0.56, -0.56), (-0.56, -0.56))
                    || rect(-0.56, -0.18, -0.12, 0.2)
                    || rect(-0.04, -0.18, 0.4, 0.2)
                    || rect(-0.32, 0.22, 0.12, 0.54)
            }
        }
    }
}

/// RGBA pixels (`ICON_PX` square, row 0 at the top) of a glyph: white shape, dark outline,
/// transparent elsewhere.
pub fn render(glyph: Glyph) -> Vec<u8> {
    let n = ICON_PX as usize;
    // Coverage per pixel from supersampling.
    let mut cover = vec![0f32; n * n];
    for py in 0..n {
        for px in 0..n {
            let mut hits = 0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let fx = px as f32 + (sx as f32 + 0.5) / SS as f32;
                    let fy = py as f32 + (sy as f32 + 0.5) / SS as f32;
                    let u = fx / n as f32 * 2.0 - 1.0;
                    let v = 1.0 - fy / n as f32 * 2.0;
                    // Leave room for the outline at the edges.
                    if glyph.inside(u / 0.9, v / 0.9) {
                        hits += 1;
                    }
                }
            }
            cover[py * n + px] = hits as f32 / (SS * SS) as f32;
        }
    }
    // Outline: the strongest coverage within OUTLINE_PX.
    let r = OUTLINE_PX.ceil() as i64;
    let mut out = vec![0u8; n * n * 4];
    for py in 0..n as i64 {
        for px in 0..n as i64 {
            let c = cover[(py as usize) * n + px as usize];
            let mut o = 0f32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    if d > OUTLINE_PX + 0.5 {
                        continue;
                    }
                    let (x, y) = (px + dx, py + dy);
                    if x < 0 || y < 0 || x >= n as i64 || y >= n as i64 {
                        continue;
                    }
                    let fade = (OUTLINE_PX + 0.5 - d).clamp(0.0, 1.0);
                    o = o.max(cover[(y as usize) * n + x as usize] * fade);
                }
            }
            let a = c.max(o);
            let i = ((py as usize) * n + px as usize) * 4;
            if a <= 0.0 {
                continue;
            }
            let w = c / a;
            for k in 0..3 {
                out[i + k] = (DARK[k] as f32 * (1.0 - w) + 255.0 * w).round() as u8;
            }
            out[i + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_has_a_shape_and_an_outline() {
        for g in Glyph::ALL {
            let px = render(g);
            assert_eq!(px.len(), (ICON_PX * ICON_PX * 4) as usize);
            let white = px.chunks(4).filter(|p| p[3] == 255 && p[0] > 240).count();
            let dark = px.chunks(4).filter(|p| p[3] > 128 && p[0] < 60).count();
            assert!(white > 120, "{g:?}: {white} white pixels");
            assert!(dark > 60, "{g:?}: {dark} outline pixels");
            // Corners stay transparent (shapes fit inside the icon).
            assert_eq!(px[3], 0, "{g:?}: top-left corner");
        }
    }

    #[test]
    fn glyphs_differ() {
        let all: Vec<Vec<u8>> = Glyph::ALL.iter().map(|&g| render(g)).collect();
        for i in 0..all.len() {
            for j in i + 1..all.len() {
                assert_ne!(
                    all[i],
                    all[j],
                    "{:?} and {:?} look the same",
                    Glyph::ALL[i],
                    Glyph::ALL[j]
                );
            }
        }
    }

    #[test]
    fn every_building_and_ship_has_a_glyph() {
        for k in [
            UnitKind::City,
            UnitKind::Port,
            UnitKind::Factory,
            UnitKind::DefensePost,
            UnitKind::MissileSilo,
            UnitKind::SamLauncher,
            UnitKind::Warship,
            UnitKind::TransportShip,
            UnitKind::TradeShip,
        ] {
            assert!(Glyph::of(k).is_some(), "{k:?}");
        }
        assert_eq!(Glyph::of(UnitKind::AtomBomb), None);
    }
}

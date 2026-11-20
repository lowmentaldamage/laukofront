//! Map previews while choosing something in the build bar: the blast radius (inner and outer)
//! under the cursor for missiles, enemy SAM ranges that could stop it, and a ring around your
//! selected warship.

use bevy::prelude::*;

use crate::client::{ClientSettings, GameState};
use crate::engine::units::UnitKind;
use crate::engine::weapons::{blast_radius, sam_range};
use crate::engine::HUMAN;
use crate::input::Hovered;
use crate::render::tile_center;

pub struct PreviewPlugin;

impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_preview);
    }
}

fn draw_preview(
    game: Res<GameState>,
    settings: Res<ClientSettings>,
    hovered: Res<Hovered>,
    mut gizmos: Gizmos,
) {
    let g = &game.0;
    let map = g.map();
    if let Some(ship) = settings
        .selected_ship
        .and_then(|s| g.unit(s))
        .filter(|u| u.is_active())
    {
        gizmos.circle_2d(
            tile_center(map, ship.tile()),
            6.0,
            Color::srgba(1.0, 0.85, 0.3, 0.9),
        );
    }
    let Some(kind) = settings.build else { return };
    if !kind.is_nuke() {
        return;
    }
    let Some(t) = hovered.0 else { return };
    let c = tile_center(map, t);
    let (inner, outer) = if kind == UnitKind::Mirv {
        let w = &g.config().weapons;
        (w.warhead_outer, w.mirv_spread)
    } else {
        blast_radius(g, kind)
    };
    gizmos.circle_2d(c, inner as f32, Color::srgba(1.0, 0.3, 0.2, 0.9));
    gizmos.circle_2d(c, outer as f32, Color::srgba(1.0, 0.6, 0.2, 0.6));
    if kind != UnitKind::Mirv {
        for sam in g
            .units()
            .iter()
            .filter(|u| u.is_active() && u.kind() == UnitKind::SamLauncher && u.owner() != HUMAN)
        {
            gizmos.circle_2d(
                tile_center(map, sam.tile()),
                sam_range(g, sam.level()) as f32,
                Color::srgba(0.4, 0.9, 1.0, 0.35),
            );
        }
    }
}

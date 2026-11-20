// A pentacle shown in the world from the Dictionary of Spirits' "Show in the world" button, as
// Modonomicon's multiblock preview: until it is placed it follows the block under the crosshair
// (where the golden bowl goes), turned the way the player looks; a right-click on a block places it
// there. What is still missing is drawn: a glyph as a mark in its chalk's colour on the floor, any
// other block as a box. It goes when the circle is complete, or when the button is pressed again.
// With a ritual satchel in hand, a click on the placed preview asks the server to build from the
// player's inventory: the clicked place (the first satchel) or the whole circle (the second),
// as Occultism's ritual satchels build Modonomicon's preview.

use crate::game::lock;
use crate::ritual;
use mod_api::*;
use std::sync::Mutex;

pub const SATCHEL_T1: &str = "occultism:ritual_satchel_t1";
pub const SATCHEL_T2: &str = "occultism:ritual_satchel_t2";
pub const CHANNEL: &str = "occultism:satchel";

/// A glyph's colour, by its chalk's colour name (rites.rs's glyph colours, brightened to show).
fn glyph_colour(block: &str) -> Option<u32> {
    let colour = block.strip_prefix("occultism:chalk_glyph_")?;
    Some(match colour {
        "white" => 0xF0F0F0,
        "orange" => 0xF9801D,
        "magenta" => 0xC74EBD,
        "light_blue" => 0x3AB3DA,
        "yellow" => 0xFED83D,
        "lime" => 0x80C71F,
        "pink" => 0xF38BAA,
        "gray" => 0x474F52,
        "light_gray" => 0x9D9D97,
        "cyan" => 0x169C9C,
        "purple" => 0x8932B8,
        "blue" => 0x3C44AA,
        "brown" => 0x835432,
        "green" => 0x5E7C16,
        "red" => 0xB02E26,
        _ => 0x1D1D21,
    })
}

struct Shown {
    pentacle: String,
    parts: Vec<([i32; 3], Vec<String>)>,
    anchor: Option<[i32; 3]>,
    rotation: u8,
}

/// A request to build from a placed preview: the pentacle, where its bowl goes and how it is
/// turned, the place clicked, and whether the whole circle is wanted.
#[derive(Clone, Debug, PartialEq)]
pub struct Build {
    pub pentacle: String,
    pub anchor: [i32; 3],
    pub rotation: u8,
    pub target: [i32; 3],
    pub all: bool,
}

impl Build {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::json!({"pentacle": self.pentacle, "anchor": self.anchor, "rotation": self.rotation, "target": self.target, "all": self.all})
            .to_string()
            .into_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Option<Build> {
        let j: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        let pos = |v: &serde_json::Value| -> Option<[i32; 3]> {
            let a = v.as_array()?;
            Some([a.first()?.as_i64()? as i32, a.get(1)?.as_i64()? as i32, a.get(2)?.as_i64()? as i32])
        };
        Some(Build {
            pentacle: j["pentacle"].as_str()?.to_string(),
            anchor: pos(&j["anchor"])?,
            rotation: (j["rotation"].as_u64()? % 4) as u8,
            target: pos(&j["target"])?,
            all: j["all"].as_bool()?,
        })
    }
}

#[derive(Default)]
pub struct Preview {
    shown: Mutex<Option<Shown>>,
    outbox: Mutex<Vec<Vec<u8>>>,
}

/// The rotation (quarter turns) a pentacle takes facing `yaw`, as `ritual::rotate` turns it.
fn rotation_for(yaw: f32) -> u8 {
    ((yaw / 90.0 + 0.5).floor() as i32).rem_euclid(4) as u8
}

impl Preview {
    /// The book's button: show `pentacle` (its parts from the book), or stop showing it.
    pub fn toggle(&self, pentacle: &str, parts: Vec<([i32; 3], Vec<String>)>) {
        let mut shown = lock(&self.shown);
        if shown.as_ref().is_some_and(|s| s.pentacle == pentacle) {
            *shown = None;
            return;
        }
        *shown = Some(Shown { pentacle: pentacle.to_string(), parts, anchor: None, rotation: 0 });
    }

    pub fn outbox(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut *lock(&self.outbox))
    }

    /// A right-click on a block: places the preview there, or builds from it with a satchel.
    pub fn clicked(&self, host: &dyn ClientHost, click: &BlockClick<'_>) -> bool {
        let mut guard = lock(&self.shown);
        let Some(shown) = guard.as_mut() else { return false };
        let target = [click.pos[0] + click.hit.face[0], click.pos[1] + click.hit.face[1], click.pos[2] + click.hit.face[2]];
        let held = click.stack.map(|s| crate::game::id(host, s)).unwrap_or("");
        match shown.anchor {
            None => {
                shown.anchor = Some(target);
                shown.rotation = player_yaw(host).map_or(0, rotation_for);
                true
            }
            Some(anchor) if held == SATCHEL_T1 || held == SATCHEL_T2 => {
                let build = Build { pentacle: shown.pentacle.clone(), anchor, rotation: shown.rotation, target, all: held == SATCHEL_T2 };
                lock(&self.outbox).push(build.encode());
                true
            }
            Some(_) => false,
        }
    }

    pub fn draw(&self, host: &dyn ClientHost, event: &mut RenderLevelStageEvent<'_>) {
        if event.stage != RenderLevelStage::AfterLevel {
            return;
        }
        let mut guard = lock(&self.shown);
        let Some(shown) = guard.as_mut() else { return };
        let Some(level) = host.level(host.dimension()) else { return };

        let (anchor, rotation) = match shown.anchor {
            Some(a) => (a, shown.rotation),
            None => match looked_at(host, level) {
                Some(at) => (at, player_yaw(host).map_or(0, rotation_for)),
                None => return,
            },
        };
        let mut quads = Vec::new();
        let mut lines = Vec::new();
        let mut missing = 0;
        for (offset, accepts) in &shown.parts {
            let o = ritual::rotate(*offset, rotation);
            let at = [anchor[0] + o[0], anchor[1] + o[1], anchor[2] + o[2]];
            let here = crate::game::block_name(host, level.block(host, at));
            if accepts.iter().any(|a| a == here) {
                continue;
            }
            missing += 1;
            let (x, y, z) = (f64::from(at[0]), f64::from(at[1]), f64::from(at[2]));
            match accepts.first().and_then(|b| glyph_colour(b)) {
                Some(rgb) => {
                    let h = y + 0.03;
                    quads.push(WorldQuad::new([[x + 0.1, h, z + 0.1], [x + 0.1, h, z + 0.9], [x + 0.9, h, z + 0.9], [x + 0.9, h, z + 0.1]], 0xA0000000 | rgb, WorldDepth::Tested));
                }
                None => {
                    let (lo, hi) = (0.15, 0.85);
                    let c = |dx: f64, dy: f64, dz: f64| [x + dx, y + dy, z + dz];
                    for (a, b) in [
                        (c(lo, 0.0, lo), c(hi, 0.0, lo)), (c(hi, 0.0, lo), c(hi, 0.0, hi)), (c(hi, 0.0, hi), c(lo, 0.0, hi)), (c(lo, 0.0, hi), c(lo, 0.0, lo)),
                        (c(lo, 0.7, lo), c(hi, 0.7, lo)), (c(hi, 0.7, lo), c(hi, 0.7, hi)), (c(hi, 0.7, hi), c(lo, 0.7, hi)), (c(lo, 0.7, hi), c(lo, 0.7, lo)),
                        (c(lo, 0.0, lo), c(lo, 0.7, lo)), (c(hi, 0.0, lo), c(hi, 0.7, lo)), (c(hi, 0.0, hi), c(hi, 0.7, hi)), (c(lo, 0.0, hi), c(lo, 0.7, hi)),
                    ] {
                        lines.push(WorldLine::new(a, b, 0xC0E0C060, 2.0, WorldDepth::Tested));
                    }
                }
            }
        }
        if missing == 0 && shown.anchor.is_some() {
            *guard = None;
            return;
        }
        let _ = event.draw.quads(&quads);
        let _ = event.draw.lines(&lines);
    }
}

/// The place above or beside the block the player looks at, up to 32 blocks away: where the
/// crosshair would place it (`crosshair_target` isn't there while the world draws).
fn looked_at(host: &dyn ClientHost, level: LevelRef) -> Option<[i32; 3]> {
    let entity = host.local_player()?.entity();
    let eye = entity.eye_position(host)?;
    let [yaw, pitch] = entity.rotation(host)?;
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    let dir = [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
    let mut last = [eye[0].floor() as i32, eye[1].floor() as i32, eye[2].floor() as i32];
    for step in 1..=640 {
        let d = f64::from(step) * 0.05;
        let p = [eye[0] + dir[0] * d, eye[1] + dir[1] * d, eye[2] + dir[2] * d];
        let block = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
        if block == last {
            continue;
        }
        if !ritual::AIR.contains(&crate::game::block_name(host, level.block(host, block))) {
            return Some(last);
        }
        last = block;
    }
    None
}

fn player_yaw(host: &dyn ClientHost) -> Option<f32> {
    host.local_player()?.entity().rotation(host).map(|r| r[0])
}

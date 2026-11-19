// The unbound spirits' looks: Occultism's GeckoLib models and animations (spirit_geo.rs, written
// from Occultism's by tools/build_content.py), turned into a vanilla entity model layer and posed each
// frame as GeckoLib's default controller poses them (walking, else idle).
//
// A GeckoLib (Bedrock) model is y up, in texels from the entity's feet; a vanilla layer is y down
// from 24 texels above them. So a point (x, y, z) is (x, 24 - y, z) here, and a rotation (x, y, z)
// in degrees is (-x, y, -z) in radians: flipping y turns the rotations about x and z the other way.
// The box UV layouts are the same.

use crate::spirit_geo;
use mod_api::*;
use serde_json::Value;
use std::collections::HashMap;

/// A spirit's look: its layer's id, its texture and its animations.
pub struct Look {
    pub layer: &'static str,
    pub sprite: &'static str,
    geo: &'static str,
    animations: &'static str,
}

pub const LOOKS: [(&str, Look); 3] = [
    (
        crate::spirits::AFRIT,
        Look {
            layer: "occultism:afrit_unbound#main",
            sprite: "occultism:entity/afrit",
            geo: spirit_geo::AFRIT_GEO,
            animations: spirit_geo::AFRIT_ANIMATION,
        },
    ),
    (
        crate::spirits::MARID,
        Look {
            layer: "occultism:marid_unbound#main",
            sprite: "occultism:entity/marid",
            geo: spirit_geo::MARID_GEO,
            animations: spirit_geo::MARID_ANIMATION,
        },
    ),
    // A foliot in Occultism's janitor look, its one job yet.
    (
        crate::workers::FOLIOT,
        Look {
            layer: "occultism:foliot#main",
            sprite: "occultism:entity/foliot_janitor",
            geo: spirit_geo::FOLIOT_JANITOR_GEO,
            animations: spirit_geo::FOLIOT_JANITOR_ANIMATION,
        },
    ),
];

pub fn look(kind: &str) -> Option<&'static Look> {
    LOOKS.iter().find(|(k, _)| *k == kind).map(|(_, l)| l)
}

fn vec3(v: &Value) -> [f32; 3] {
    let n = |i: usize| v.get(i).and_then(Value::as_f64).unwrap_or(0.0) as f32;
    [n(0), n(1), n(2)]
}

/// A Bedrock rotation in degrees as a vanilla one in radians.
fn rotation(r: [f32; 3]) -> [f32; 3] {
    [(-r[0]).to_radians(), r[1].to_radians(), (-r[2]).to_radians()]
}

/// One cube on `builder`, placed from the part's `pivot` (Bedrock coordinates). A cube with
/// per-face UVs takes the box layout its north face starts (the two marid cubes that have them
/// are laid out as boxes).
fn add_cube(builder: CubeListBuilder, cube: &Value, pivot: [f32; 3]) -> CubeListBuilder {
    let (o, s) = (vec3(&cube["origin"]), vec3(&cube["size"]));
    let (u, v) = match &cube["uv"] {
        Value::Array(uv) => (uv[0].as_f64().unwrap_or(0.0), uv[1].as_f64().unwrap_or(0.0)),
        faces => {
            let north = vec3(&faces["north"]["uv"]);
            (f64::from(north[0] - s[2]), f64::from(north[1] - s[2]))
        }
    };
    let grow = CubeDeformation::new(cube["inflate"].as_f64().unwrap_or(0.0) as f32);
    builder
        .tex_offs(u.floor() as i32, v.floor() as i32)
        .set_mirror(cube["mirror"].as_bool().unwrap_or(false))
        .add_box_with([o[0] - pivot[0], pivot[1] - o[1] - s[1], o[2] - pivot[2]], s, grow)
}

/// The vanilla layer of a GeckoLib model: one part per bone, named as the bone, and a child part
/// for each cube turned on its own pivot.
pub fn layer(geo: &str) -> Option<LayerDefinition> {
    let json: Value = serde_json::from_str(geo).ok()?;
    let g = &json["minecraft:geometry"][0];
    let size = |k: &str| g["description"][k].as_u64().unwrap_or(64) as u32;
    let mut mesh = MeshDefinition::new();
    let mut placed: HashMap<String, (PartId, [f32; 3])> = HashMap::new();
    let mut left: Vec<&Value> = g["bones"].as_array()?.iter().collect();
    // Parents first, whatever order the file lists them in.
    while !left.is_empty() {
        let before = left.len();
        left.retain(|bone| {
            let name = bone["name"].as_str().unwrap_or_default();
            let (parent, parent_pivot) = match bone["parent"].as_str() {
                None => (mesh.root(), None),
                Some(p) => match placed.get(p) {
                    Some((id, pivot)) => (*id, Some(*pivot)),
                    None => return true,
                },
            };
            let pivot = vec3(&bone["pivot"]);
            let at = match parent_pivot {
                None => [pivot[0], 24.0 - pivot[1], pivot[2]],
                Some(pp) => [pivot[0] - pp[0], pp[1] - pivot[1], pivot[2] - pp[2]],
            };
            let r = rotation(vec3(&bone["rotation"]));
            let cubes = bone["cubes"].as_array().map(Vec::as_slice).unwrap_or_default();
            let mut plain = CubeListBuilder::create();
            for cube in cubes.iter().filter(|c| c["rotation"].is_null()) {
                plain = add_cube(plain, cube, pivot);
            }
            let id = mesh.add_or_replace_child(parent, name, plain, PartPose::offset_and_rotation(at[0], at[1], at[2], r[0], r[1], r[2]));
            for (i, cube) in cubes.iter().filter(|c| !c["rotation"].is_null()).enumerate() {
                let cp = if cube["pivot"].is_null() { pivot } else { vec3(&cube["pivot"]) };
                let cr = rotation(vec3(&cube["rotation"]));
                let pose = PartPose::offset_and_rotation(cp[0] - pivot[0], pivot[1] - cp[1], cp[2] - pivot[2], cr[0], cr[1], cr[2]);
                mesh.add_or_replace_child(id, &format!("{name}#{i}"), add_cube(CubeListBuilder::create(), cube, cp), pose);
            }
            placed.insert(name.to_string(), (id, pivot));
            false
        });
        if left.len() == before {
            break; // a bone whose parent isn't there
        }
    }
    Some(LayerDefinition::create(mesh, size("texture_width"), size("texture_height")))
}

/// A channel's keyframes: (seconds, value), in time order.
type Keys = Vec<(f32, [f32; 3])>;

#[derive(Default)]
struct Channels {
    rotation: Keys,
    position: Keys,
    scale: Keys,
}

pub struct Animation {
    length: f32,
    looped: bool,
    bones: Vec<(String, Channels)>,
}

fn keys(v: &Value) -> Keys {
    let value = |v: &Value| match v {
        Value::Array(_) => Some(vec3(v)),
        Value::Object(o) => o.get("vector").or_else(|| o.get("post")).map(vec3),
        Value::Number(n) => {
            let n = n.as_f64().unwrap_or(0.0) as f32;
            Some([n, n, n])
        }
        _ => None,
    };
    match v {
        Value::Object(o) if !o.contains_key("vector") => {
            let mut keys: Keys = o.iter().filter_map(|(t, k)| Some((t.parse().ok()?, value(k)?))).collect();
            keys.sort_by(|a, b| a.0.total_cmp(&b.0));
            keys
        }
        other => value(other).map(|v| vec![(0.0, v)]).unwrap_or_default(),
    }
}

/// The animations of a GeckoLib animation file, by name.
pub fn animations(text: &str) -> HashMap<String, Animation> {
    let Ok(json) = serde_json::from_str::<Value>(text) else { return HashMap::new() };
    let Some(all) = json["animations"].as_object() else { return HashMap::new() };
    all.iter()
        .map(|(name, a)| {
            let bones = a["bones"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(bone, ch)| {
                    (bone.clone(), Channels { rotation: keys(&ch["rotation"]), position: keys(&ch["position"]), scale: keys(&ch["scale"]) })
                })
                .collect();
            let looped = a["loop"].as_bool().unwrap_or(false);
            (name.clone(), Animation { length: a["animation_length"].as_f64().unwrap_or(1.0).max(0.05) as f32, looped, bones })
        })
        .collect()
}

/// A channel's value at `t` seconds, linear between keyframes.
fn sample(keys: &Keys, t: f32) -> Option<[f32; 3]> {
    let first = keys.first()?;
    if t <= first.0 {
        return Some(first.1);
    }
    for pair in keys.windows(2) {
        let ((t0, a), (t1, b)) = (pair[0], pair[1]);
        if t <= t1 {
            let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
            return Some(std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f));
        }
    }
    keys.last().map(|k| k.1)
}

fn find<'a>(part: &'a mut ModelPart, name: &str) -> Option<&'a mut ModelPart> {
    if part.get_child(name).is_some() {
        return part.get_child_mut(name);
    }
    for (_, child) in part.children_mut() {
        if let Some(found) = find(child, name) {
            return Some(found);
        }
    }
    None
}

impl Animation {
    /// `root`'s bones posed `seconds` into the animation, on top of their rest poses.
    pub fn pose(&self, root: &mut ModelPart, seconds: f32) {
        let t = if self.looped { seconds % self.length } else { seconds.min(self.length) };
        for (bone, ch) in &self.bones {
            let Some(part) = find(root, bone) else { continue };
            if let Some(r) = sample(&ch.rotation, t) {
                let r = rotation(r);
                part.x_rot += r[0];
                part.y_rot += r[1];
                part.z_rot += r[2];
            }
            if let Some(p) = sample(&ch.position, t) {
                part.x += p[0];
                part.y -= p[1];
                part.z += p[2];
            }
            if let Some(s) = sample(&ch.scale, t) {
                part.x_scale *= s[0];
                part.y_scale *= s[1];
                part.z_scale *= s[2];
            }
        }
    }
}

/// A spirit's baked model and animations, made on its first frame.
pub struct Posed {
    pub root: ModelPart,
    pub animations: HashMap<String, Animation>,
}

impl Posed {
    pub fn new(host: &dyn ClientHost, look: &Look) -> Option<Posed> {
        Some(Posed { root: host.bake_layer(look.layer)?, animations: animations(look.animations) })
    }

    /// Posed for a frame: walking or idle, the head turned to where it looks.
    pub fn pose(&mut self, mob: &MobRender<'_>) {
        self.root.reset_all_poses();
        let name = if mob.walk_speed > 0.05 { "walk" } else { "idle" };
        if let Some(animation) = self.animations.get(name) {
            animation.pose(&mut self.root, mob.age / 20.0);
        }
        if let Some(head) = find(&mut self.root, "Head") {
            head.y_rot += mob.head_yaw.to_radians();
            head.x_rot += mob.pitch.to_radians();
        }
    }
}

/// The layers this port adds (`register_layer_definitions`).
pub fn layers() -> Vec<(String, LayerDefinition)> {
    LOOKS.iter().filter_map(|(_, look)| Some((look.layer.to_string(), layer(look.geo)?))).collect()
}

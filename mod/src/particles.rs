// Occultism's ritual particles: the portal swirl over a golden bowl while its ritual runs, the gray
// smoke around it while the ritual waits for its sacrifice or item use (`RitualWaitingParticle`), and
// the spirit-fire flames that circle the next bowl the ritual takes from, then the golden bowl
// (`GoldenSacrificialBowlBlockEntity.tick`, `Ritual.markNextIngredient`).
//
// The two particle types of Occultism's own are declared in assets/occultism/particles/ (written by
// tools/build_content.py): the flame wholly there, the smoke moved by `RitualWaiting` below.

use mod_api::*;
use std::sync::{Arc, OnceLock};

pub const WAITING: &str = "occultism:ritual_waiting";
pub const FLAME: &str = "occultism:spirit_fire_flame";

/// The particle types the rituals send, resolved once at registration.
pub struct Keys {
    pub portal: ParticleKey,
    pub waiting: ParticleKey,
    pub flame: ParticleKey,
    /// The puff a leader's followers go in (`ParticleTypes.EXPLOSION`).
    pub explosion: ParticleKey,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

/// Asks for the particle types, as the mod registers.
pub fn request(r: &mut dyn Registrar) {
    let _ = KEYS.set(Keys { portal: r.particle_key("minecraft:portal"), waiting: r.particle_key(WAITING), flame: r.particle_key(FLAME), explosion: r.particle_key("minecraft:explosion") });
}

pub fn keys() -> Option<&'static Keys> {
    KEYS.get()
}

/// `count` particles of `key` around `at`, each offset by a Gaussian times `spread` and moving by
/// one times `speed`.
pub fn send(host: &mut dyn ServerHost, level: LevelRef, key: ParticleKey, at: [f64; 3], spread: [f64; 3], speed: f64, count: i32) {
    level.send_particles(host, &ParticleOptions::simple(key), None, at, spread, speed, count);
}

/// The ritual's waiting smoke (`RitualWaitingParticle`): it moves as vanilla's portal particle does,
/// falling a block onto where it was sent as it grows, in a random shade of dark gray.
pub struct RitualWaiting;

impl ParticleProvider for RitualWaiting {
    fn tick(&self, p: &mut ModParticle) {
        if p.age <= 1 {
            // Where it was sent, and its shade (0.4 to 1 of 0.3), from where that was: the tick has
            // no random of its own.
            let bits = (p.position[0] * 7919.0 + p.position[2] * 104_729.0).to_bits();
            let shade = 0.4 + 0.6 * ((bits.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f64 / f64::from(1u32 << 24));
            p.state = [p.position[0], p.position[1], p.position[2], shade * 0.3];
        }
        let f = f64::from(p.age) / f64::from(p.lifetime.max(1));
        let back = 1.0 - (-f + f * f * 2.0);
        p.position = [
            p.state[0] + p.velocity[0] * back,
            p.state[1] + p.velocity[1] * back + (1.0 - f),
            p.state[2] + p.velocity[2] * back,
        ];
        let g = p.state[3] as f32;
        p.rgba = [g, g, g, 1.0];
        let grown = 1.0 - (1.0 - f) * (1.0 - f);
        p.size = (0.06 * grown) as f32;
    }
}

/// The client's providers for the mod's particle types.
pub fn register_providers(event: &mut RegisterParticleProvidersEvent<'_>) {
    event.providers.register(WAITING.into(), Arc::new(RitualWaiting));
}

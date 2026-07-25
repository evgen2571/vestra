//! Deterministic camera-shake transform contribution.

use crate::animation::Transform2D;

#[allow(
    clippy::too_many_arguments,
    reason = "the evaluated shake signal is kept allocation-free"
)]
pub(super) fn apply(
    transform: &mut Transform2D,
    time: u128,
    position_amount: f64,
    rotation_radians: f64,
    scale_amount: f64,
    frequency: f64,
    seed: u64,
    attack: f64,
    decay: f64,
) {
    let seconds = time as f64 / 1_000_000_000.0;
    let attack = if attack <= 0.0 {
        1.0
    } else {
        (seconds / attack).clamp(0.0, 1.0)
    };
    let envelope = attack * (-seconds / decay.max(0.000_1)).exp();
    let phase = |channel| {
        let value = stable_seed(seed.wrapping_add(channel));
        f64::from((value >> 11) as u32) / f64::from(u32::MAX) * std::f64::consts::TAU
    };
    let sample = |channel: u64| {
        let primary = phase(channel);
        let secondary = phase(channel.wrapping_add(0x9e37_79b9));
        ((seconds * frequency * std::f64::consts::TAU + primary).sin()
            + 0.5 * (seconds * frequency * 1.618 * std::f64::consts::TAU + secondary).sin())
            / 1.5
    };
    transform.position.x += sample(0) * position_amount * envelope;
    transform.position.y += sample(1) * position_amount * envelope;
    transform.rotation_radians += sample(2) * rotation_radians * envelope;
    let scale = 1.0 + sample(3).abs() * scale_amount * envelope;
    transform.scale.x *= scale;
    transform.scale.y *= scale;
}

fn stable_seed(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

//! Smoke tests for the runtime bitrate cap: constant-quality encoders
//! must keep producing output after `set_bitrate_cap_kbps` /
//! `reset_bitrate_cap` without being rebuilt.

use vpx_encode::{Config, Encoder, VideoCodecId};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;

/// Deterministic pseudo-random I420 buffer so the encoder has
/// non-trivial content to compress.
fn synthetic_i420(seed: u32) -> Vec<u8> {
    let len = (WIDTH * HEIGHT) as usize * 3 / 2;
    let mut state = seed.wrapping_mul(2654435761).wrapping_add(1);
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state >> 24) as u8
        })
        .collect()
}

fn encode_one(encoder: &mut Encoder, pts: i64, seed: u32) -> usize {
    let data = synthetic_i420(seed);
    let packets = encoder.encode(pts, &data).expect("encode failed");
    packets.map(|f| f.data.len()).sum()
}

fn run_cap_cycle(codec: VideoCodecId) {
    let mut encoder = Encoder::new(Config {
        width: WIDTH,
        height: HEIGHT,
        timebase: [1, 30],
        bitrate: 8_000, // kbps cap at init
        quality: Some(25),
        codec,
    })
    .expect("encoder build failed");

    // Warm up until the encoder starts emitting packets — VP9's default
    // `g_lag_in_frames` buffers the first frames before any output.
    let mut pts: i64 = 0;
    let mut warmed_up = false;
    for _ in 0..60 {
        if encode_one(&mut encoder, pts, pts as u32 + 1) > 0 {
            warmed_up = true;
            break;
        }
        pts += 1;
    }
    assert!(warmed_up, "no output within the warm-up window");

    encoder.set_bitrate_cap_kbps(500).expect("tighten cap failed");
    let mut produced = 0usize;
    for _ in 0..10 {
        pts += 1;
        produced += encode_one(&mut encoder, pts, pts as u32 + 1);
    }
    assert!(produced > 0, "no output after tightening the cap");

    encoder.reset_bitrate_cap().expect("reset cap failed");
    let mut produced = 0usize;
    for _ in 0..10 {
        pts += 1;
        produced += encode_one(&mut encoder, pts, pts as u32 + 1);
    }
    assert!(produced > 0, "no output after resetting the cap");
}

#[test]
fn vp8_runtime_bitrate_cap() {
    run_cap_cycle(VideoCodecId::VP8);
}

#[cfg(feature = "vp9")]
#[test]
fn vp9_runtime_bitrate_cap() {
    run_cap_cycle(VideoCodecId::VP9);
}

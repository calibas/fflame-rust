//! Disk persistence for reference orbits (desktop only).
//!
//! Deep-zoom reference orbits are the expensive part of returning to
//! a bookmarked location — minutes of fixed-point iteration at high
//! limb counts. This store memoizes them exactly: a file is keyed on
//! the full request identity (center strings, precision, plane,
//! power, Ship variant) and holds the orbit plus the LIVE fixed-point
//! state, so a reloaded orbit deepens with `extend()` exactly like
//! the in-memory one (`deepen_is_an_append` semantics carry over).
//!
//! Nucleus-relocated orbits carry a view-dependent `ref_offset`
//! (pixel units at a given zoom + viewport height); files record the
//! provenance view and consumers rescale to theirs
//! (`offset_for_view`), so a stored orbit serves any view the
//! rescale can express. Offset-free orbits (Julia, Ship, plain
//! fallback) serve any view at their precision.
//!
//! Format: `FFORBIT1` magic, then a little-endian binary layout
//! written/read by `ReferenceOrbit::{to_bytes, from_bytes}` (the
//! fixed-point state is private to reference.rs). The orbit length
//! sits at a fixed offset right after the magic so staleness ("is the
//! file at least as deep as what we have?") is a 12-byte read.
//! Any parse failure or version mismatch is a miss, never an error.

use super::reference::ReferenceOrbit;
use std::path::{Path, PathBuf};

/// Bump on any layout change: old files then read as misses.
pub const MAGIC: &[u8; 8] = b"FFORBIT7";

/// Save when recomputing would actually hurt: orbit length times
/// limbs² is proportional to the fixed-point work done. ~2e6 is a
/// second-plus of CPU on typical hardware. Periodic (nucleus) orbits
/// are short but carry a Newton search — save those from deep zooms.
const SAVE_COST_THRESHOLD: u64 = 2_000_000;

/// Eviction caps: newest-first by modification time.
const MAX_FILES: usize = 24;

/// Byte cap, runtime-settable because the right value depends on what
/// the user is doing: one 10.1M-iteration reference is ~202 MB, so the
/// old 256 MB constant retained exactly ONE deep location and made a
/// second dive evict the first — at eight minutes each to rebuild.
static MAX_TOTAL_BYTES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1024 * 1024 * 1024);

/// Set the store's byte cap (from SystemSettings, in megabytes).
pub fn set_max_total_mb(mb: u32) {
    MAX_TOTAL_BYTES.store(
        (mb.max(1) as u64) * 1024 * 1024,
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// Bytes currently held by the store, for the settings display.
pub fn bytes_in_use() -> u64 {
    let Some(dir) = default_dir() else {
        return 0;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

/// Delete every stored orbit. The next visit to any deep location
/// pays its reference build again, so this is a deliberate act.
pub fn clear() {
    let Some(dir) = default_dir() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().is_some_and(|x| x == "orbit") {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// The default store directory, or None when app storage is
/// unavailable (headless CI without a profile, etc. — the store just
/// disables itself).
fn default_dir() -> Option<PathBuf> {
    let dir = crate::storage::backend::get_app_data_dir().ok()?.join("orbit_cache");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Content-addressed file name for a request identity.
pub fn key_for(
    center_re: &str,
    center_im: &str,
    n_limbs: usize,
    julia_c: Option<(f32, f32)>,
    power: u32,
    ship: bool,
    ship_variant: u32,
) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(center_re.as_bytes());
    h.update([0u8]);
    h.update(center_im.as_bytes());
    h.update([0u8]);
    h.update((n_limbs as u64).to_le_bytes());
    match julia_c {
        None => h.update([0u8; 9]),
        Some((re, im)) => {
            h.update([1u8]);
            h.update(re.to_le_bytes());
            h.update(im.to_le_bytes());
        }
    }
    h.update(power.to_le_bytes());
    h.update([ship as u8]);
    h.update(ship_variant.to_le_bytes());
    let digest = h.finalize();
    let mut name = String::with_capacity(36);
    for b in &digest[..16] {
        name.push_str(&format!("{b:02x}"));
    }
    name.push_str(".orbit");
    name
}

/// Orbit length and tried-hint recorded in a file (24-byte read), or
/// None on any mismatch — the cheap staleness probe. Both live in the
/// format's fixed prefix precisely so this stays a header read.
fn saved_meta(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 16];
    f.read_exact(&mut head).ok()?;
    if &head[..8] != MAGIC {
        return None;
    }
    Some((
        u32::from_le_bytes(head[8..12].try_into().unwrap()),
        u32::from_le_bytes(head[12..16].try_into().unwrap()),
    ))
}

/// Save into an explicit directory (tests). See [`maybe_save`] for
/// the production entry point with the cost gate.
pub fn save_to(dir: &Path, orbit: &ReferenceOrbit) -> bool {
    let name = key_for(
        &orbit.center_re,
        &orbit.center_im,
        orbit.n_limbs,
        orbit.julia_c,
        orbit.power,
        orbit.ship,
        orbit.ship_variant,
    );
    let path = dir.join(name);
    // Don't rewrite a file that is already at least as deep AND
    // already records the same tried-hint. The second half matters:
    // the fallback orbit built for a rejected hint is exactly as long
    // as the plain one already stored, so a length-only test would
    // drop the one fact that stops the next session repeating a
    // multi-minute rebuild.
    if let Some((len, hint)) = saved_meta(&path) {
        if len >= orbit.len() && hint == orbit.hint_period.unwrap_or(0) {
            return true;
        }
    }
    let bytes = orbit.to_bytes();
    let tmp = path.with_extension("orbit.tmp");
    if std::fs::write(&tmp, &bytes).is_err() {
        return false;
    }
    // Rename-over: readers never see a torn file.
    if std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return false;
    }
    trim(dir);
    true
}

/// Load from an explicit directory (tests). Validates identity,
/// magic/version, and — for relocated orbits — the (zoom, height)
/// the pixel-unit offset was measured at.
pub fn load_from(
    dir: &Path,
    center_re: &str,
    center_im: &str,
    n_limbs: usize,
    julia_c: Option<(f32, f32)>,
    power: u32,
    ship: bool,
    ship_variant: u32,
    zoom_log2: f64,
    height_px: f64,
) -> Option<ReferenceOrbit> {
    let name = key_for(center_re, center_im, n_limbs, julia_c, power, ship, ship_variant);
    let path = dir.join(name);
    let bytes = std::fs::read(&path).ok()?;
    let orbit = ReferenceOrbit::from_bytes(&bytes)?;
    // Defense in depth: the deserialized identity must serve the
    // request (hash collisions, hand-edited files).
    if !orbit.serves(
        center_re, center_im, n_limbs, julia_c, power, ship, ship_variant, orbit.map_params,
    )
        || orbit.n_limbs != n_limbs
    {
        return None;
    }
    // The offset carries its provenance view; consumers rescale via
    // offset_for_view. A view it can't rescale to is a miss.
    if !orbit.relocation_serves(zoom_log2, height_px) {
        return None;
    }
    Some(orbit)
}

/// Production save: cost-gated, into the default store directory.
pub fn maybe_save(orbit: &ReferenceOrbit) {
    let limbs = orbit.n_limbs as u64;
    let cost = orbit.len() as u64 * limbs * limbs;
    let precious_nucleus = orbit.periodic.is_some() && orbit.n_limbs >= 4;
    if cost < SAVE_COST_THRESHOLD && !precious_nucleus {
        return;
    }
    if let Some(dir) = default_dir() {
        save_to(&dir, orbit);
    }
}

/// Production load, from the default store directory.
#[allow(clippy::too_many_arguments)]
pub fn load(
    center_re: &str,
    center_im: &str,
    n_limbs: usize,
    julia_c: Option<(f32, f32)>,
    power: u32,
    ship: bool,
    ship_variant: u32,
    zoom_log2: f64,
    height_px: f64,
) -> Option<ReferenceOrbit> {
    let dir = default_dir()?;
    load_from(
        &dir, center_re, center_im, n_limbs, julia_c, power, ship, ship_variant, zoom_log2,
        height_px,
    )
}

/// Newest-first eviction to the file/byte caps.
fn trim(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_none_or(|x| x != "orbit") {
                return None;
            }
            let meta = e.metadata().ok()?;
            Some((meta.modified().ok()?, meta.len(), path))
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0)); // newest first
    let mut total = 0u64;
    for (i, (_, size, path)) in files.iter().enumerate() {
        total += size;
        if i >= MAX_FILES || total > MAX_TOTAL_BYTES.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from("output").join("orbit_store_test").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_rejected_hint_is_worth_a_rewrite() {
        // The fallback orbit built after a hint is found too shallow
        // is exactly as long as the plain orbit already stored, so a
        // length-only staleness test drops the one fact that stops
        // the next session repeating the whole computation.
        let dir = test_dir("hint_rewrite");
        let plain =
            ReferenceOrbit::compute("-0.5", "0.1", 60.0, None, 500, None, 2, false, 0, [0.0, 0.0]).unwrap();
        assert!(save_to(&dir, &plain));
        // Rebuilt rather than cloned: ReferenceOrbit is deliberately
        // not Clone (these reach hundreds of megabytes).
        let mut with_hint =
            ReferenceOrbit::compute("-0.5", "0.1", 60.0, None, 500, None, 2, false, 0, [0.0, 0.0]).unwrap();
        with_hint.hint_period = Some(4242);
        with_hint.hint_octave = -100;
        assert!(save_to(&dir, &with_hint));
        let loaded =
            load_from(&dir, "-0.5", "0.1", plain.n_limbs, None, 2, false, 0, 60.0, 320.0)
                .expect("hit");
        assert_eq!(
            loaded.hint_period,
            Some(4242),
            "the tried-hint must survive; without it the next session rebuilds"
        );
        assert_eq!(loaded.hint_octave, -100);
        // ...and the same save again is a no-op (nothing new to add).
        assert!(save_to(&dir, &with_hint));
        let again =
            load_from(&dir, "-0.5", "0.1", plain.n_limbs, None, 2, false, 0, 60.0, 320.0)
                .expect("hit");
        assert_eq!(again.hint_period, Some(4242));
    }

    #[test]
    fn roundtrip_serves_and_deepens_like_the_original() {
        let dir = test_dir("roundtrip");
        let orbit =
            ReferenceOrbit::compute("-0.5", "0.1", 60.0, None, 500, None, 2, false, 0, [0.0, 0.0]).unwrap();
        assert!(save_to(&dir, &orbit));
        let mut loaded =
            load_from(&dir, "-0.5", "0.1", orbit.n_limbs, None, 2, false, 0, 60.0, 320.0)
                .expect("hit");
        assert_eq!(loaded.len(), orbit.len());
        assert_eq!(loaded.orbit, orbit.orbit);
        // The live fixed-point state must have survived: deepening the
        // loaded orbit matches a fresh full compute exactly.
        loaded.extend(800);
        let fresh =
            ReferenceOrbit::compute("-0.5", "0.1", 60.0, None, 800, None, 2, false, 0, [0.0, 0.0]).unwrap();
        assert_eq!(loaded.orbit, fresh.orbit, "resumed deepening diverged");
    }

    #[test]
    fn misses_on_identity_and_view_mismatches() {
        let dir = test_dir("miss");
        let orbit =
            ReferenceOrbit::compute("-0.5", "0.1", 60.0, None, 200, None, 2, false, 0, [0.0, 0.0]).unwrap();
        let n = orbit.n_limbs;
        assert!(save_to(&dir, &orbit));
        // Different center / power / plane: different key, miss.
        assert!(load_from(&dir, "-0.6", "0.1", n, None, 2, false, 0, 60.0, 320.0).is_none());
        assert!(load_from(&dir, "-0.5", "0.1", n, None, 3, false, 0, 60.0, 320.0).is_none());
        assert!(
            load_from(&dir, "-0.5", "0.1", n, Some((0.1, 0.2)), 2, false, 0, 60.0, 320.0)
                .is_none()
        );
        // Offset-free orbit: a DIFFERENT view still hits (precision
        // is the key, the offset is zero).
        assert!(load_from(&dir, "-0.5", "0.1", n, None, 2, false, 0, 55.0, 640.0).is_some());
        // Corrupt magic: miss, not error.
        let name = key_for("-0.5", "0.1", n, None, 2, false, 0);
        let path = dir.join(name);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();
        assert!(load_from(&dir, "-0.5", "0.1", n, None, 2, false, 0, 60.0, 320.0).is_none());
    }

    #[test]
    fn shallower_file_is_not_rewritten_deeper_is() {
        let dir = test_dir("depth");
        let mut orbit =
            ReferenceOrbit::compute("-0.5", "0.1", 60.0, None, 300, None, 2, false, 0, [0.0, 0.0]).unwrap();
        let n = orbit.n_limbs;
        assert!(save_to(&dir, &orbit));
        let len_300 = load_from(&dir, "-0.5", "0.1", n, None, 2, false, 0, 60.0, 320.0)
            .unwrap()
            .len();
        // Deepen and re-save: the file must pick up the new depth.
        orbit.extend(600);
        assert!(save_to(&dir, &orbit));
        let len_600 = load_from(&dir, "-0.5", "0.1", n, None, 2, false, 0, 60.0, 320.0)
            .unwrap()
            .len();
        assert!(len_600 > len_300, "{len_600} vs {len_300}");
        // Saving the SHALLOW state again must not clobber the deep file.
        let shallow =
            ReferenceOrbit::compute("-0.5", "0.1", 60.0, None, 300, None, 2, false, 0, [0.0, 0.0]).unwrap();
        assert!(save_to(&dir, &shallow));
        let still = load_from(&dir, "-0.5", "0.1", n, None, 2, false, 0, 60.0, 320.0)
            .unwrap()
            .len();
        assert_eq!(still, len_600);
    }

    #[test]
    fn compressed_files_are_small_and_exact() {
        // The array section is corrections + an RLE e-stream; on a
        // bounded chaotic orbit the corrections are sparse, so the
        // file must be far below the raw 20 B/iteration -- while the
        // roundtrip stays BYTE-exact (the roundtrip test above pins
        // exactness; this pins that we actually compressed).
        let dir = test_dir("compressed_small");
        let orbit =
            ReferenceOrbit::compute("-1.5", "0", 60.0, None, 20_000, None, 2, false, 0, [0.0, 0.0])
                .unwrap();
        assert_eq!(orbit.len(), 20_001, "(-1.5, 0) is bounded");
        assert!(save_to(&dir, &orbit));
        let name = key_for("-1.5", "0", orbit.n_limbs, None, 2, false, 0);
        let size = std::fs::metadata(dir.join(name)).unwrap().len();
        let raw = 20u64 * 20_001;
        assert!(
            size < raw / 4,
            "compressed file is {size} B vs {raw} B raw -- compression regressed"
        );
        let loaded =
            load_from(&dir, "-1.5", "0", orbit.n_limbs, None, 2, false, 0, 60.0, 320.0)
                .expect("hit");
        assert_eq!(loaded.orbit, orbit.orbit);
        assert_eq!(loaded.orbit_lo, orbit.orbit_lo);
        assert_eq!(loaded.orbit_e, orbit.orbit_e);
    }

    #[test]
    fn deep_dip_orbit_roundtrips_exactly() {
        // The period-3 antenna nucleus: |Z| dips toward zero every 3
        // iterations, driving nonzero per-entry exponents and dense
        // corrections -- the compression scheme's worst case. Byte
        // exactness must hold anyway.
        let orbit = ReferenceOrbit::compute(
            "-1.7548776662466927600495088963585286918946",
            "0",
            120.0,
            None,
            2_000,
            None,
            2,
            false,
            0, [0.0, 0.0]
        )
        .unwrap();
        assert!(
            orbit.orbit_e.iter().any(|&e| e != 0),
            "the dip case must actually exercise nonzero exponents"
        );
        let dir = test_dir("deep_dip");
        assert!(save_to(&dir, &orbit));
        let loaded = load_from(
            &dir,
            "-1.7548776662466927600495088963585286918946",
            "0",
            orbit.n_limbs,
            None,
            2,
            false,
            0,
            120.0,
            320.0,
        )
        .expect("hit");
        assert_eq!(loaded.orbit, orbit.orbit);
        assert_eq!(loaded.orbit_lo, orbit.orbit_lo);
        assert_eq!(loaded.orbit_e, orbit.orbit_e);
        // Dip-dense orbits must fall back to RAW encoding rather
        // than ballooning past the FFORBIT5 size (52 B/correction on
        // nearly every entry is 2.6x worse than 20 B/iteration).
        let name = key_for(
            "-1.7548776662466927600495088963585286918946",
            "0",
            orbit.n_limbs,
            None,
            2,
            false,
            0,
        );
        let size = std::fs::metadata(dir.join(name)).unwrap().len();
        let raw = 20u64 * orbit.len() as u64 + 4 * 1024;
        assert!(size <= raw, "dip case ballooned: {size} B");
        // And the resumed live state still deepens identically.
        let mut deep = loaded;
        deep.extend(2_500);
        let fresh = ReferenceOrbit::compute(
            "-1.7548776662466927600495088963585286918946",
            "0",
            120.0,
            None,
            2_500,
            None,
            2,
            false,
            0, [0.0, 0.0]
        )
        .unwrap();
        assert_eq!(deep.orbit, fresh.orbit, "post-load deepening diverged");
    }

    #[test]
    fn trim_keeps_newest() {
        let dir = test_dir("trim");
        // Distinct centers → distinct keys.
        for i in 0..(MAX_FILES + 4) {
            let re = format!("-0.5000{i}");
            let orbit =
                ReferenceOrbit::compute(&re, "0.1", 60.0, None, 50, None, 2, false, 0, [0.0, 0.0]).unwrap();
            assert!(save_to(&dir, &orbit));
        }
        let count = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "orbit"))
            .count();
        assert!(count <= MAX_FILES, "{count} files survived trim");
    }
}

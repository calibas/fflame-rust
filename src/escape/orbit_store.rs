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
///
/// 50, not the 24 this started at. That number predates FFORBIT6's
/// compression, when a deep orbit on disk was ~200 MB and two dozen
/// was already more than the byte cap would ever allow; the same
/// orbit is ~2.8 MB now, so the file cap was evicting locations the
/// byte cap had ample room for.
const MAX_FILES: usize = 50;

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
///
/// Under `cargo test` this is a throwaway directory, NOT the user's
/// real cache. Tests reach the production load path through
/// `OrbitCache`, and once that path could serve a nearby center by
/// relocation it could also reach whatever the developer happened to
/// have cached: two tests began failing with a ten-million-iteration
/// orbit from a real deep zoom substituted for the tiny one they
/// built. Test outcomes must not depend on the contents of a
/// developer's home directory.
#[cfg(not(test))]
fn default_dir() -> Option<PathBuf> {
    let dir = crate::storage::backend::get_app_data_dir().ok()?.join("orbit_cache");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// The throwaway store directory used under `cargo test`, for tests
/// that drive the production save path and need to see its output.
#[cfg(test)]
pub fn test_store_dir() -> Option<PathBuf> {
    default_dir()
}

#[cfg(test)]
fn default_dir() -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(format!("fflame-orbit-store-test-{}", std::process::id()));
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
    map_params: [f32; 2],
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
    h.update(map_params[0].to_le_bytes());
    h.update(map_params[1].to_le_bytes());
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
    // The big-float families carry live state the file format has no
    // slot for (a BigComplex, not fixed point), and their orbits are
    // short: never stored, so never loaded.
    if super::reference::map_is_big(orbit.ship, orbit.ship_variant) {
        return false;
    }
    let name = key_for(
        &orbit.center_re,
        &orbit.center_im,
        orbit.n_limbs,
        orbit.julia_c,
        orbit.power,
        orbit.ship,
        orbit.ship_variant,
        orbit.map_params,
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
///
/// Tries the exact center first, then any stored orbit of the same map
/// that can be RELOCATED to this center (see [`load_nearby`]).
#[allow(clippy::too_many_arguments)]
pub fn load_from(
    dir: &Path,
    center_re: &str,
    center_im: &str,
    n_limbs: usize,
    julia_c: Option<(f32, f32)>,
    power: u32,
    ship: bool,
    ship_variant: u32,
    map_params: [f32; 2],
    zoom_log2: f64,
    height_px: f64,
) -> Option<ReferenceOrbit> {
    let name = key_for(
        center_re, center_im, n_limbs, julia_c, power, ship, ship_variant, map_params,
    );
    let exact = load_exact(
        &dir.join(name),
        center_re,
        center_im,
        n_limbs,
        julia_c,
        power,
        ship,
        ship_variant,
        map_params,
        zoom_log2,
        height_px,
    );
    if exact.is_some() {
        return exact;
    }
    load_nearby(
        dir, center_re, center_im, n_limbs, julia_c, power, ship, ship_variant, map_params,
        zoom_log2, height_px,
    )
}

/// One file, by exact identity.
#[allow(clippy::too_many_arguments)]
fn load_exact(
    path: &Path,
    center_re: &str,
    center_im: &str,
    n_limbs: usize,
    julia_c: Option<(f32, f32)>,
    power: u32,
    ship: bool,
    ship_variant: u32,
    map_params: [f32; 2],
    zoom_log2: f64,
    height_px: f64,
) -> Option<ReferenceOrbit> {
    let bytes = std::fs::read(path).ok()?;
    let orbit = ReferenceOrbit::from_bytes(&bytes)?;
    // Defense in depth: the deserialized identity must serve the
    // request (hash collisions, hand-edited files).
    if !orbit.serves(
        center_re, center_im, n_limbs, julia_c, power, ship, ship_variant, map_params,
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

/// How many relocation candidates are worth decoding before giving up
/// and rebuilding. The ranking is an estimate (`ref_offset` is stored
/// as f32), so the closest candidate can in principle be refused by
/// `relocate_to` while a slightly further one is accepted; three
/// covers that without turning a miss into seconds of decoding.
const RELOCATE_CANDIDATES: usize = 3;

/// Serve a center the store has no exact file for, by relocating a
/// nearby one.
///
/// The engine can re-anchor a reference orbit to any center within
/// [`MAX_RELOCATE_PX`](super::reference::MAX_RELOCATE_PX) — that is
/// what makes panning at depth free. The store could not, because it
/// keys on the exact center string: a pan of one pixel produced a key
/// miss and an eight-minute rebuild of an orbit already sitting on
/// disk. This closes that gap.
///
/// Candidates are ranked from FILE HEADERS, a few kilobytes each,
/// because decoding a ten-million-point body costs the better part of
/// a second and most files in the directory are other locations.
/// Only the best few are decoded, and `relocate_to` — working from the
/// exact fixed-point reference — makes the final decision.
#[allow(clippy::too_many_arguments)]
fn load_nearby(
    dir: &Path,
    center_re: &str,
    center_im: &str,
    n_limbs: usize,
    julia_c: Option<(f32, f32)>,
    power: u32,
    ship: bool,
    ship_variant: u32,
    map_params: [f32; 2],
    zoom_log2: f64,
    height_px: f64,
) -> Option<ReferenceOrbit> {
    // A Julia orbit's reference is its seed, which relocation cannot
    // move (see relocate_to) — no point reading the directory.
    if julia_c.is_some() {
        return None;
    }
    let h = height_px.max(1.0);
    // Parse the REQUEST's center once: it is the same for every
    // candidate, and a deep center is thousands of digits.
    let to = (
        super::fixedpoint::FixedPoint::from_decimal(center_re, n_limbs)?,
        super::fixedpoint::FixedPoint::from_decimal(center_im, n_limbs)?,
    );
    let mut ranked: Vec<(f64, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|x| x != "orbit") {
            continue;
        }
        let Some(head) = read_header(&path) else {
            continue;
        };
        // EQUAL precision, not merely sufficient. `serves_shape`
        // accepts a deeper orbit because an in-memory one is already
        // paid for; from disk it is not, and decoding a
        // ten-million-point 197-limb body to serve a shallow view
        // costs far more than rebuilding that view from scratch. The
        // exact-key path has always required this (n_limbs is part of
        // the key), so matching it adds the nearby case and changes
        // nothing else.
        if head.n_limbs != n_limbs
            || !head.serves_shape(n_limbs, julia_c, power, ship, ship_variant, map_params)
        {
            continue;
        }
        let Some(off) = head.offset_estimate_px(&to, zoom_log2, h) else {
            continue;
        };
        let far = super::reference::MAX_RELOCATE_PX;
        if !off[0].is_finite() || !off[1].is_finite() || off[0].abs() > far || off[1].abs() > far {
            continue;
        }
        ranked.push((off[0].hypot(off[1]), path));
    }
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0));

    for (_, path) in ranked.iter().take(RELOCATE_CANDIDATES) {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let Some(mut orbit) = ReferenceOrbit::from_bytes(&bytes) else {
            continue;
        };
        if orbit.n_limbs != n_limbs
            || !orbit.serves_shape(n_limbs, julia_c, power, ship, ship_variant, map_params)
        {
            continue;
        }
        if orbit.relocate_to(center_re, center_im, zoom_log2, h) {
            super::diag::update(|d| d.orbit_relocations += 1);
            return Some(orbit);
        }
    }
    None
}

/// A file's identity prefix, without decoding its body.
fn read_header(path: &Path) -> Option<super::reference::StoredHeader> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; super::reference::MAX_HEADER_BYTES];
    // Short files are fine: read_header validates as it goes, and a
    // truncated read simply fails to parse.
    let mut filled = 0;
    while filled < buf.len() {
        match f.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => return None,
        }
    }
    buf.truncate(filled);
    super::reference::header_from_bytes(&buf)
}

/// Production save: cost-gated, into the default store directory.
///
/// Takes `&mut` to CLEAR `store_grown` once the orbit is on disk. The
/// flag means "has grown since it was last persisted", and the clear
/// is what makes that true: a relocated orbit keeps its identity
/// strings updated to each new view center, so an orbit that stayed
/// flagged would be written out again, in full, under a fresh center
/// key on every pan and every wheel notch. Observed before the clear
/// existed: 24 files, 2.8 MB each, all holding the SAME 10.1M-iteration
/// orbit at centers a handful of pixels apart — a cache made entirely
/// of one location, having evicted every other.
pub fn maybe_save(orbit: &mut ReferenceOrbit) {
    if let Some(dir) = default_dir() {
        maybe_save_to(&dir, orbit);
    }
}

/// [`maybe_save`] into an explicit directory.
pub fn maybe_save_to(dir: &Path, orbit: &mut ReferenceOrbit) {
    // Nothing new to persist: loaded-from-store orbits that have not
    // deepened, and orbits relocated since their last save.
    if !orbit.store_grown {
        return;
    }
    if super::reference::map_is_big(orbit.ship, orbit.ship_variant) {
        return;
    }
    let limbs = orbit.n_limbs as u64;
    let cost = orbit.len() as u64 * limbs * limbs;
    let precious_nucleus = orbit.periodic.is_some() && orbit.n_limbs >= 4;
    if cost < SAVE_COST_THRESHOLD && !precious_nucleus {
        return;
    }
    if save_to(dir, orbit) {
        orbit.store_grown = false;
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
    map_params: [f32; 2],
    zoom_log2: f64,
    height_px: f64,
) -> Option<ReferenceOrbit> {
    let dir = default_dir()?;
    load_from(
        &dir, center_re, center_im, n_limbs, julia_c, power, ship, ship_variant, map_params,
        zoom_log2, height_px,
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
            load_from(
                &dir, "-0.5", "0.1", plain.n_limbs, None, 2, false, 0, [0.0, 0.0], 60.0, 320.0
            )
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
            load_from(
                &dir, "-0.5", "0.1", plain.n_limbs, None, 2, false, 0, [0.0, 0.0], 60.0, 320.0
            )
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
            load_from(
                &dir, "-0.5", "0.1", orbit.n_limbs, None, 2, false, 0, [0.0, 0.0], 60.0, 320.0
            )
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
        assert!(load_from(
            &dir, "-0.6", "0.1", n, None, 2, false, 0, [0.0, 0.0], 60.0, 320.0).is_none()
        );
        assert!(load_from(
            &dir, "-0.5", "0.1", n, None, 3, false, 0, [0.0, 0.0], 60.0, 320.0).is_none()
        );
        assert!(
            load_from(
                &dir, "-0.5", "0.1", n, Some((0.1, 0.2)), 2, false, 0, [0.0, 0.0], 60.0, 320.0
            )
                .is_none()
        );
        // Offset-free orbit: a DIFFERENT view still hits (precision
        // is the key, the offset is zero).
        assert!(load_from(
            &dir, "-0.5", "0.1", n, None, 2, false, 0, [0.0, 0.0], 55.0, 640.0).is_some()
        );
        // Corrupt magic: miss, not error.
        let name = key_for("-0.5", "0.1", n, None, 2, false, 0, [0.0, 0.0]);
        let path = dir.join(name);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();
        assert!(load_from(
            &dir, "-0.5", "0.1", n, None, 2, false, 0, [0.0, 0.0], 60.0, 320.0).is_none()
        );
    }

    #[test]
    fn shallower_file_is_not_rewritten_deeper_is() {
        let dir = test_dir("depth");
        let mut orbit =
            ReferenceOrbit::compute("-0.5", "0.1", 60.0, None, 300, None, 2, false, 0, [0.0, 0.0]).unwrap();
        let n = orbit.n_limbs;
        assert!(save_to(&dir, &orbit));
        let len_300 = load_from(&dir, "-0.5", "0.1", n, None, 2, false, 0, [0.0, 0.0], 60.0, 320.0)
            .unwrap()
            .len();
        // Deepen and re-save: the file must pick up the new depth.
        orbit.extend(600);
        assert!(save_to(&dir, &orbit));
        let len_600 = load_from(&dir, "-0.5", "0.1", n, None, 2, false, 0, [0.0, 0.0], 60.0, 320.0)
            .unwrap()
            .len();
        assert!(len_600 > len_300, "{len_600} vs {len_300}");
        // Saving the SHALLOW state again must not clobber the deep file.
        let shallow =
            ReferenceOrbit::compute("-0.5", "0.1", 60.0, None, 300, None, 2, false, 0, [0.0, 0.0]).unwrap();
        assert!(save_to(&dir, &shallow));
        let still = load_from(&dir, "-0.5", "0.1", n, None, 2, false, 0, [0.0, 0.0], 60.0, 320.0)
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
        let name = key_for("-1.5", "0", orbit.n_limbs, None, 2, false, 0, [0.0, 0.0]);
        let size = std::fs::metadata(dir.join(name)).unwrap().len();
        let raw = 20u64 * 20_001;
        assert!(
            size < raw / 4,
            "compressed file is {size} B vs {raw} B raw -- compression regressed"
        );
        let loaded =
            load_from(&dir, "-1.5", "0", orbit.n_limbs, None, 2, false, 0, [0.0, 0.0], 60.0, 320.0)
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
            0, [0.0, 0.0],
            120.0,
            320.0)
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
            0, [0.0, 0.0]);
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

    /// Two orbits that differ only in the map's continuous parameter
    /// must not share a file. The key hashes the whole map identity;
    /// without the parameter, a second Phoenix `p` would load the
    /// first one's orbit and render the wrong fractal from cache.
    #[test]
    fn different_map_parameters_get_different_files() {
        let a = key_for("-0.2", "0.35", 8, None, 2, false, 2, [-0.5, 0.0]);
        let b = key_for("-0.2", "0.35", 8, None, 2, false, 2, [0.25, 0.1]);
        let c = key_for("-0.2", "0.35", 8, None, 2, false, 2, [-0.5, 0.0]);
        assert_ne!(a, b, "different p must not collide");
        assert_eq!(a, c, "the same identity must be stable");
    }

    /// A center string at deep-zoom precision, nudged in its `digit`th
    /// decimal place — the shape a pan produces at depth.
    fn nudged_center(digit: usize) -> String {
        format!("-0.5{}1", "0".repeat(digit))
    }

    /// An orbit expensive enough to pass the save cost gate.
    fn costly_orbit(center_re: &str) -> ReferenceOrbit {
        let o = ReferenceOrbit::compute(
            center_re, "0.1", 2000.0, None, 3000, None, 2, false, 0, [0.0, 0.0],
        )
        .unwrap();
        let cost = o.len() as u64 * (o.n_limbs as u64).pow(2);
        assert!(cost >= SAVE_COST_THRESHOLD, "test orbit too cheap to be saved: {cost}");
        o
    }

    fn count_orbits(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "orbit"))
            .count()
    }

    /// THE 24-FILE BUG: panning must not rewrite the same orbit under
    /// every center it passes through.
    ///
    /// Reported from the app: a single deep dive left 24 files of
    /// 2.8 MB each, all holding the SAME 10.1M-iteration orbit at
    /// centers a handful of pixels apart, having evicted every other
    /// location. Relocation correctly avoided RECOMPUTING the orbit —
    /// 24 rebuilds could not fit in the 40 seconds they spanned — but
    /// each new view center produced a new cache key, and the orbit
    /// still counted as "grown", so it was serialized and written out
    /// again every time.
    #[test]
    fn a_relocated_orbit_is_not_resaved_under_every_center() {
        let dir = test_dir("relocation_resave");
        let mut orbit = costly_orbit("-0.5");
        maybe_save_to(&dir, &mut orbit);
        assert_eq!(count_orbits(&dir), 1, "the first save must land");

        // Pan across a run of nearby centers, as a drag does.
        let mut relocations = 0;
        for d in 0..12 {
            let c = nudged_center(600 + d);
            if orbit.relocate_to(&c, "0.1", 2000.0, 512.0) {
                relocations += 1;
                assert_eq!(orbit.center_re, c, "relocation updates the identity");
            }
            maybe_save_to(&dir, &mut orbit);
        }
        assert!(relocations >= 8, "test needs real relocations, got {relocations}");
        assert_eq!(
            count_orbits(&dir),
            1,
            "{relocations} relocations of ONE orbit wrote {} files — a pan is \
             rewriting the cache under every center it passes through",
            count_orbits(&dir)
        );
    }

    /// Deepening after a save must still be persisted: the flag means
    /// "grown since last written", not "never write again".
    #[test]
    fn deepening_after_a_save_is_still_written() {
        let dir = test_dir("resave_after_growth");
        let mut orbit = costly_orbit("-0.5");
        maybe_save_to(&dir, &mut orbit);
        let before = orbit.len();
        orbit.extend(before + 2000);
        assert!(orbit.len() > before, "test needs the orbit to actually grow");
        maybe_save_to(&dir, &mut orbit);
        let reloaded = load_from(
            &dir, "-0.5", "0.1", orbit.n_limbs, None, 2, false, 0, [0.0, 0.0], 2000.0, 512.0,
        )
        .expect("hit");
        assert_eq!(
            reloaded.len(),
            orbit.len(),
            "the deepened orbit was not persisted — the next session rebuilds it"
        );
    }

    /// A center the store has no file for is served by relocating a
    /// nearby one, instead of rebuilding from scratch.
    ///
    /// Without this the store could not serve what the engine could:
    /// one pixel of pan produced a key miss and a full rebuild of an
    /// orbit already sitting on disk.
    #[test]
    fn a_nearby_center_is_served_by_relocation() {
        let dir = test_dir("nearby_relocation");
        let mut orbit = costly_orbit("-0.5");
        let len = orbit.len();
        let limbs = orbit.n_limbs;
        maybe_save_to(&dir, &mut orbit);

        let near = nudged_center(600);
        let hit = load_from(
            &dir, &near, "0.1", limbs, None, 2, false, 0, [0.0, 0.0], 2000.0, 512.0,
        )
        .expect("a center a fraction of a pixel away must be served by relocation");
        assert_eq!(hit.len(), len, "it must be the stored orbit, not a rebuild");
        assert_eq!(hit.center_re, near, "and it must now serve the requested center");
        assert!(
            !hit.store_grown,
            "a relocated load has nothing new to persist"
        );
    }

    /// With a directory full of other locations, the scan must still
    /// find the one orbit that can be relocated here.
    ///
    /// Candidates are ranked and filtered from HEADERS, a few
    /// kilobytes each, because decoding a deep body costs the better
    /// part of a second — a miss among fifty stored locations must not
    /// become a multi-second stall.
    #[test]
    fn the_relocatable_candidate_is_found_among_many() {
        let dir = test_dir("nearby_ranking");
        // Decoys: same map, same precision, unrelated centers.
        for d in [80usize, 90, 100, 110, 120] {
            let mut decoy = costly_orbit(&nudged_center(d));
            maybe_save_to(&dir, &mut decoy);
        }
        let mut target = costly_orbit("-0.5");
        let len = target.len();
        let limbs = target.n_limbs;
        maybe_save_to(&dir, &mut target);
        assert_eq!(count_orbits(&dir), 6, "decoys and target must all be stored");

        let near = nudged_center(600);
        let hit = load_from(
            &dir, &near, "0.1", limbs, None, 2, false, 0, [0.0, 0.0], 2000.0, 512.0,
        )
        .expect("the one relocatable orbit must be found among the decoys");
        assert_eq!(hit.len(), len);
        assert_eq!(hit.center_re, near);
    }

    /// ...but only within relocation range. A center far away must
    /// MISS, not silently render against a reference that cannot
    /// serve it.
    #[test]
    fn a_far_center_is_not_served_by_relocation() {
        let dir = test_dir("far_no_relocation");
        let mut orbit = costly_orbit("-0.5");
        let limbs = orbit.n_limbs;
        maybe_save_to(&dir, &mut orbit);
        // At zoom 2^-2000 a pixel is ~1e-601 wide, so a nudge in the
        // 100th decimal place is astronomically out of range.
        let far = nudged_center(100);
        assert!(
            load_from(
                &dir, &far, "0.1", limbs, None, 2, false, 0, [0.0, 0.0], 2000.0, 512.0
            )
            .is_none(),
            "a center far outside relocation range must miss"
        );
    }

    /// A different map must never be relocated into, however close its
    /// center: the nearby scan widens what the store will serve, and
    /// identity is the thing it must not widen.
    #[test]
    fn relocation_never_crosses_map_identity() {
        let dir = test_dir("nearby_identity");
        let mut orbit = costly_orbit("-0.5");
        let limbs = orbit.n_limbs;
        maybe_save_to(&dir, &mut orbit);
        let near = nudged_center(600);
        for (power, ship, variant) in [(3u32, false, 0u32), (2, true, 0), (2, false, super::super::reference::MAP_CONJ)] {
            assert!(
                load_from(
                    &dir, &near, "0.1", limbs, None, power, ship, variant, [0.0, 0.0], 2000.0,
                    512.0
                )
                .is_none(),
                "a power-{power} ship-{ship} variant-{variant} request was served a \
                 power-2 Mandelbrot orbit"
            );
        }
    }

    /// The header probe must be able to parse any header the writer
    /// can produce from its first [`MAX_HEADER_BYTES`] bytes alone —
    /// that bound is what lets the store rank candidates without
    /// decoding ten-million-point bodies.
    #[test]
    fn a_header_probe_reads_enough() {
        let orbit = costly_orbit("-0.5");
        let bytes = orbit.to_bytes();
        let probe = &bytes[..super::super::reference::MAX_HEADER_BYTES.min(bytes.len())];
        let head = super::super::reference::header_from_bytes(probe)
            .expect("the header must parse from the probe prefix alone");
        assert_eq!(head.center_re, orbit.center_re);
        assert_eq!(head.n_limbs, orbit.n_limbs);
        assert_eq!(head.orbit_len, orbit.len() as usize);
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

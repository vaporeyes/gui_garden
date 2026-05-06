// ABOUTME: Doom WAD file parser - header, directory, and map lumps
// ABOUTME: (VERTEXES, LINEDEFS, THINGS) needed for the 2D map viewer.
//
// WAD ("Where's All the Data") is Doom's archive format. Layout:
//
//   header (12 bytes):
//     4   ASCII magic ("IWAD" or "PWAD")
//     4   u32 LE — number of directory entries
//     4   u32 LE — directory file offset
//
//   each directory entry (16 bytes):
//     4   u32 LE — lump file offset
//     4   u32 LE — lump size in bytes
//     8   ASCII name, null-padded
//
// Maps are stored as a "header" lump (e.g. "E1M1") followed by ten
// fixed-name data lumps in directory order. We only consume three of
// them (THINGS, LINEDEFS, VERTEXES) — enough for a top-down viewer.
//
// All multi-byte numbers in WAD are little-endian.
//
// Some fields on the parsed structs (`Wad::kind`, the unused `Linedef`
// special/tag/sidedef refs, `Map::bbox`, etc.) are populated but not
// yet consumed by the 2D viewer. They're public surface that the
// upcoming 3D BSP renderer will need, so we silence dead-code warnings
// at the type level rather than gating each field individually.

#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub enum WadError {
    Io(std::io::Error),
    Format(String),
}

impl std::fmt::Display for WadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WadError::Io(e) => write!(f, "I/O error: {}", e),
            WadError::Format(s) => write!(f, "WAD format error: {}", s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WadKind {
    Iwad,
    Pwad,
}

#[derive(Debug, Clone)]
pub struct Lump {
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

pub struct Wad {
    pub kind: WadKind,
    pub lumps: Vec<Lump>,
    /// Raw file bytes, shared so callers can keep the WAD alive while
    /// holding maps that reference back into it. Lump bytes are slices
    /// into this buffer.
    bytes: Arc<[u8]>,
}

impl Wad {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, WadError> {
        let bytes = std::fs::read(path).map_err(WadError::Io)?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, WadError> {
        if bytes.len() < 12 {
            return Err(WadError::Format("file shorter than header".into()));
        }
        let magic = &bytes[0..4];
        let kind = match magic {
            b"IWAD" => WadKind::Iwad,
            b"PWAD" => WadKind::Pwad,
            _ => {
                return Err(WadError::Format(format!(
                    "bad magic: {:?}",
                    String::from_utf8_lossy(magic)
                )))
            }
        };
        let num_lumps = read_u32_le(&bytes, 4) as usize;
        let dir_offset = read_u32_le(&bytes, 8) as usize;
        let dir_end = dir_offset
            .checked_add(num_lumps.checked_mul(16).ok_or_else(|| {
                WadError::Format(format!("directory size overflow ({} lumps)", num_lumps))
            })?)
            .ok_or_else(|| WadError::Format("directory end overflow".into()))?;
        if dir_end > bytes.len() {
            return Err(WadError::Format(format!(
                "directory extends past EOF (need {}, have {})",
                dir_end,
                bytes.len()
            )));
        }
        let mut lumps = Vec::with_capacity(num_lumps);
        for i in 0..num_lumps {
            let base = dir_offset + i * 16;
            let offset = read_u32_le(&bytes, base) as usize;
            let size = read_u32_le(&bytes, base + 4) as usize;
            let name = read_lump_name(&bytes[base + 8..base + 16]);
            // We don't validate offset+size < bytes.len() here; a few
            // lumps may have offset 0 for size 0 by convention. We
            // re-check on access in `lump_bytes`.
            lumps.push(Lump { name, offset, size });
        }
        Ok(Self {
            kind,
            lumps,
            bytes: Arc::from(bytes.into_boxed_slice()),
        })
    }

    pub fn lump_bytes(&self, idx: usize) -> Result<&[u8], WadError> {
        let l = self
            .lumps
            .get(idx)
            .ok_or_else(|| WadError::Format(format!("lump index {} out of range", idx)))?;
        if l.size == 0 {
            return Ok(&[]);
        }
        let end = l.offset.checked_add(l.size).ok_or_else(|| {
            WadError::Format(format!("lump {} offset+size overflow", l.name))
        })?;
        if end > self.bytes.len() {
            return Err(WadError::Format(format!(
                "lump {} ({}+{}) past EOF ({})",
                l.name,
                l.offset,
                l.size,
                self.bytes.len()
            )));
        }
        Ok(&self.bytes[l.offset..end])
    }

    pub fn find_lump_after(&self, start: usize, name: &str) -> Option<usize> {
        self.lumps
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, l)| l.name == name)
            .map(|(i, _)| i)
    }

    pub fn find_lump(&self, name: &str) -> Option<usize> {
        self.find_lump_after(0, name)
    }

    /// All lump names that look like map headers — `E#M#` (Doom 1) or
    /// `MAP##` (Doom 2 / Final Doom). Returned in directory order.
    pub fn map_names(&self) -> Vec<String> {
        self.lumps
            .iter()
            .filter(|l| is_map_header(&l.name))
            .map(|l| l.name.clone())
            .collect()
    }

    /// Fully parse a map by name. Looks up the header lump, then reads
    /// the named data lumps that should follow it within the next
    /// dozen-or-so directory entries. SECTORS and SIDEDEFS are parsed
    /// when present (almost always) so the viewer can render
    /// per-sector visualizations; their absence isn't fatal.
    pub fn load_map(&self, name: &str) -> Result<Map, WadError> {
        let header = self
            .find_lump(name)
            .ok_or_else(|| WadError::Format(format!("map '{}' not found", name)))?;

        let vertexes_idx = self
            .find_lump_after(header, "VERTEXES")
            .ok_or_else(|| WadError::Format(format!("map '{}': no VERTEXES lump", name)))?;
        let linedefs_idx = self
            .find_lump_after(header, "LINEDEFS")
            .ok_or_else(|| WadError::Format(format!("map '{}': no LINEDEFS lump", name)))?;
        let things_idx = self
            .find_lump_after(header, "THINGS")
            .ok_or_else(|| WadError::Format(format!("map '{}': no THINGS lump", name)))?;

        let vertexes = parse_vertexes(self.lump_bytes(vertexes_idx)?)?;
        let linedefs = parse_linedefs(self.lump_bytes(linedefs_idx)?)?;
        let things = parse_things(self.lump_bytes(things_idx)?)?;

        let sidedefs = match self.find_lump_after(header, "SIDEDEFS") {
            Some(idx) => parse_sidedefs(self.lump_bytes(idx)?)?,
            None => Vec::new(),
        };
        let sectors = match self.find_lump_after(header, "SECTORS") {
            Some(idx) => parse_sectors(self.lump_bytes(idx)?)?,
            None => Vec::new(),
        };

        let bbox = Bbox::from_vertexes(&vertexes);
        Ok(Map {
            name: name.to_string(),
            vertexes,
            linedefs,
            things,
            sidedefs,
            sectors,
            bbox,
        })
    }
}

fn is_map_header(name: &str) -> bool {
    let bytes = name.as_bytes();
    // E#M#  (4 chars) — episode # 1-9, map # 1-9
    if bytes.len() == 4
        && bytes[0] == b'E'
        && bytes[1].is_ascii_digit()
        && bytes[2] == b'M'
        && bytes[3].is_ascii_digit()
    {
        return true;
    }
    // MAP##  (5 chars) — Doom 2 / Final Doom
    if bytes.len() == 5
        && &bytes[0..3] == b"MAP"
        && bytes[3].is_ascii_digit()
        && bytes[4].is_ascii_digit()
    {
        return true;
    }
    false
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_i16_le(bytes: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_lump_name(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

// ---------- Map data ----------

#[derive(Debug, Clone)]
pub struct Map {
    pub name: String,
    pub vertexes: Vec<Vertex>,
    pub linedefs: Vec<Linedef>,
    pub things: Vec<Thing>,
    /// One sidedef per linedef face (1-sided lines have one, 2-sided
    /// have two). Each sidedef points at exactly one sector.
    pub sidedefs: Vec<Sidedef>,
    /// Sectors define floor/ceiling heights, light level, and
    /// floor/ceiling textures for the regions between linedefs.
    pub sectors: Vec<Sector>,
    pub bbox: Bbox,
}

impl Map {
    /// Sector index referenced by the `right_sidedef` of `ld`, if any.
    /// Most viewer code wants this for "which sector does this wall
    /// belong to" — convenience that handles the SIDEDEFS indirection.
    pub fn sector_of_right_side(&self, ld: &Linedef) -> Option<u16> {
        self.sidedefs
            .get(ld.right_sidedef as usize)
            .map(|sd| sd.sector)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub x: i16,
    pub y: i16,
}

#[derive(Debug, Clone, Copy)]
pub struct Linedef {
    pub v1: u16,
    pub v2: u16,
    pub flags: u16,
    pub special: u16,
    pub tag: u16,
    pub right_sidedef: u16,
    pub left_sidedef: u16,
}

impl Linedef {
    /// 2-sided linedefs sit between two sectors (they're step/portals
    /// rather than solid walls). Indicated by a non-`NO_SIDEDEF` left
    /// sidedef ref, *or* by the explicit `TWO_SIDED` flag (bit 2).
    pub fn two_sided(&self) -> bool {
        const NO_SIDEDEF: u16 = 0xFFFF;
        const TWO_SIDED_FLAG: u16 = 0x0004;
        self.left_sidedef != NO_SIDEDEF || (self.flags & TWO_SIDED_FLAG) != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Thing {
    pub x: i16,
    pub y: i16,
    pub angle: u16,
    pub doom_type: u16,
    pub flags: u16,
}

/// One face of a linedef. A 1-sided wall has only the `right_sidedef`
/// of its linedef set; 2-sided portals (step changes between sectors)
/// have both.
#[derive(Debug, Clone)]
pub struct Sidedef {
    pub x_offset: i16,
    pub y_offset: i16,
    /// 8-char texture name; "-" means "no texture on this face".
    pub upper_tex: String,
    pub lower_tex: String,
    pub middle_tex: String,
    /// Index into `Map::sectors` — which sector this face belongs to.
    pub sector: u16,
}

/// A region of the map with a floor + ceiling height, light level,
/// and floor/ceiling texture references. Linedefs reference sectors
/// indirectly via their sidedefs.
#[derive(Debug, Clone)]
pub struct Sector {
    pub floor_height: i16,
    pub ceiling_height: i16,
    pub floor_tex: String,
    pub ceiling_tex: String,
    pub light: u16,
    pub special: u16,
    pub tag: u16,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Bbox {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

impl Bbox {
    pub fn from_vertexes(vs: &[Vertex]) -> Self {
        if vs.is_empty() {
            return Self::default();
        }
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_y = i32::MAX;
        let mut max_y = i32::MIN;
        for v in vs {
            min_x = min_x.min(v.x as i32);
            max_x = max_x.max(v.x as i32);
            min_y = min_y.min(v.y as i32);
            max_y = max_y.max(v.y as i32);
        }
        Self {
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }
}

// ---------- Lump parsers ----------

const VERTEX_SIZE: usize = 4;
const LINEDEF_SIZE: usize = 14;
const THING_SIZE: usize = 10;
const SIDEDEF_SIZE: usize = 30;
const SECTOR_SIZE: usize = 26;

/// Read an 8-byte texture/flat name, null- or space-padded. Returns
/// the trimmed ASCII string with "-" preserved (Doom uses it as the
/// "no texture" sentinel).
fn read_tex_name(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn parse_vertexes(bytes: &[u8]) -> Result<Vec<Vertex>, WadError> {
    if !bytes.len().is_multiple_of(VERTEX_SIZE) {
        return Err(WadError::Format(format!(
            "VERTEXES not multiple of {}: {} bytes",
            VERTEX_SIZE,
            bytes.len()
        )));
    }
    let n = bytes.len() / VERTEX_SIZE;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let base = i * VERTEX_SIZE;
        out.push(Vertex {
            x: read_i16_le(bytes, base),
            y: read_i16_le(bytes, base + 2),
        });
    }
    Ok(out)
}

fn parse_linedefs(bytes: &[u8]) -> Result<Vec<Linedef>, WadError> {
    if !bytes.len().is_multiple_of(LINEDEF_SIZE) {
        return Err(WadError::Format(format!(
            "LINEDEFS not multiple of {}: {} bytes",
            LINEDEF_SIZE,
            bytes.len()
        )));
    }
    let n = bytes.len() / LINEDEF_SIZE;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let base = i * LINEDEF_SIZE;
        out.push(Linedef {
            v1: read_u16_le(bytes, base),
            v2: read_u16_le(bytes, base + 2),
            flags: read_u16_le(bytes, base + 4),
            special: read_u16_le(bytes, base + 6),
            tag: read_u16_le(bytes, base + 8),
            right_sidedef: read_u16_le(bytes, base + 10),
            left_sidedef: read_u16_le(bytes, base + 12),
        });
    }
    Ok(out)
}

fn parse_things(bytes: &[u8]) -> Result<Vec<Thing>, WadError> {
    if !bytes.len().is_multiple_of(THING_SIZE) {
        return Err(WadError::Format(format!(
            "THINGS not multiple of {}: {} bytes",
            THING_SIZE,
            bytes.len()
        )));
    }
    let n = bytes.len() / THING_SIZE;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let base = i * THING_SIZE;
        out.push(Thing {
            x: read_i16_le(bytes, base),
            y: read_i16_le(bytes, base + 2),
            angle: read_u16_le(bytes, base + 4),
            doom_type: read_u16_le(bytes, base + 6),
            flags: read_u16_le(bytes, base + 8),
        });
    }
    Ok(out)
}

fn parse_sidedefs(bytes: &[u8]) -> Result<Vec<Sidedef>, WadError> {
    if !bytes.len().is_multiple_of(SIDEDEF_SIZE) {
        return Err(WadError::Format(format!(
            "SIDEDEFS not multiple of {}: {} bytes",
            SIDEDEF_SIZE,
            bytes.len()
        )));
    }
    let n = bytes.len() / SIDEDEF_SIZE;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let base = i * SIDEDEF_SIZE;
        out.push(Sidedef {
            x_offset: read_i16_le(bytes, base),
            y_offset: read_i16_le(bytes, base + 2),
            upper_tex: read_tex_name(&bytes[base + 4..base + 12]),
            lower_tex: read_tex_name(&bytes[base + 12..base + 20]),
            middle_tex: read_tex_name(&bytes[base + 20..base + 28]),
            sector: read_u16_le(bytes, base + 28),
        });
    }
    Ok(out)
}

fn parse_sectors(bytes: &[u8]) -> Result<Vec<Sector>, WadError> {
    if !bytes.len().is_multiple_of(SECTOR_SIZE) {
        return Err(WadError::Format(format!(
            "SECTORS not multiple of {}: {} bytes",
            SECTOR_SIZE,
            bytes.len()
        )));
    }
    let n = bytes.len() / SECTOR_SIZE;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let base = i * SECTOR_SIZE;
        out.push(Sector {
            floor_height: read_i16_le(bytes, base),
            ceiling_height: read_i16_le(bytes, base + 2),
            floor_tex: read_tex_name(&bytes[base + 4..base + 12]),
            ceiling_tex: read_tex_name(&bytes[base + 12..base + 20]),
            light: read_u16_le(bytes, base + 20),
            special: read_u16_le(bytes, base + 22),
            tag: read_u16_le(bytes, base + 24),
        });
    }
    Ok(out)
}

/// Coarse bucket for a Doom thing type — used by the 2D viewer to
/// decide what color dot to draw. Numbers come from the standard Doom
/// editor reference; we only encode the buckets, not specific items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThingCategory {
    PlayerStart,
    Monster,
    Weapon,
    Ammo,
    HealthArmor,
    Key,
    Decoration,
    Other,
}

impl ThingCategory {
    pub fn classify(doom_type: u16) -> Self {
        match doom_type {
            1..=4 | 11 => Self::PlayerStart,
            // E1 monsters: trooper (3004), sergeant (9), imp (3001),
            // demon (3002), spectre (58), baron (3003), cacodemon (3005),
            // lost soul (3006), and a couple others.
            9 | 58 | 3001..=3006 => Self::Monster,
            // Weapons.
            2001..=2006 => Self::Weapon,
            // Ammo.
            2007..=2010 | 2046..=2049 => Self::Ammo,
            // Health / armor.
            2011..=2015 | 2018..=2019 | 2022..=2025 => Self::HealthArmor,
            // Keys.
            5..=6 | 13 | 38..=40 => Self::Key,
            // Common decorations: lamps, candelabra, gore.
            14..=15 | 25..=37 | 41..=63 | 70..=89 => Self::Decoration,
            _ => Self::Other,
        }
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic minimal WAD with two empty lumps.
    fn synthetic_wad() -> Vec<u8> {
        // Header (12) + dir (2 entries × 16) = 44 bytes; lump payload empty.
        let mut buf = Vec::new();
        buf.extend_from_slice(b"IWAD");
        buf.extend_from_slice(&2u32.to_le_bytes()); // num lumps
        buf.extend_from_slice(&12u32.to_le_bytes()); // dir offset
        // entry 0: name "PLAYPAL", offset 0, size 0
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(b"PLAYPAL\0");
        // entry 1: name "E1M1", offset 0, size 0
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(b"E1M1\0\0\0\0");
        buf
    }

    #[test]
    fn parse_synthetic_header() {
        let wad = Wad::from_bytes(synthetic_wad()).unwrap();
        assert_eq!(wad.kind, WadKind::Iwad);
        assert_eq!(wad.lumps.len(), 2);
        assert_eq!(wad.lumps[0].name, "PLAYPAL");
        assert_eq!(wad.lumps[1].name, "E1M1");
    }

    #[test]
    fn map_names_filters_correctly() {
        let wad = Wad::from_bytes(synthetic_wad()).unwrap();
        let names = wad.map_names();
        assert_eq!(names, vec!["E1M1".to_string()]);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = synthetic_wad();
        bytes[0] = b'X';
        assert!(matches!(Wad::from_bytes(bytes), Err(WadError::Format(_))));
    }

    #[test]
    fn rejects_short_file() {
        assert!(matches!(
            Wad::from_bytes(vec![1, 2, 3]),
            Err(WadError::Format(_))
        ));
    }

    #[test]
    fn is_map_header_recognizes_doom1_and_doom2_styles() {
        assert!(is_map_header("E1M1"));
        assert!(is_map_header("E3M9"));
        assert!(is_map_header("MAP01"));
        assert!(is_map_header("MAP32"));
        assert!(!is_map_header("E1"));
        assert!(!is_map_header("MAP1"));
        assert!(!is_map_header("PLAYPAL"));
        assert!(!is_map_header("THINGS"));
    }

    #[test]
    fn linedef_two_sided_via_left_sidedef() {
        let solid = Linedef {
            v1: 0,
            v2: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 0xFFFF,
        };
        assert!(!solid.two_sided());
        let portal = Linedef {
            left_sidedef: 1,
            ..solid
        };
        assert!(portal.two_sided());
    }

    #[test]
    fn thing_classify_buckets() {
        assert_eq!(ThingCategory::classify(1), ThingCategory::PlayerStart);
        assert_eq!(ThingCategory::classify(3001), ThingCategory::Monster);
        assert_eq!(ThingCategory::classify(2001), ThingCategory::Weapon);
        assert_eq!(ThingCategory::classify(2011), ThingCategory::HealthArmor);
        assert_eq!(ThingCategory::classify(50000), ThingCategory::Other);
    }

    #[test]
    fn parse_sidedefs_round_trip() {
        let mut bytes = vec![0u8; SIDEDEF_SIZE];
        bytes[0..2].copy_from_slice(&8i16.to_le_bytes());
        bytes[2..4].copy_from_slice(&(-4i16).to_le_bytes());
        bytes[4..12].copy_from_slice(b"STARTAN3");
        bytes[12..20].copy_from_slice(b"-       ");
        bytes[20..28].copy_from_slice(b"DOORSTOP");
        bytes[28..30].copy_from_slice(&7u16.to_le_bytes());
        let sds = parse_sidedefs(&bytes).unwrap();
        assert_eq!(sds.len(), 1);
        assert_eq!(sds[0].x_offset, 8);
        assert_eq!(sds[0].y_offset, -4);
        assert_eq!(sds[0].upper_tex, "STARTAN3");
        assert_eq!(sds[0].lower_tex, "-");
        assert_eq!(sds[0].middle_tex, "DOORSTOP");
        assert_eq!(sds[0].sector, 7);
    }

    #[test]
    fn parse_sectors_round_trip() {
        let mut bytes = vec![0u8; SECTOR_SIZE];
        bytes[0..2].copy_from_slice(&0i16.to_le_bytes());
        bytes[2..4].copy_from_slice(&128i16.to_le_bytes());
        bytes[4..12].copy_from_slice(b"FLOOR4_8");
        bytes[12..20].copy_from_slice(b"CEIL3_5\0");
        bytes[20..22].copy_from_slice(&160u16.to_le_bytes());
        bytes[22..24].copy_from_slice(&0u16.to_le_bytes());
        bytes[24..26].copy_from_slice(&0u16.to_le_bytes());
        let s = parse_sectors(&bytes).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].floor_height, 0);
        assert_eq!(s[0].ceiling_height, 128);
        assert_eq!(s[0].floor_tex, "FLOOR4_8");
        assert_eq!(s[0].ceiling_tex, "CEIL3_5");
        assert_eq!(s[0].light, 160);
    }

    #[test]
    fn parse_vertexes_round_trip() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(-128i16).to_le_bytes());
        bytes.extend_from_slice(&512i16.to_le_bytes());
        bytes.extend_from_slice(&0i16.to_le_bytes());
        bytes.extend_from_slice(&(-32768i16).to_le_bytes());
        let vs = parse_vertexes(&bytes).unwrap();
        assert_eq!(vs.len(), 2);
        assert_eq!(vs[0].x, -128);
        assert_eq!(vs[0].y, 512);
        assert_eq!(vs[1].x, 0);
        assert_eq!(vs[1].y, -32768);
    }
}

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

/// Human-readable name for a Doom thing type. Returns `None` for
/// numbers not in our small table — the viewer falls back to "Type #"
/// in that case rather than fudging a guess. Coverage focuses on
/// shareware Doom 1 contents (E1) since that's what most users will
/// load; full Doom 2 / Plutonia / TNT bestiary is intentionally out
/// of scope here.
pub fn thing_type_name(doom_type: u16) -> Option<&'static str> {
    Some(match doom_type {
        // Player starts.
        1 => "Player 1 start",
        2 => "Player 2 start",
        3 => "Player 3 start",
        4 => "Player 4 start",
        11 => "Deathmatch start",
        14 => "Teleport landing",
        // Monsters (Doom 1 set).
        9 => "Shotgun guy",
        58 => "Spectre",
        3001 => "Imp",
        3002 => "Demon (pinky)",
        3003 => "Baron of Hell",
        3004 => "Zombieman",
        3005 => "Cacodemon",
        3006 => "Lost soul",
        // Weapons.
        2001 => "Shotgun",
        2002 => "Chaingun",
        2003 => "Rocket launcher",
        2004 => "Plasma rifle",
        2005 => "Chainsaw",
        2006 => "BFG 9000",
        // Ammo.
        2007 => "Clip (10 bullets)",
        2008 => "Shotgun shells (4)",
        2010 => "Rocket",
        2046 => "Box of rockets (5)",
        2047 => "Cell (20)",
        2048 => "Box of bullets (50)",
        2049 => "Box of shells (20)",
        // Health & armor.
        2011 => "Stimpack (+10 hp)",
        2012 => "Medikit (+25 hp)",
        2013 => "Soulsphere (+100)",
        2014 => "Health bonus (+1)",
        2015 => "Armor bonus (+1)",
        2018 => "Green armor (100)",
        2019 => "Blue armor (200)",
        2022 => "Invulnerability",
        2023 => "Berserk",
        2024 => "Invisibility",
        2025 => "Radiation suit",
        2026 => "Computer area map",
        2045 => "Light goggles",
        // Keys.
        5 => "Blue keycard",
        6 => "Yellow keycard",
        13 => "Red keycard",
        38 => "Red skull key",
        39 => "Yellow skull key",
        40 => "Blue skull key",
        // Common decorations.
        15 => "Dead player",
        18 => "Dead zombieman",
        19 => "Dead shotgun guy",
        20 => "Dead imp",
        21 => "Dead demon",
        22 => "Dead cacodemon",
        25 => "Impaled human",
        26 => "Twitching impaled human",
        27 => "Skull on stake",
        28 => "5 skulls on stake",
        29 => "Pile of skulls and candles",
        30 => "Tall green pillar",
        31 => "Short green pillar",
        32 => "Tall red pillar",
        33 => "Short red pillar",
        34 => "Candle",
        35 => "Candelabra",
        36 => "Tall short pillar with heart",
        37 => "Tall short pillar with skull",
        41 => "Evil eye",
        42 => "Floating skull",
        43 => "Burning tree",
        44 => "Tall blue torch",
        45 => "Tall green torch",
        46 => "Tall red torch",
        47 => "Stalagmite",
        48 => "Tall techno column",
        49 => "Hanging victim, twitching",
        50 => "Hanging victim, arms out",
        51 => "Hanging victim, one-legged",
        52 => "Hanging pair of legs",
        53 => "Hanging leg",
        54 => "Large brown tree",
        55 => "Short blue torch",
        56 => "Short green torch",
        57 => "Short red torch",
        59 => "Hanging victim, twitching (block)",
        60 => "Hanging victim, arms out (block)",
        61 => "Hanging pair of legs (block)",
        62 => "Hanging leg (block)",
        63 => "Hanging victim, one-legged (block)",
        70 => "Burning barrel",
        2028 => "Floor lamp",
        2035 => "Exploding barrel",
        _ => return None,
    })
}

/// Common Doom linedef specials. Covers doors, switches, exits, lifts,
/// stairs — the high-traffic types most relevant when inspecting a
/// map. Long-tail specials return `None` and the viewer falls back to
/// "Special N".
pub fn linedef_special_name(special: u16) -> Option<&'static str> {
    Some(match special {
        0 => "(none)",
        // Doors.
        1 => "DR Door (open, close after delay)",
        2 => "W1 Door (open, stay)",
        3 => "W1 Door (close)",
        4 => "W1 Door (open, close after delay)",
        16 => "W1 Door (close, open after 30s)",
        26 => "DR Blue Door (locked)",
        27 => "DR Yellow Door (locked)",
        28 => "DR Red Door (locked)",
        29 => "S1 Door (open, close)",
        31 => "D1 Door (open, stay)",
        32 => "D1 Blue Door (locked)",
        33 => "D1 Red Door (locked)",
        34 => "D1 Yellow Door (locked)",
        46 => "GR Door (open, stay)",
        61 => "SR Door (open, stay)",
        63 => "SR Door (open, close)",
        90 => "WR Door (open, close)",
        99 => "SR Blue Door (locked)",
        103 => "S1 Door (open, stay)",
        108 => "W1 Door fast (open, close)",
        109 => "W1 Door fast (open, stay)",
        110 => "W1 Door fast (close)",
        111 => "S1 Door fast (open, close)",
        112 => "S1 Door fast (open, stay)",
        113 => "S1 Door fast (close)",
        114 => "SR Door fast (open, close)",
        115 => "SR Door fast (open, stay)",
        116 => "SR Door fast (close)",
        117 => "DR Door fast (open, close)",
        118 => "D1 Door fast (open, stay)",
        // Exits.
        11 => "S1 Exit level",
        51 => "S1 Exit level (secret)",
        52 => "W1 Exit level",
        124 => "W1 Exit level (secret)",
        // Lifts (platforms).
        10 => "W1 Lift (down, wait, up)",
        21 => "S1 Lift (down, wait, up)",
        62 => "SR Lift (down, wait, up)",
        88 => "WR Lift (down, wait, up)",
        121 => "W1 Lift fast",
        122 => "S1 Lift fast",
        123 => "SR Lift fast",
        // Floors.
        5 => "W1 Floor up to lowest ceiling",
        14 => "S1 Floor up 32",
        18 => "S1 Floor up to next higher",
        19 => "W1 Floor down to highest",
        20 => "S1 Floor up to next higher (change tex)",
        23 => "S1 Floor down to lowest",
        30 => "W1 Floor up by shortest lower texture",
        36 => "W1 Floor down to 8 above highest",
        38 => "W1 Floor down to lowest",
        58 => "W1 Floor up 24",
        59 => "W1 Floor up 24 (change tex)",
        // Stairs.
        7 => "S1 Stairs (slow, +8)",
        8 => "W1 Stairs (slow, +8)",
        100 => "W1 Stairs (fast, +16)",
        127 => "S1 Stairs (fast, +16)",
        // Teleporters.
        39 => "W1 Teleport",
        97 => "WR Teleport",
        125 => "W1 Teleport (monsters only)",
        126 => "WR Teleport (monsters only)",
        // Lighting.
        12 => "W1 Light to highest neighbour",
        13 => "W1 Light to 255",
        17 => "W1 Light strobe",
        35 => "W1 Light to 35",
        79 => "WR Light to 35",
        80 => "WR Light to highest neighbour",
        81 => "WR Light to 255",
        104 => "W1 Light to lowest neighbour",
        // Crushers.
        6 => "W1 Crusher fast",
        25 => "W1 Crusher",
        49 => "S1 Crusher",
        73 => "WR Crusher",
        77 => "WR Crusher fast",
        _ => return None,
    })
}

/// Decode the standard Doom linedef flag bits.
pub fn linedef_flag_descriptions(flags: u16) -> Vec<&'static str> {
    let mut out = Vec::new();
    if flags & 0x0001 != 0 {
        out.push("Impassable");
    }
    if flags & 0x0002 != 0 {
        out.push("Block monsters");
    }
    if flags & 0x0004 != 0 {
        out.push("Two-sided");
    }
    if flags & 0x0008 != 0 {
        out.push("Upper unpegged");
    }
    if flags & 0x0010 != 0 {
        out.push("Lower unpegged");
    }
    if flags & 0x0020 != 0 {
        out.push("Secret (drawn 1-sided on map)");
    }
    if flags & 0x0040 != 0 {
        out.push("Block sound");
    }
    if flags & 0x0080 != 0 {
        out.push("Hidden on map");
    }
    if flags & 0x0100 != 0 {
        out.push("Always shown on map");
    }
    out
}

/// Common Doom sector specials.
pub fn sector_special_name(special: u16) -> Option<&'static str> {
    Some(match special {
        0 => "Normal",
        1 => "Random light flicker",
        2 => "Strobe fast",
        3 => "Strobe slow",
        4 => "Strobe + 20% damage",
        5 => "10% damage / sec",
        7 => "5% damage / sec",
        8 => "Glow / oscillate",
        9 => "Secret",
        10 => "Door close after 30 sec",
        11 => "20% damage + end level",
        12 => "Sync strobe slow",
        13 => "Sync strobe fast",
        14 => "Door open after 300 sec",
        16 => "20% damage / sec",
        17 => "Random light flicker",
        _ => return None,
    })
}

/// Decode the standard Doom thing flag bits into human-readable
/// strings, in order. Empty vec for `flags == 0`.
pub fn thing_flag_descriptions(flags: u16) -> Vec<&'static str> {
    let mut out = Vec::new();
    if flags & 0x0001 != 0 {
        out.push("Skill 1-2 (easy)");
    }
    if flags & 0x0002 != 0 {
        out.push("Skill 3 (medium)");
    }
    if flags & 0x0004 != 0 {
        out.push("Skill 4-5 (hard)");
    }
    if flags & 0x0008 != 0 {
        out.push("Deaf / ambush");
    }
    if flags & 0x0010 != 0 {
        out.push("Multiplayer only");
    }
    out
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
    fn thing_type_name_known_and_unknown() {
        assert_eq!(thing_type_name(1), Some("Player 1 start"));
        assert_eq!(thing_type_name(3001), Some("Imp"));
        assert_eq!(thing_type_name(2003), Some("Rocket launcher"));
        assert_eq!(thing_type_name(50000), None);
    }

    #[test]
    fn linedef_special_name_known_and_unknown() {
        assert_eq!(linedef_special_name(0), Some("(none)"));
        assert_eq!(linedef_special_name(1), Some("DR Door (open, close after delay)"));
        assert_eq!(linedef_special_name(11), Some("S1 Exit level"));
        assert!(linedef_special_name(50000).is_none());
    }

    #[test]
    fn linedef_flag_descriptions_decodes_bits() {
        assert!(linedef_flag_descriptions(0).is_empty());
        let combo = linedef_flag_descriptions(0x0001 | 0x0004 | 0x0020);
        assert_eq!(
            combo,
            vec![
                "Impassable",
                "Two-sided",
                "Secret (drawn 1-sided on map)",
            ]
        );
    }

    #[test]
    fn sector_special_name_known_and_unknown() {
        assert_eq!(sector_special_name(0), Some("Normal"));
        assert_eq!(sector_special_name(9), Some("Secret"));
        assert!(sector_special_name(50000).is_none());
    }

    #[test]
    fn thing_flag_descriptions_decodes_bits() {
        assert!(thing_flag_descriptions(0).is_empty());
        let easy_deaf = thing_flag_descriptions(0x0001 | 0x0008);
        assert_eq!(easy_deaf, vec!["Skill 1-2 (easy)", "Deaf / ambush"]);
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

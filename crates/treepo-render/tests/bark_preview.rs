//! ★ The instrument for judging the surface — the one thing a test cannot assert.
//!
//! Every material constant in [`treepo_render::surface`] that was "set by argument" is waiting
//! on a picture. `claude-progress.md` says so four times over: `blend_floor`, the mosaic's cell
//! bounds, `merge_window` and `stress.ceiling` are all recorded as *unjudged* because materials
//! had no appearance to judge them against. They have one now, and this is where it is looked
//! at.
//!
//! ```text
//! cargo test -p treepo-render --test bark_preview -- --ignored --nocapture
//! ```
//!
//! Writes PNGs to `target/tmp/bark-preview/`. Ignored by default, because a test that writes a
//! megabyte of pictures nobody reads is a slow test with no assertion in it.
//!
//! # Four views, because they answer four different questions
//!
//! * **`families.png`** — the six [`MaterialFamily`]s as long limbs, each carrying a four-holder
//!   mosaic and an age gradient. Answers "does this read as bark, and can I still see who owns
//!   what".
//! * **`readings.png`** — one family, six times, with a single reading changed each time.
//!   Answers "is this reading visible, and is it visible *without* drowning the material" —
//!   which is `F-MAT-6`'s "coexists with" as a picture.
//! * **`grain.png`** — one limb at four times the density. Answers the one a strip cannot: is
//!   this a plate or a stripe.
//! * **`tree.png`** and **`tree-near.png`** — a hand-built fan, framed and then zoomed into.
//!   Answers whether the surface still reads when a limb is forty texels long, and whether the
//!   joints between limbs hold up when it is four hundred.
//!
//! Each strip also writes a `-detail` crop at one-to-one, taken across the mosaic's run
//! boundaries. Shrinking a surface is exactly the operation that makes a bad one look fine, so
//! the judgment is made on those.
//!
//! # The PNG encoder is here rather than as a dependency
//!
//! Same argument `tools/m0-silhouette/src/png.rs` makes at length and for the same reason: a
//! zlib stream may be built from *stored* DEFLATE blocks, a stored block is a length, its
//! complement and the bytes, and that is the whole compressor. The files land in `target/`,
//! they are the raw size, and they are looked at once.

use bevy::prelude::*;
use treepo_det::{Angle, Fx, OrderedMap, Seed};
use treepo_id::Palette;
use treepo_model::{
    AgeGradient, AuthorKey, Composition, FamilyMix, Material, MaterialFamily, MaterialMap, Mosaic,
    NodeId, NodeRole, Point, RepoPath, Segment, Skeleton, Stress,
};
use treepo_render::{Extent, bake};

/// Where the pictures land.
///
/// `CARGO_TARGET_TMPDIR` rather than a relative path: an integration test runs with its
/// *crate* directory as the working directory, so `target/…` would quietly create a second
/// target tree under `crates/treepo-render/`. This one is the real build directory.
const OUT: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/bark-preview");

/// How many texels one world unit is worth in the two close views.
///
/// The near band, deliberately: this is where every octave is in play and where a surface that
/// is merely noise stops being able to hide.
const NEAR: f32 = 1.0;

// ---------------------------------------------------------------------------------------
// The views
// ---------------------------------------------------------------------------------------

#[test]
#[ignore = "writes pictures; run it when the surface changes"]
fn families() {
    let mut scene = Scene::new(1440, 6 * 150);
    for (row, family) in MaterialFamily::ALL.into_iter().enumerate() {
        scene.limb(row, |material| {
            material.family = family;
            material.mosaic = mosaic(&["ada", "brahe", "curie", "dijkstra"], &[9, 5, 7, 3]);
            // A representative span rather than an extreme one: `age_full_scale_days` is ten
            // years, so a well-worked directory in a live repository sits near the middle.
            material.gradient = Some(AgeGradient::new(
                Fx::from_ratio(11, 20),
                Fx::from_ratio(1, 20),
            ));
            material.stress = Stress::new([None, None, Some(Fx::from_ratio(1, 5))]);
        });
    }
    scene.write("families.png");
}

#[test]
#[ignore = "writes pictures; run it when the surface changes"]
fn readings() {
    let mut scene = Scene::new(1440, 6 * 150);
    let dressings: [Dressing; 6] = [
        ("plain heartwood", |_| {}),
        ("two holders", |material| {
            material.mosaic = mosaic(&["ada", "brahe"], &[8, 6]);
        }),
        ("six holders", |material| {
            material.mosaic = mosaic(
                &["ada", "brahe", "curie", "dijkstra", "euler", "fermat"],
                &[7, 2, 5, 9, 3, 6],
            );
        }),
        ("old, unowned", |material| {
            material.gradient = Some(AgeGradient::uniform(Fx::ONE));
        }),
        ("cracked and blended", |material| {
            material.composition = Composition::Blended {
                secondary: MaterialFamily::Ore,
                weight: Fx::from_ratio(1, 5),
            };
            material.stress = Stress::new([Some(Fx::from_ratio(4, 5)), None, None]);
        }),
        ("a container's inventory", |material| {
            let mut shares = [Fx::ZERO; MaterialFamily::ALL.len()];
            shares[MaterialFamily::Parchment.position()] = Fx::from_ratio(1, 2);
            shares[MaterialFamily::Ore.position()] = Fx::from_ratio(3, 10);
            shares[MaterialFamily::Resin.position()] = Fx::from_ratio(2, 10);
            material.composition = Composition::Subordinate(FamilyMix::new(shares));
        }),
    ];
    for (row, (label, dress)) in dressings.into_iter().enumerate() {
        println!("  row {row}: {label}");
        scene.limb(row, dress);
    }
    scene.write("readings.png");
}

/// One limb, close enough to see what it is made of.
///
/// The other two views are strips: a limb is a hundred texels tall and the surface has to be
/// judged through that. This one is the same shader at four times the density, which is the
/// only way to tell a plate from a stripe — and the near LOD band a user reaches by zooming in
/// on one directory is much closer to this than to the strips.
#[test]
#[ignore = "writes pictures; run it when the surface changes"]
fn grain() {
    let mut scene = Scene::new(1200, 2 * ROW);
    scene.thickness = 250.0;
    scene.limb(0, |material| {
        material.mosaic = mosaic(&["ada", "brahe", "curie"], &[5, 4, 6]);
        material.gradient = Some(AgeGradient::new(Fx::from_ratio(4, 5), Fx::ZERO));
        material.stress = Stress::new([None, None, Some(Fx::from_ratio(1, 5))]);
    });
    scene.write("grain.png");
}

/// The whole-tree read, at the density a framed tree is actually baked at.
///
/// A hand-built fan rather than a grown skeleton: `treepo-gen` needs a manifest, a manifest
/// needs `treepo-vcs`, and none of that would change the question this view asks.
#[test]
#[ignore = "writes pictures; run it when the surface changes"]
fn tree() {
    let mut skeleton = Skeleton::new();
    let mut materials = MaterialMap::new();
    let mut segments: Vec<Segment> = Vec::new();

    grow(
        &mut skeleton,
        &mut materials,
        &mut segments,
        Branch {
            parent: None,
            path: RepoPath::root(),
            from: Vec2::new(700.0, 60.0),
            heading: 0.0,
            length: 260.0,
            width: 78.0,
            depth: 0,
        },
    );
    skeleton.extend_segments(segments);

    let indices: Vec<u32> = (0..skeleton.segments().len() as u32).collect();
    let layer = bake::rasterize(
        &skeleton,
        &materials,
        &Palette::built_in(),
        &indices,
        Extent {
            min: Vec2::ZERO,
            max: Vec2::new(1400.0, 1000.0),
        },
        UVec2::new(1400, 1000),
    );
    write_png(
        &format!("{OUT}/tree.png"),
        layer.size,
        &backed(&layer.color, layer.size),
    );

    // And the same tree at the band a user reaches by zooming into one bough — which is where
    // `AC-NAV-2`'s gesture ends and where the surface has to hold up.
    let near = bake::rasterize(
        &skeleton,
        &materials,
        &Palette::built_in(),
        &indices,
        Extent {
            min: Vec2::new(520.0, 240.0),
            max: Vec2::new(940.0, 555.0),
        },
        UVec2::new(1260, 945),
    );
    write_png(
        &format!("{OUT}/tree-near.png"),
        near.size,
        &backed(&near.color, near.size),
    );
    println!(
        "  {} nodes, {} segments",
        skeleton.nodes().len(),
        indices.len()
    );
}

// ---------------------------------------------------------------------------------------
// A row of limbs
// ---------------------------------------------------------------------------------------

/// One row of `readings.png`: what it is called, and the one reading it turns on.
type Dressing = (&'static str, fn(&mut Material));

/// How tall one limb's row is, in texels.
const ROW: u32 = 150;

/// A strip of horizontal limbs, one per row, baked in a single pass.
struct Scene {
    skeleton: Skeleton,
    materials: MaterialMap,
    segments: Vec<Segment>,
    size: UVec2,
    /// How wide a limb is drawn at its base, in world units — and therefore, since the region
    /// maps one world unit to one texel, how many texels the surface has across a half-width.
    thickness: f32,
}

impl Scene {
    fn new(width: u32, height: u32) -> Self {
        Self {
            skeleton: Skeleton::new(),
            materials: MaterialMap::new(),
            segments: Vec::new(),
            size: UVec2::new(width, height),
            thickness: 108.0,
        }
    }

    /// One limb across the given row, built from four segments so the span table is exercised.
    fn limb(&mut self, row: usize, dress: impl FnOnce(&mut Material)) {
        let node = NodeId::new(self.skeleton.nodes().len() as u32);
        let name = format!("src/limb{row}/surface.rs");
        self.skeleton.push_node(
            None,
            Point::ORIGIN,
            Angle::ZERO,
            Seed::root(b"bark-preview"),
            NodeRole::Limb {
                path: RepoPath::new(name.as_bytes()).expect("a well-formed path"),
            },
        );

        let mut material = Material {
            family: MaterialFamily::Heartwood,
            composition: Composition::Pure,
            budget: Fx::from_ratio(1, 2),
            mosaic: Mosaic::new(OrderedMap::new(), 0),
            gradient: None,
            stress: None,
        };
        dress(&mut material);
        self.materials.push(material);

        // World `y` runs up and row zero is the top, so the first row drawn is the last one
        // written. Mirrored here so the caller counts rows the way the picture reads.
        let centre = (self.size.y - row as u32 * ROW - ROW / 2) as f32;
        let (left, right) = (40.0f32, self.size.x as f32 - 40.0);
        let (base, tip) = (self.thickness, self.thickness * 0.41);
        for step in 0..4 {
            let (a, b) = (step as f32 / 4.0, (step + 1) as f32 / 4.0);
            self.segments.push(Segment {
                start: point(left + (right - left) * a, centre),
                end: point(left + (right - left) * b, centre),
                base_width: fx(base + (tip - base) * a),
                tip_width: fx(base + (tip - base) * b),
                node,
                generation: 0,
            });
        }
    }

    fn write(mut self, name: &str) {
        self.skeleton.extend_segments(self.segments);
        let indices: Vec<u32> = (0..self.skeleton.segments().len() as u32).collect();
        let layer = bake::rasterize(
            &self.skeleton,
            &self.materials,
            &Palette::built_in(),
            &indices,
            Extent {
                min: Vec2::ZERO,
                max: self.size.as_vec2() / NEAR,
            },
            self.size,
        );
        let picture = backed(&layer.color, layer.size);
        write_png(&format!("{OUT}/{name}"), layer.size, &picture);

        // The same pixels at one-to-one over the first two rows. A whole strip has to be shrunk
        // to be looked at, and shrinking a surface is exactly the operation that makes a bad one
        // look fine — so the judgment is made on this one. Taken from the middle of the limb
        // rather than its base, because that is where the mosaic's run boundaries are and the
        // boundaries are the half of this that a texture cannot fake.
        let detail = UVec2::new(760.min(layer.size.x), (2 * ROW).min(layer.size.y));
        write_png(
            &format!("{OUT}/{}-detail.png", name.trim_end_matches(".png")),
            detail,
            &crop(&picture, layer.size, UVec2::new(430, 0), detail),
        );
    }
}

/// A branch waiting to be grown, and its children after it.
struct Branch {
    parent: Option<NodeId>,
    path: RepoPath,
    from: Vec2,
    heading: f32,
    length: f32,
    width: f32,
    depth: u32,
}

/// Grows one branch and recurses, giving every node a material that varies with its depth.
fn grow(
    skeleton: &mut Skeleton,
    materials: &mut MaterialMap,
    segments: &mut Vec<Segment>,
    branch: Branch,
) {
    if branch.depth > 4 || branch.width < 1.0 {
        return;
    }
    let node = NodeId::new(skeleton.nodes().len() as u32);
    skeleton.push_node(
        branch.parent,
        Point::ORIGIN,
        Angle::ZERO,
        Seed::root(b"bark-preview"),
        NodeRole::Limb {
            path: branch.path.clone(),
        },
    );

    // What a code repository actually grows: heartwood boughs, ore and resin where the build
    // and the config live, parchment out at the documentation twigs. Picking families off the
    // declaration order instead put `Parchment` — the palest of the six — on half the limbs,
    // which made the whole tree read as bleached when the shader was not at fault.
    let family = [
        MaterialFamily::Heartwood,
        MaterialFamily::Heartwood,
        MaterialFamily::Ore,
        MaterialFamily::Resin,
        MaterialFamily::Parchment,
    ][branch.depth as usize % 5];
    let holders: &[&str] = &["ada", "brahe", "curie", "dijkstra", "euler"];
    let take = 2 + branch.depth as usize % 3;
    materials.push(Material {
        family,
        composition: Composition::Pure,
        budget: Fx::from_ratio(1, 2),
        mosaic: mosaic(&holders[..take], &[7, 3, 5, 9, 2][..take]),
        // Old at the base, vital at the tip, and shallower with every generation — the shape
        // `F-MAT-4` describes, at spans a live repository actually produces on a ten-year scale.
        gradient: Some(AgeGradient::new(
            Fx::from_ratio(8 - i64::from(branch.depth), 10),
            Fx::from_ratio(4 - i64::from(branch.depth).min(3), 10),
        )),
        stress: Stress::new([None, None, Some(Fx::from_ratio(1, 6))]),
    });

    // Four segments per branch, curving gently, so the limb bends the way a grown one does.
    let mut at = branch.from;
    let mut heading = branch.heading;
    let curve = if branch.depth.is_multiple_of(2) {
        0.05
    } else {
        -0.07
    };
    for step in 0..4 {
        let (a, b) = (step as f32 / 4.0, (step + 1) as f32 / 4.0);
        heading += curve;
        let next = at + Vec2::new(heading.sin(), heading.cos()) * (branch.length / 4.0);
        segments.push(Segment {
            start: point(at.x, at.y),
            end: point(next.x, next.y),
            base_width: fx(branch.width * (1.0 - 0.45 * a)),
            tip_width: fx(branch.width * (1.0 - 0.45 * b)),
            node,
            generation: branch.depth as u8,
        });
        at = next;
    }

    for child in 0..3 {
        let spread = (child as f32 - 1.0) * 0.62;
        grow(
            skeleton,
            materials,
            segments,
            Branch {
                parent: Some(node),
                path: branch
                    .path
                    .join(format!("b{child}").as_bytes())
                    .expect("a well-formed component"),
                from: at,
                heading: heading + spread,
                length: branch.length * 0.66,
                width: branch.width * 0.52,
                depth: branch.depth + 1,
            },
        );
    }
}

// ---------------------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------------------

fn point(x: f32, y: f32) -> Point {
    Point::new(fx(x), fx(y))
}

/// A world coordinate as the fixed-point the model speaks.
fn fx(value: f32) -> Fx {
    Fx::from_f64(f64::from(value))
}

/// A mosaic of named holders and their cell counts.
fn mosaic(names: &[&str], cells: &[u32]) -> Mosaic {
    let mut held = OrderedMap::new();
    for (name, count) in names.iter().zip(cells) {
        held.insert(
            AuthorKey::from_email(format!("{name}@example.invalid").as_bytes()),
            *count,
        );
    }
    let total = cells.iter().sum();
    Mosaic::new(held, total)
}

/// The layer's RGBA over a dark backing, as the RGB triples a PNG wants.
///
/// A baked layer is transparent where nothing was drawn, and a viewer that composites it over
/// white shows the tree as a silhouette in a glare. The backing is the app's own dark field.
fn backed(rgba: &[u8], size: UVec2) -> Vec<u8> {
    let mut out = Vec::with_capacity(size.x as usize * size.y as usize * 3);
    for texel in rgba.chunks_exact(4) {
        let alpha = u32::from(texel[3]);
        for channel in texel.iter().take(3) {
            let over = u32::from(*channel) * alpha;
            let under = 14 * (255 - alpha);
            out.push(((over + under) / 255) as u8);
        }
    }
    out
}

/// A rectangle of an RGB picture, clamped to what is there.
fn crop(rgb: &[u8], size: UVec2, at: UVec2, want: UVec2) -> Vec<u8> {
    let mut out = Vec::with_capacity(want.x as usize * want.y as usize * 3);
    for row in 0..want.y {
        for column in 0..want.x {
            let x = (at.x + column).min(size.x - 1) as usize;
            let y = (at.y + row).min(size.y - 1) as usize;
            let from = (y * size.x as usize + x) * 3;
            out.extend_from_slice(&rgb[from..from + 3]);
        }
    }
    out
}

// --- the encoder -----------------------------------------------------------------------

fn write_png(path: &str, size: UVec2, rgb: &[u8]) {
    let (width, height) = (size.x, size.y);
    assert_eq!(
        rgb.len(),
        width as usize * height as usize * 3,
        "the picture is not the size it says it is"
    );

    // PNG rows are prefixed with a filter byte; zero is "none", which is what an uncompressed
    // stream wants anyway.
    let mut raw = Vec::with_capacity(rgb.len() + height as usize);
    for row in 0..height as usize {
        raw.push(0u8);
        let from = row * width as usize * 3;
        raw.extend_from_slice(&rgb[from..from + width as usize * 3]);
    }

    let mut out = Vec::from([0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, truecolour, no interlace
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);

    std::fs::create_dir_all(OUT).expect("target/ is writable");
    std::fs::write(path, &out).expect("the picture is writable");
    println!(
        "  wrote {path} ({width}×{height}, {} KiB)",
        out.len() / 1024
    );
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let mut crc = crc32(kind);
    for byte in body {
        crc = (crc >> 8) ^ CRC_TABLE[((crc ^ u32::from(*byte)) & 0xff) as usize];
    }
    out.extend_from_slice(&(!crc).to_be_bytes());
}

/// A zlib stream of stored — uncompressed — DEFLATE blocks. RFC 1950 §2, RFC 1951 §3.2.4.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::from([0x78u8, 0x01]);
    let mut rest = raw;
    while !rest.is_empty() {
        let take = rest.len().min(0xFFFF);
        let (block, next) = rest.split_at(take);
        out.push(u8::from(next.is_empty()));
        out.extend_from_slice(&(take as u16).to_le_bytes());
        out.extend_from_slice(&(!(take as u16)).to_le_bytes());
        out.extend_from_slice(block);
        rest = next;
    }
    if raw.is_empty() {
        out.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for byte in raw {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in bytes {
        crc = (crc >> 8) ^ CRC_TABLE[((crc ^ u32::from(*byte)) & 0xff) as usize];
    }
    crc
}

const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0usize;
    while n < 256 {
        let mut c = n as u32;
        let mut bit = 0;
        while bit < 8 {
            c = if c & 1 == 0 {
                c >> 1
            } else {
                0xEDB8_8320 ^ (c >> 1)
            };
            bit += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
};

// Draws the app icon (a jade ring on a dark rounded card) at every size the
// installers need, and writes the PNG / ICO / ICNS files into icons/.
//
// Run from the app folder:
//   cargo run --manifest-path tools/make_icon/Cargo.toml
//
// Kept in its own crate with its own [workspace], so it never affects the
// shipped binary.

use std::fs;

/// How many samples per axis inside one output pixel. Averaging these is what
/// gives clean edges at small sizes.
const SUBSAMPLES: usize = 4;

/// The card and ring, measured as fractions of the icon (so it is the same
/// drawing at 16 pixels and at 1024). Returns premultiplied RGBA in 0..=1.
fn sample(u: f32, v: f32) -> [f32; 4] {
    // Rounded square: signed distance to a square with corner radius 0.22.
    let radius = 0.22f32;
    let inset = 0.5 - radius;
    let dx = (u - 0.5).abs() - inset;
    let dy = (v - 0.5).abs() - inset;
    let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt()
        + dx.max(dy).min(0.0)
        - radius;
    if outside > 0.0 {
        return [0.0, 0.0, 0.0, 0.0];
    }

    // Card: vertical gradient #2b2140 -> #150f1e.
    let card = [
        (43.0 - 22.0 * v) / 255.0,
        (33.0 - 18.0 * v) / 255.0,
        (64.0 - 34.0 * v) / 255.0,
    ];
    let mut out = [card[0], card[1], card[2], 1.0];

    // Jade ring, amber at the top shading to green at the bottom.
    let r = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt();
    if r <= 0.395 && r >= 0.245 {
        let ring = [
            (236.0 - 40.0 * v) / 255.0,
            (186.0 - 20.0 * v) / 255.0,
            (96.0 + 30.0 * v) / 255.0,
        ];
        for i in 0..3 {
            out[i] = ring[i] + out[i] * 0.0;
        }
        out[3] = 1.0;
        return out;
    }

    // Soft warm glow in the middle, so the centre isn't a flat hole.
    let glow = (1.0 - r / 0.30).clamp(0.0, 1.0) * 0.22;
    if glow > 0.0 {
        let g = [224.0 / 255.0, 150.0 / 255.0, 90.0 / 255.0];
        for i in 0..3 {
            out[i] = g[i] * glow + out[i] * (1.0 - glow);
        }
    }
    out
}

/// Draw the icon at `n` by `n` pixels, premultiplied RGBA.
fn render(n: usize) -> Vec<[u8; 4]> {
    let mut px = vec![[0u8; 4]; n * n];
    let mut row = vec![[0u8; 4]; n];
    for y in 0..n {
        for x in 0..n {
            let mut acc = [0.0f32; 4];
            for sy in 0..SUBSAMPLES {
                for sx in 0..SUBSAMPLES {
                    let u = (x as f32 + (sx as f32 + 0.5) / SUBSAMPLES as f32) / n as f32;
                    let v = (y as f32 + (sy as f32 + 0.5) / SUBSAMPLES as f32) / n as f32;
                    let s = sample(u, v);
                    for i in 0..4 {
                        acc[i] += s[i];
                    }
                }
            }
            let total = (SUBSAMPLES * SUBSAMPLES) as f32;
            let a = acc[3] / total;
            // Stored premultiplied, so undo it to get the usual straight-alpha PNG.
            let mut out = [0u8; 4];
            for i in 0..3 {
                let straight = if a > 0.0 { acc[i] / total / a } else { 0.0 };
                out[i] = (straight.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            out[3] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
            row[x] = out;
        }
        px[y * n..(y + 1) * n].copy_from_slice(&row);
    }
    px
}

fn encode_png(n: usize) -> Vec<u8> {
    let px = render(n);
    let mut bytes: Vec<u8> = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut bytes, n as u32, n as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().expect("png header");
        let mut raw = Vec::with_capacity(n * n * 4);
        for p in &px {
            raw.extend_from_slice(p);
        }
        w.write_image_data(&raw).expect("png data");
    }
    bytes
}

/// A Windows .ico holding several sizes, each stored as a PNG (Vista and later).
fn encode_ico(sizes: &[usize]) -> Vec<u8> {
    let images: Vec<(usize, Vec<u8>)> = sizes.iter().map(|&s| (s, encode_png(s))).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());

    // Each directory entry is 16 bytes; the pictures follow all of them.
    let mut offset = 6 + 16 * images.len();
    for (size, png) in &images {
        // 256 is written as 0 in this one-byte field.
        out.push(if *size >= 256 { 0 } else { *size as u8 });
        out.push(if *size >= 256 { 0 } else { *size as u8 });
        out.push(0); // palette colours
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in &images {
        out.extend_from_slice(png);
    }
    out
}

/// A macOS .icns holding PNG pictures, as modern macOS expects.
fn encode_icns(entries: &[(&[u8; 4], usize)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (kind, size) in entries {
        let png = encode_png(*size);
        body.extend_from_slice(*kind);
        // Chunk length counts its own 8-byte header.
        body.extend_from_slice(&((png.len() + 8) as u32).to_be_bytes());
        body.extend_from_slice(&png);
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

fn main() {
    fs::create_dir_all("icons").expect("icons dir");

    // The plain PNG used by Linux packages and as the master picture.
    let master = encode_png(512);
    fs::write("icons/icon.png", &master).expect("write icon.png");
    println!("icons/icon.png   512x512  {} bytes", master.len());

    // The separate sizes Tauri's own icon layout uses.
    for (name, size) in [
        ("32x32.png", 32usize),
        ("128x128.png", 128),
        ("128x128@2x.png", 256),
    ] {
        let bytes = encode_png(size);
        fs::write(format!("icons/{name}"), &bytes).expect("write sized png");
        println!("icons/{name:<16} {size}x{size}  {} bytes", bytes.len());
    }

    // Windows: one .ico carrying the sizes Explorer and the installers pick from.
    let ico = encode_ico(&[16, 32, 48, 64, 128, 256]);
    fs::write("icons/icon.ico", &ico).expect("write icon.ico");
    println!("icons/icon.ico   6 sizes  {} bytes", ico.len());

    // macOS: the .icns the .dmg and the .app bundle need.
    let icns = encode_icns(&[
        (b"icp4", 16),
        (b"icp5", 32),
        (b"icp6", 64),
        (b"ic07", 128),
        (b"ic08", 256),
        (b"ic09", 512),
        (b"ic10", 1024),
        (b"ic11", 32),
        (b"ic12", 64),
        (b"ic13", 256),
        (b"ic14", 512),
    ]);
    fs::write("icons/icon.icns", &icns).expect("write icon.icns");
    println!("icons/icon.icns  11 sizes  {} bytes", icns.len());
}
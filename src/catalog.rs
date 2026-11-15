//! What the Preferences window reads (in <config>/catalog/):
//!   catalog.json  breeds, hats, file types and sprite-sheet geometry
//!   ducks.png / hats.png  the sprite sheets, for live previews
//!   items.json    the desktop files and the duck each one currently is
//! The window writes only preferences.json; the pond applies it live.

use crate::items::{config_dir, Kind};
use crate::scene::Duck;
use crate::sprite_meta::{BREEDS, BREED_NAMES, CELL_H, CELL_W, HATS, HAT_CELL_W};
use serde_json::json;
use std::path::PathBuf;

pub fn dir() -> PathBuf {
    let d = config_dir().join("catalog");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn hat_label(id: &str) -> &'static str {
    match id {
        "straw" => "Straw sun hat",
        "beret" => "Painter's beret",
        "beret_navy" => "Navy beret",
        "mortar" => "Graduation cap",
        "propeller" => "Propeller beanie",
        "party" => "Party hat",
        "cap" => "Baseball cap",
        "crown" => "Tiny crown",
        "flower" => "Flower",
        "bow" => "Bow",
        "glasses" => "Reading glasses",
        "headphones" => "Headphones",
        _ => "Hat",
    }
}

fn write(path: PathBuf, bytes: &[u8]) {
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, bytes).is_ok() {
        let _ = std::fs::rename(tmp, path);
    }
}

/// Breeds, hats, kinds and the sprite sheets. Written once at startup.
pub fn write_static(ducks_png: &[u8], hats_png: &[u8]) {
    let d = dir();
    write(d.join("ducks.png"), ducks_png);
    write(d.join("hats.png"), hats_png);
    let breeds: Vec<_> = BREEDS
        .iter()
        .enumerate()
        .map(|(i, b)| json!({ "id": b, "name": BREED_NAMES[i], "row": i * 2, "baby_row": i * 2 + 1 }))
        .collect();
    let hats: Vec<_> = HATS
        .iter()
        .enumerate()
        .map(|(i, h)| json!({ "id": h.name, "name": hat_label(h.name), "x": i as u32 * HAT_CELL_W, "w": h.w, "h": h.h }))
        .collect();
    let kinds: Vec<_> = Kind::ALL.iter().map(|k| json!({ "id": k.key(), "name": k.label() })).collect();
    let cat = json!({
        "version": 1,
        "cell": [CELL_W, CELL_H],
        "breeds": breeds,
        "hats": hats,
        "kinds": kinds,
    });
    if let Ok(b) = serde_json::to_vec_pretty(&cat) {
        write(d.join("catalog.json"), &b);
    }
}

/// The current flock: one entry per desktop file, sorted by name.
pub fn write_items(ducks: &[Duck]) {
    let mut rows: Vec<_> = ducks
        .iter()
        .map(|d| {
            json!({
                "key": d.key,
                "name": d.name,
                "kind": d.kind.key(),
                "breed": BREEDS[d.breed.min(BREEDS.len() - 1)],
                "baby": d.baby,
                "hat": d.hat.map(|h| HATS[h].name),
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        a["name"].as_str().unwrap_or("").to_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
    });
    if let Ok(b) = serde_json::to_vec(&json!({ "items": rows })) {
        write(dir().join("items.json"), &b);
    }
}

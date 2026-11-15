//! The pond: background, ducks, bread, ripples. Pure simulation + software
//! rendering into a small RGBA framebuffer (art pixels); the GPU only scales it.

use crate::font;
use crate::items::Kind;
use crate::sprite_meta::*;

pub struct Img {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Img {
    pub fn decode(bytes: &[u8]) -> Img {
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut r = dec.read_info().expect("png header");
        let mut buf = vec![0; r.output_buffer_size()];
        let info = r.next_frame(&mut buf).expect("png data");
        let (w, h) = (info.width, info.height);
        let n = (w * h) as usize;
        let mut px = Vec::with_capacity(n * 4);
        match info.color_type {
            png::ColorType::Rgba => px.extend_from_slice(&buf[..n * 4]),
            png::ColorType::Rgb => buf[..n * 3]
                .chunks(3)
                .for_each(|c| px.extend_from_slice(&[c[0], c[1], c[2], 255])),
            png::ColorType::GrayscaleAlpha => buf[..n * 2]
                .chunks(2)
                .for_each(|c| px.extend_from_slice(&[c[0], c[0], c[0], c[1]])),
            _ => buf[..n]
                .iter()
                .for_each(|&g| px.extend_from_slice(&[g, g, g, 255])),
        }
        Img { w, h, px }
    }
}

pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

pub fn hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum St {
    Idle,
    Wander(f32, f32),
    Seek(u32),
    Eat(u32),
    /// queued behind the eater of this crumb
    Wait(u32),
    /// a duckling trailing its parent
    Follow,
    Home,
    Sleep,
    Held,
}

/// Something the app layer should do in response to the simulation.
pub enum Sound {
    Quack(u8), // 0 low, 1 mid, 2 high
    Peep,
}

pub struct Duck {
    pub key: String,
    pub name: String,
    pub baby: bool,
    pub kind: Kind,
    pub breed: usize,
    pub hat: Option<usize>,
    pub x: f32,
    pub y: f32,
    pub home: (f32, f32),
    right: bool,
    st: St,
    timer: f32,
    anim: f32,
    quack: f32,
    blink: f32,
    next_blink: f32,
    pace: f32,
    hatch: f32,
    was_land: bool,
    voice: u8,
    vx: f32,
    vy: f32,
    /// -1 / +1: which side of its crumb this duck eats from
    side: f32,
    /// place in this crumb's line: 0 eats, 1.. wait their turn
    slot: u8,
    /// heading in 8 compass steps: 0 E, 1 SE, 2 S, 3 SW, 4 W, 5 NW, 6 N, 7 NE
    dir: u8,
    cand: u8,
    cand_t: f32,
    /// time until this duck's next wake / idle ripple
    wake: f32,
    /// ducklings: index of the adult they follow, and their place in line
    parent: Option<usize>,
    rank: u8,
    /// seconds a duckling stays at its own spot after being sent home
    stay: f32,
    /// thought bubble: seconds left showing, and what it's about
    think: f32,
    idea: u8,
    next_think: f32,
    /// happy hop after a click / shake after climbing out of the water
    hop: f32,
    shake: f32,
}

struct Crumb {
    id: u32,
    x: f32,
    y: f32,
    z: f32,
    vz: f32,
    bites: u8,
    age: f32,
    landed: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum PKind {
    Heart,
    Zed,
    Crumb,
    Drop,
    Sparkle,
}

struct Particle {
    kind: PKind,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max: f32,
}

struct Petal {
    x: f32,
    y: f32,
    vx: f32,
    spin: f32,
    life: f32,
}

struct Ripple {
    x: f32,
    y: f32,
    r: f32,
    life: f32,
}

pub struct Scene {
    pub w: u32,
    pub h: u32,
    bg: Vec<u8>,
    /// bg with the time-of-day grade applied
    bg_tint: Vec<u8>,
    tint: ([f32; 3], [f32; 3]),
    /// 0 = full day, 1 = deep night
    pub night: f32,
    fireflies: Vec<(f32, f32, f32)>,
    land: Vec<bool>,
    glints: Vec<(u32, f32)>,
    bg_k: u32,
    pub ducks: Vec<Duck>,
    crumbs: Vec<Crumb>,
    ripples: Vec<Ripple>,
    particles: Vec<Particle>,
    petals: Vec<Petal>,
    next_petal: f32,
    next_id: u32,
    rng: Rng,
    t: f32,
    pub hover: Option<usize>,
    pub selected: Option<usize>,
    pub show_labels: bool,
    pub show_hats: bool,
    /// hover/selected tags are drawn by the hi-res overlay instead
    pub hires_tags: bool,
    sheet: Img,
    hats: Img,
    order: Vec<usize>,
    /// per (row, frame): lowest opaque row and its x-extent, for waterlines
    extents: Vec<Vec<(i32, i32, i32)>>,
    pending_food: Vec<(f32, f32, f32)>,
    pub sounds: Vec<Sound>,
    sep: Vec<(f32, f32)>,
    families_dirty: bool,
    dragonfly: Option<Dragonfly>,
    next_dragonfly: f32,
    next_bloop: f32,
}

struct Dragonfly {
    x: f32,
    y: f32,
    tx: f32,
    ty: f32,
    hover: f32,
    darts: u8,
    t: f32,
}

/// Each painting pixel covers BG_SCALE x BG_SCALE art pixels (chunkier Monet).
pub const BG_SCALE: u32 = 2;
/// Keep duck heads clear of the menu bar / top edge (art px).
const TOP_MARGIN: f32 = 8.0;
const ROAM: f32 = 26.0; // how far (art px) a duck wanders from home
const SLOW: f32 = 11.0;
const EAGER: f32 = 30.0;
const SCURRY: f32 = 85.0;

impl Scene {
    pub fn new(w: u32, h: u32, pond: &Img, landmask: &Img, sheet: Img, hats: Img) -> Scene {
        let mut s = Scene {
            w,
            h,
            bg: vec![],
            bg_tint: vec![],
            tint: ([1.0; 3], [0.0; 3]),
            night: 0.0,
            fireflies: vec![],
            land: vec![],
            glints: vec![],
            bg_k: 1,
            ducks: vec![],
            crumbs: vec![],
            ripples: vec![],
            particles: vec![],
            petals: vec![],
            next_petal: 8.0,
            next_id: 1,
            rng: Rng::new(0x9e3779b97f4a7c15 ^ (w as u64 * 31 + h as u64)),
            t: 0.0,
            hover: None,
            selected: None,
            show_labels: false,
            show_hats: false,
            hires_tags: false,
            sheet,
            hats,
            order: vec![],
            extents: vec![],
            pending_food: vec![],
            sounds: vec![],
            sep: vec![],
            families_dirty: true,
            dragonfly: None,
            next_dragonfly: 25.0,
            next_bloop: 4.0,
        };
        s.build_background(pond, landmask);
        s.apply_tint();
        s.extents = sprite_extents(&s.sheet);
        s
    }

    /// Crop the pre-pixelated Monet to the screen's art size. Painting pixels
    /// are only ever repeated a whole number of times (k x k), so the painting
    /// stays crisp and its pixel grid lines up with the ducks'.
    fn build_background(&mut self, pond: &Img, landmask: &Img) {
        let (w, h) = (self.w, self.h);
        let k = ((w + pond.w - 1) / pond.w)
            .max((h + pond.h - 1) / pond.h)
            .max(BG_SCALE);
        self.bg_k = k;
        let used_w = (w + k - 1) / k;
        let used_h = (h + k - 1) / k;
        let ox = pond.w.saturating_sub(used_w) / 2;
        let oy = (pond.h.saturating_sub(used_h) as f32 * 0.55) as u32;
        self.bg = vec![0; (w * h * 4) as usize];
        self.land = vec![false; (w * h) as usize];
        self.glints.clear();
        let mut rng = Rng::new(7);
        for y in 0..h {
            for x in 0..w {
                let sx = (ox + x / k).min(pond.w - 1);
                let sy = (oy + y / k).min(pond.h - 1);
                let si = ((sy * pond.w + sx) * 4) as usize;
                let di = ((y * w + x) * 4) as usize;
                self.bg[di..di + 4].copy_from_slice(&pond.px[si..si + 4]);
                self.bg[di + 3] = 255;
                let li = ((sy * landmask.w + sx) * 4) as usize;
                let is_land = landmask.px[li] > 127;
                self.land[(y * w + x) as usize] = is_land;
                let p = &pond.px[si..si + 3];
                let lum = p[0] as u32 + p[1] as u32 + p[2] as u32;
                if !is_land && lum > 330 && x % k == 0 && y % k == 0 && rng.f() < 0.08 {
                    self.glints.push(((y * w + x), rng.range(0.0, 6.28)));
                }
            }
        }
    }

    pub fn is_land(&self, x: f32, y: f32) -> bool {
        let xi = (x.max(0.0) as u32).min(self.w - 1);
        let yi = (y.max(0.0) as u32).min(self.h - 1);
        self.land[(yi * self.w + xi) as usize]
    }

    // ------------------------------------------------------------------ ducks

    pub fn add_duck(
        &mut self,
        key: String,
        name: String,
        kind: Kind,
        baby: bool,
        breed: usize,
        hat: Option<usize>,
        home: (f32, f32),
        entrance_delay: f32,
    ) {
        let h = hash(&name);
        let breed = breed.min(BREEDS.len() - 1);
        let home = self.clamp(home.0, home.1);
        let voice = ((h >> 20) % 3) as u8;
        let d = Duck {
            key,
            name,
            baby,
            kind,
            breed,
            hat,
            x: home.0,
            y: home.1,
            home,
            right: (h >> 4) & 1 == 0,
            st: St::Idle,
            timer: self.rng.range(1.0, 6.0),
            anim: self.rng.range(0.0, 10.0),
            quack: 0.0,
            blink: 0.0,
            next_blink: self.rng.range(1.0, 5.0),
            pace: if baby {
                self.rng.range(0.9, 1.25)
            } else {
                self.rng.range(0.7, 1.1)
            },
            hatch: -entrance_delay,
            was_land: true,
            voice,
            vx: 0.0,
            vy: 0.0,
            side: 1.0,
            slot: 0,
            dir: if (h >> 4) & 1 == 0 { 0 } else { 4 },
            cand: 0,
            cand_t: 0.0,
            wake: (h % 300) as f32 / 100.0,
            parent: None,
            rank: 0,
            stay: 0.0,
            think: 0.0,
            idea: 0,
            next_think: 20.0 + ((h >> 12) % 9000) as f32 / 100.0,
            hop: 0.0,
            shake: 0.0,
        };
        let was_land = self.is_land(d.x, d.y);
        self.ducks.push(Duck { was_land, ..d });
        self.families_dirty = true;
    }

    pub fn remove_duck(&mut self, key: &str) {
        if let Some(i) = self.ducks.iter().position(|d| d.key == key) {
            self.ducks.remove(i);
            self.families_dirty = true;
            self.hover = None;
            self.selected = match self.selected {
                Some(s) if s == i => None,
                Some(s) if s > i => Some(s - 1),
                s => s,
            };
        }
    }

    pub fn find(&self, key: &str) -> Option<usize> {
        self.ducks.iter().position(|d| d.key == key)
    }

    fn clamp(&self, x: f32, y: f32) -> (f32, f32) {
        (
            x.clamp(9.0, self.w as f32 - 10.0),
            y.clamp(CELL_H as f32 + TOP_MARGIN, self.h as f32 - 3.0),
        )
    }

    /// A free spot for a new desktop item, Finder-style: right column first.
    pub fn free_slot(&self) -> (f32, f32) {
        let step = 26.0;
        let mut x = self.w as f32 - 16.0;
        while x > 10.0 {
            let mut y = 24.0;
            while y < self.h as f32 - 6.0 {
                if self
                    .ducks
                    .iter()
                    .all(|d| (d.home.0 - x).hypot(d.home.1 - y) > 13.0)
                {
                    return (x, y);
                }
                y += step;
            }
            x -= step;
        }
        (self.w as f32 / 2.0, self.h as f32 / 2.0)
    }

    /// Desktop icons can sit almost on top of each other (macOS stacks new
    /// files in the top-right corner). Nudge homes apart until every duck has
    /// room, moving each as little as possible. Ducks waddle to the new spot.
    pub fn relax_homes(&mut self) {
        const D: f32 = 12.0;
        let n = self.ducks.len();
        let mut homes: Vec<(f32, f32)> = self.ducks.iter().map(|d| d.home).collect();
        for _ in 0..40 {
            let mut moved = false;
            for a in 0..n {
                for b in a + 1..n {
                    let (dx, dy) = (homes[a].0 - homes[b].0, (homes[a].1 - homes[b].1) * 1.4);
                    let d = dx.hypot(dy);
                    if d < D {
                        let (ux, uy) = if d > 0.01 {
                            (dx / d, dy / d)
                        } else {
                            let t = (a * 7 + b * 13) as f32;
                            (t.cos(), t.sin())
                        };
                        let push = (D - d) * 0.5;
                        homes[a].0 += ux * push;
                        homes[a].1 += uy * push / 1.4;
                        homes[b].0 -= ux * push;
                        homes[b].1 -= uy * push / 1.4;
                        moved = true;
                    }
                }
            }
            for h in homes.iter_mut() {
                *h = self.clamp(h.0, h.1);
            }
            if !moved {
                break;
            }
        }
        for (d, h) in self.ducks.iter_mut().zip(homes) {
            if (d.home.0 - h.0).hypot(d.home.1 - h.1) > 0.5 {
                d.home = h;
                if matches!(d.st, St::Idle | St::Sleep) {
                    d.st = St::Home;
                }
            }
        }
    }

    /// Every duckling adopts the nearest adult (its own breed preferred);
    /// each adult takes at most four. Called whenever the flock changes.
    pub fn mark_families_dirty(&mut self) {
        self.families_dirty = true;
    }

    fn rebuild_families(&mut self) {
        self.families_dirty = false;
        let n = self.ducks.len();
        let mut kids = vec![0u8; n];
        for d in &mut self.ducks {
            d.parent = None;
        }
        let mut babies: Vec<usize> = (0..n).filter(|&i| self.ducks[i].baby).collect();
        babies.sort_by_key(|&i| hash(&self.ducks[i].name));
        for b in babies {
            let (bx, by, bb) = (
                self.ducks[b].home.0,
                self.ducks[b].home.1,
                self.ducks[b].breed,
            );
            let mut best: Option<(f32, usize)> = None;
            for (a, ad) in self.ducks.iter().enumerate() {
                if ad.baby || kids[a] >= 6 {
                    continue;
                }
                let dist = (ad.home.0 - bx).hypot(ad.home.1 - by);
                if dist > 170.0 {
                    continue;
                }
                let cost = dist + if ad.breed == bb { 0.0 } else { 45.0 };
                if best.map(|x| cost < x.0).unwrap_or(true) {
                    best = Some((cost, a));
                }
            }
            if let Some((_, a)) = best {
                self.ducks[b].parent = Some(a);
                self.ducks[b].rank = kids[a];
                kids[a] += 1;
            }
        }
    }

    /// Where duckling i should paddle: in a line behind its parent.
    fn follow_spot(&self, i: usize) -> Option<(f32, f32, bool)> {
        let p = self.ducks[i].parent?;
        let pd = self.ducks.get(p)?;
        let sp = pd.vx.hypot(pd.vy);
        let (bx, by) = if sp > 2.0 {
            (-pd.vx / sp, -pd.vy / sp)
        } else {
            (if pd.right { -1.0 } else { 1.0 }, 0.0)
        };
        let gap = 8.0 + self.ducks[i].rank as f32 * 7.0;
        // a little sideways sway so the line isn't ruler-straight
        let sway = ((self.t * 1.3 + self.ducks[i].rank as f32).sin()) * 1.5;
        let (tx, ty) = self.clamp(
            pd.x + bx * gap - by * sway,
            pd.y + by * gap * 0.6 + bx * sway * 0.4,
        );
        let moving = sp > 2.0 || matches!(pd.st, St::Wander(..) | St::Seek(_) | St::Follow);
        Some((tx, ty, moving))
    }

    /// Snapshot/test helper: show every dream icon and a dragonfly right away.
    pub fn demo_extras(&mut self) {
        let n = self.ducks.len();
        for k in 0..6usize {
            if let Some(d) = self.ducks.get_mut((k * 37 + 5) % n.max(1)) {
                d.think = 4.0;
                d.idea = k as u8;
                d.st = St::Idle;
                d.timer = 10.0;
            }
        }
        for k in 0..6usize {
            let i = (k * 53 + 11) % n.max(1);
            if let Some(d) = self.ducks.get_mut(i) {
                d.st = St::Sleep;
                d.timer = 30.0;
                d.think = 0.0;
            }
        }
        let (x, y) = (self.w as f32 * 0.5, self.h as f32 * 0.5);
        self.dragonfly = Some(Dragonfly {
            x,
            y,
            tx: x,
            ty: y,
            hover: 5.0,
            darts: 0,
            t: 0.0,
        });
    }

    pub fn send_all_home(&mut self) {
        for i in 0..self.ducks.len() {
            let d = &mut self.ducks[i];
            if d.st != St::Held {
                d.st = St::Home;
            }
        }
        for _ in 0..3 {
            let n = self.ducks.len();
            if n > 0 {
                let i = (self.rng.next() as usize) % n;
                self.ducks[i].quack = 0.4;
                self.voice(i);
            }
        }
    }

    pub fn wake_and_quack(&mut self, i: usize) {
        if let Some(d) = self.ducks.get_mut(i) {
            if d.st == St::Sleep {
                d.st = St::Idle;
                d.timer = 2.0;
            }
            d.quack = 0.45;
            d.hop = 0.38;
        }
        self.voice(i);
    }

    fn voice(&mut self, i: usize) {
        if let Some(d) = self.ducks.get(i) {
            self.sounds.push(if d.baby {
                Sound::Peep
            } else {
                Sound::Quack(d.voice)
            });
        }
    }

    pub fn grab(&mut self, i: usize) {
        if let Some(d) = self.ducks.get_mut(i) {
            d.st = St::Held;
            d.quack = 0.3;
        }
    }

    pub fn drag_to(&mut self, i: usize, x: f32, y: f32) {
        let (x, y) = self.clamp(x, y);
        if let Some(d) = self.ducks.get_mut(i) {
            if (x - d.x).abs() > 0.3 {
                d.right = x > d.x;
            }
            d.x = x;
            d.y = y;
        }
    }

    /// Drop a held duck: wherever it lands becomes its new home.
    pub fn release(&mut self, i: usize) -> Option<(f32, f32)> {
        let d = self.ducks.get_mut(i)?;
        d.home = (d.x, d.y);
        d.st = St::Idle;
        d.timer = 3.0;
        let (x, y) = (d.x, d.y);
        if !self.is_land(x, y) {
            self.ripples.push(Ripple {
                x,
                y,
                r: 2.0,
                life: 1.0,
            });
        }
        Some((x, y))
    }

    pub fn hit(&self, x: f32, y: f32) -> Option<usize> {
        for &i in self.order.iter().rev() {
            let d = &self.ducks[i];
            if d.hatch < 1.0 {
                continue;
            }
            let (hw, top) = if d.baby { (6.0, 12.0) } else { (8.0, 16.0) };
            if (x - d.x).abs() <= hw && y >= d.y - top && y <= d.y + 1.5 {
                return Some(i);
            }
        }
        None
    }

    // ------------------------------------------------------------------ food

    /// Schedule a handful of crumbs at (x, y). They fall, splash, and ducks come.
    pub fn drop_food(&mut self, x: f32, y: f32) {
        let n = 6 + (self.rng.next() % 4) as usize;
        let mut placed: Vec<(f32, f32)> = vec![];
        for k in 0..n {
            // uniform over an ellipse (flattened: the pond is seen at an angle),
            // keeping crumbs a duck-width apart so eaters don't pile up
            let mut best = (x, y);
            for _ in 0..16 {
                let a = self.rng.range(0.0, 6.283);
                let r = 34.0 * self.rng.f().sqrt();
                let c = (x + a.cos() * r, y + a.sin() * r * 0.55);
                best = c;
                if placed
                    .iter()
                    .all(|p| (p.0 - c.0).hypot((p.1 - c.1) * 1.6) > 15.0)
                {
                    break;
                }
            }
            placed.push(best);
            self.pending_food.push((best.0, best.1, k as f32 * 0.05));
        }
    }

    pub fn feed_everyone(&mut self) {
        let n = self.ducks.len().min(40);
        for _ in 0..n {
            let x = self.rng.range(12.0, self.w as f32 - 12.0);
            let y = self.rng.range(20.0, self.h as f32 - 6.0);
            self.drop_food(x, y);
        }
    }

    fn attract(&mut self, x: f32, y: f32) {
        let mut near: Vec<(f32, usize)> = self
            .ducks
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                matches!(d.st, St::Idle | St::Wander(..) | St::Sleep | St::Follow) && d.hatch >= 1.0
            })
            .map(|(i, d)| ((d.x - x).hypot(d.y - y), i))
            .filter(|(dist, _)| *dist < 120.0)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(_, i) in near.iter().take(8) {
            if self.claim_crumb(i, 140.0) {
                self.ducks[i].quack = if self.rng.f() < 0.3 { 0.3 } else { 0.0 };
            }
        }
    }

    fn claimants(&self, id: u32) -> impl Iterator<Item = &Duck> {
        self.ducks
            .iter()
            .filter(move |d| matches!(d.st, St::Seek(c) | St::Eat(c) | St::Wait(c) if c == id))
    }

    /// Point duck i at the best crumb: one duck eats, up to two more queue
    /// behind it in a line. Busy crumbs cost extra so the flock spreads out.
    fn claim_crumb(&mut self, i: usize, within: f32) -> bool {
        let (x, y) = (self.ducks[i].x, self.ducks[i].y);
        let mut best: Option<(f32, u32, f32, u8)> = None;
        for c in self.crumbs.iter().filter(|c| c.landed && c.bites > 0) {
            let dist = (c.x - x).hypot(c.y - y);
            if dist > within {
                continue;
            }
            let others: Vec<(f32, u8)> = self
                .claimants(c.id)
                .filter(|d| !std::ptr::eq(*d, &self.ducks[i]))
                .map(|d| (d.side, d.slot))
                .collect();
            if others.len() >= 2 {
                continue;
            }
            let side = match others.first() {
                Some((s, _)) => *s,
                None => {
                    if x < c.x {
                        -1.0
                    } else {
                        1.0
                    }
                }
            };
            let slot = (0..2u8)
                .find(|k| others.iter().all(|o| o.1 != *k))
                .unwrap_or(1);
            let cost = dist + others.len() as f32 * 40.0;
            if best.map(|b| cost < b.0).unwrap_or(true) {
                best = Some((cost, c.id, side, slot));
            }
        }
        match best {
            Some((_, id, side, slot)) => {
                let d = &mut self.ducks[i];
                d.st = St::Seek(id);
                d.side = side;
                d.slot = slot;
                true
            }
            None => false,
        }
    }

    /// Where duck i should stand for its place in the crumb's line.
    fn spot(&self, i: usize, cx: f32, cy: f32) -> (f32, f32) {
        let d = &self.ducks[i];
        let k = d.slot as f32;
        self.clamp(cx + d.side * (7.0 + 14.0 * k), cy + 0.5 + k * 2.0)
    }

    fn crumb(&self, id: u32) -> Option<&Crumb> {
        self.crumbs.iter().find(|c| c.id == id && c.bites > 0)
    }

    // ------------------------------------------------------------------ tick

    pub fn busy(&self) -> bool {
        !self.crumbs.is_empty()
            || !self.ripples.is_empty()
            || !self.pending_food.is_empty()
            || self.dragonfly.is_some()
            || !self.particles.is_empty()
            || self.ducks.iter().any(|d| {
                d.hatch < 1.0
                    || d.quack > 0.0
                    || matches!(
                        d.st,
                        St::Wander(..)
                            | St::Seek(_)
                            | St::Wait(_)
                            | St::Home
                            | St::Held
                            | St::Follow
                    )
                    || d.hop > 0.0
                    || d.think > 0.0
                    || d.shake > 0.0
            })
    }

    pub fn update(&mut self, dt: f32) {
        self.t += dt;

        // pending crumbs → falling crumbs
        let mut i = 0;
        while i < self.pending_food.len() {
            self.pending_food[i].2 -= dt;
            if self.pending_food[i].2 <= 0.0 {
                let (x, y, _) = self.pending_food.swap_remove(i);
                let (x, y) = self.clamp(x, y);
                let id = self.next_id;
                self.next_id += 1;
                self.crumbs.push(Crumb {
                    id,
                    x,
                    y,
                    z: 16.0,
                    vz: 0.0,
                    bites: 2,
                    age: 0.0,
                    landed: false,
                });
            } else {
                i += 1;
            }
        }

        // crumbs fall, splash, sink
        let mut landed_at = vec![];
        for c in &mut self.crumbs {
            c.age += dt;
            if !c.landed {
                c.vz += 160.0 * dt;
                c.z -= c.vz * dt;
                if c.z <= 0.0 {
                    c.z = 0.0;
                    c.landed = true;
                    landed_at.push((c.x, c.y));
                }
            }
        }
        for (x, y) in landed_at.iter().copied() {
            if !self.is_land(x, y) {
                self.ripples.push(Ripple {
                    x,
                    y,
                    r: 1.0,
                    life: 1.0,
                });
                for k in 0..3 {
                    let a = -1.2 + k as f32 * 1.2 + self.rng.range(-0.3, 0.3);
                    self.particles.push(Particle {
                        kind: PKind::Drop,
                        x,
                        y: y - 1.0,
                        vx: a * 9.0,
                        vy: -26.0 - self.rng.range(0.0, 8.0),
                        life: 0.45,
                        max: 0.45,
                    });
                }
            }
        }
        if let Some(&(x, y)) = landed_at.first() {
            self.attract(x, y);
        }
        self.crumbs.retain(|c| c.bites > 0 && c.age < 60.0);

        for r in &mut self.ripples {
            r.r += dt * 9.0;
            r.life -= dt * 0.9;
        }
        self.ripples.retain(|r| r.life > 0.0);

        for p in &mut self.particles {
            p.life -= dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            match p.kind {
                PKind::Crumb | PKind::Drop => p.vy += 120.0 * dt,
                PKind::Heart => p.vy *= 1.0 - dt * 0.8,
                PKind::Zed => p.vx = (p.life * 3.0).sin() * 3.0,
                PKind::Sparkle => {}
            }
        }
        self.particles.retain(|p| p.life > 0.0);

        // the odd pink petal drifting across the pond
        self.next_petal -= dt;
        if self.next_petal <= 0.0 && self.petals.len() < 4 {
            self.next_petal = self.rng.range(14.0, 30.0);
            let left = self.rng.f() < 0.5;
            let y = self.rng.range(30.0, self.h as f32 - 20.0);
            self.petals.push(Petal {
                x: if left { -4.0 } else { self.w as f32 + 4.0 },
                y,
                vx: if left { 1.0 } else { -1.0 } * self.rng.range(2.5, 4.5),
                spin: self.rng.range(0.0, 6.0),
                life: 400.0,
            });
        }
        let (pw, t) = (self.w as f32, self.t);
        for p in &mut self.petals {
            p.x += p.vx * dt;
            p.y += (t * 0.4 + p.spin).sin() * 0.6 * dt;
            p.life -= dt;
        }
        self.petals
            .retain(|p| p.life > 0.0 && p.x > -10.0 && p.x < pw + 10.0);
        // sleepy ducks breathe out little z's
        for i in 0..self.ducks.len() {
            let d = &self.ducks[i];
            if d.st == St::Sleep
                && d.hatch >= 1.0
                && !bubbler(&d.name)
                && ((t + i as f32 * 0.7) % 2.6) < dt
            {
                let (x, y) = (
                    d.x + if d.right { 2.0 } else { -2.0 },
                    d.y - if d.baby { 11.0 } else { 13.0 },
                );
                self.particles.push(Particle {
                    kind: PKind::Zed,
                    x,
                    y,
                    vx: 0.0,
                    vy: -5.0,
                    life: 2.2,
                    max: 2.2,
                });
            }
        }

        if self.families_dirty {
            self.rebuild_families();
        }
        self.update_ambient(dt);
        self.compute_separation();
        for i in 0..self.ducks.len() {
            self.update_duck(i, dt);
            self.update_heading(i, dt);
            self.update_wake(i, dt);
        }
    }

    /// Grade the pond for the local time of day (hour 0..24).
    pub fn set_hour(&mut self, hour: f32) {
        // (hour, multiply rgb, add rgb)
        const KEYS: [(f32, [f32; 3], [f32; 3]); 9] = [
            (0.0, [0.38, 0.42, 0.60], [10.0, 12.0, 30.0]),
            (5.0, [0.48, 0.50, 0.66], [16.0, 14.0, 32.0]),
            (6.5, [0.98, 0.88, 0.90], [26.0, 12.0, 14.0]),
            (8.0, [1.0, 1.0, 1.0], [0.0, 0.0, 0.0]),
            (16.5, [1.0, 1.0, 1.0], [0.0, 0.0, 0.0]),
            (18.0, [1.04, 0.92, 0.86], [38.0, 14.0, 4.0]),
            (19.5, [0.80, 0.66, 0.78], [30.0, 8.0, 26.0]),
            (21.0, [0.38, 0.42, 0.60], [10.0, 12.0, 30.0]),
            (24.0, [0.38, 0.42, 0.60], [10.0, 12.0, 30.0]),
        ];
        let h = hour.rem_euclid(24.0);
        let k = KEYS
            .windows(2)
            .find(|w| h >= w[0].0 && h <= w[1].0)
            .unwrap_or(&KEYS[0..2]);
        let (a, b) = (k[0], k[1]);
        let t = ((h - a.0) / (b.0 - a.0).max(0.001)).clamp(0.0, 1.0);
        let t = t * t * (3.0 - 2.0 * t);
        let mut mul = [0.0; 3];
        let mut add = [0.0; 3];
        for c in 0..3 {
            mul[c] = a.1[c] + (b.1[c] - a.1[c]) * t;
            add[c] = a.2[c] + (b.2[c] - a.2[c]) * t;
        }
        let changed = (0..3).any(|c| {
            (mul[c] - self.tint.0[c]).abs() > 0.004 || (add[c] - self.tint.1[c]).abs() > 0.5
        });
        if changed || self.bg_tint.is_empty() {
            self.tint = (mul, add);
            self.night = ((1.0 - (mul[0] + mul[1] + mul[2]) / 3.0) / 0.5).clamp(0.0, 1.0);
            self.apply_tint();
        }
    }

    fn apply_tint(&mut self) {
        let (mul, add) = self.tint;
        self.bg_tint.clear();
        self.bg_tint.extend(self.bg.chunks(4).flat_map(|p| {
            [
                (p[0] as f32 * mul[0] + add[0]).clamp(0.0, 255.0) as u8,
                (p[1] as f32 * mul[1] + add[1]).clamp(0.0, 255.0) as u8,
                (p[2] as f32 * mul[2] + add[2]).clamp(0.0, 255.0) as u8,
                255,
            ]
        }));
    }

    /// How much the ducks themselves take on the time-of-day grade.
    fn duck_tint(&self, c: [u8; 4]) -> [u8; 4] {
        let (mul, add) = self.tint;
        let k = 0.65;
        let f = |v: u8, i: usize| {
            (v as f32 * (1.0 + (mul[i] - 1.0) * k) + add[i] * k).clamp(0.0, 255.0) as u8
        };
        [f(c[0], 0), f(c[1], 1), f(c[2], 2), c[3]]
    }

    fn update_ambient(&mut self, dt: f32) {
        // fireflies come out after dark
        let want = if self.night > 0.35 { 16 } else { 0 };
        while self.fireflies.len() < want {
            let (x, y) = (
                self.rng.range(10.0, self.w as f32 - 10.0),
                self.rng.range(20.0, self.h as f32 - 10.0),
            );
            self.fireflies.push((x, y, self.rng.range(0.0, 100.0)));
        }
        self.fireflies.truncate(want);
        let (w, h, t) = (self.w as f32, self.h as f32, self.t);
        for f in &mut self.fireflies {
            f.0 = (f.0 + (t * 0.3 + f.2).sin() * 4.0 * dt).clamp(4.0, w - 4.0);
            f.1 = (f.1 + (t * 0.23 + f.2 * 1.7).cos() * 3.0 * dt).clamp(14.0, h - 4.0);
        }

        // fish blooping under the lilies
        self.next_bloop -= dt;
        if self.next_bloop <= 0.0 {
            self.next_bloop = self.rng.range(3.5, 9.0);
            for _ in 0..8 {
                let (x, y) = (
                    self.rng.range(10.0, self.w as f32 - 10.0),
                    self.rng.range(25.0, self.h as f32 - 6.0),
                );
                let clear = !self.is_land(x, y)
                    && self
                        .ducks
                        .iter()
                        .all(|d| (d.x - x).abs() > 14.0 || (d.y - y).abs() > 8.0);
                if clear {
                    self.ripples.push(Ripple {
                        x,
                        y,
                        r: 0.8,
                        life: 1.1,
                    });
                    for k in 0..2 {
                        self.particles.push(Particle {
                            kind: PKind::Drop,
                            x: x + k as f32,
                            y: y - 1.0,
                            vx: 0.0,
                            vy: -4.0 - k as f32 * 2.0,
                            life: 0.5,
                            max: 0.5,
                        });
                    }
                    break;
                }
            }
        }
        // a dragonfly darting about now and then
        self.next_dragonfly -= dt;
        if self.dragonfly.is_none() && self.next_dragonfly <= 0.0 {
            self.next_dragonfly = self.rng.range(50.0, 120.0);
            let left = self.rng.f() < 0.5;
            let y = self.rng.range(40.0, self.h as f32 - 40.0);
            let x = if left { -6.0 } else { self.w as f32 + 6.0 };
            self.dragonfly = Some(Dragonfly {
                x,
                y,
                tx: x,
                ty: y,
                hover: 0.0,
                darts: 0,
                t: 0.0,
            });
        }
        let (w, h) = (self.w as f32, self.h as f32);
        let mut gone = false;
        if let Some(f) = &mut self.dragonfly {
            f.t += dt;
            let (dx, dy) = (f.tx - f.x, f.ty - f.y);
            let dist = dx.hypot(dy);
            if dist > 0.8 {
                let step = (90.0 * dt).min(dist);
                f.x += dx / dist * step;
                f.y += dy / dist * step;
            } else if f.hover > 0.0 {
                f.hover -= dt;
            } else {
                f.darts += 1;
                if f.darts > 9 {
                    // off it goes
                    f.tx = if f.x < w / 2.0 { -12.0 } else { w + 12.0 };
                    f.ty = f.y - 20.0;
                    if f.darts > 10 {
                        gone = true;
                    }
                } else {
                    let a = self.rng.range(0.0, 6.283);
                    let r = self.rng.range(25.0, 60.0);
                    f.tx = (f.x.clamp(20.0, w - 20.0) + a.cos() * r).clamp(10.0, w - 10.0);
                    f.ty = (f.y.clamp(30.0, h - 20.0) + a.sin() * r * 0.6).clamp(25.0, h - 10.0);
                    f.hover = self.rng.range(0.4, 1.6);
                }
            }
        }
        if gone {
            self.dragonfly = None;
        }
    }

    fn update_duck(&mut self, i: usize, dt: f32) {
        let (x, y) = (self.ducks[i].x, self.ducks[i].y);
        let on_land = self.is_land(x, y);
        {
            let d = &mut self.ducks[i];
            if d.hatch < 1.0 {
                let before = d.hatch;
                d.hatch = (d.hatch + dt * 1.8).min(1.0);
                if before < 1.0 && d.hatch >= 1.0 && !on_land {
                    self.ripples.push(Ripple {
                        x,
                        y,
                        r: 2.0,
                        life: 0.8,
                    });
                }
                return;
            }
            d.anim += dt;
            d.quack = (d.quack - dt).max(0.0);
            d.next_blink -= dt;
            d.blink = (d.blink - dt).max(0.0);
            if d.next_blink <= 0.0 {
                d.blink = 0.14;
                d.next_blink = 2.5 + (d.anim * 7.3).sin().abs() * 4.0;
            }
            if d.was_land && !on_land {
                self.ripples.push(Ripple {
                    x,
                    y,
                    r: 1.5,
                    life: 0.7,
                });
            }
            let climbed_out = !d.was_land && on_land;
            d.was_land = on_land;
            d.hop = (d.hop - dt).max(0.0);
            d.shake = (d.shake - dt).max(0.0);
            d.think = (d.think - dt).max(0.0);
            d.next_think -= dt;
            if climbed_out {
                d.shake = 0.45;
                let baby = d.baby;
                for k in 0..4 {
                    let a = k as f32 * 1.57 + 0.4;
                    let hgt = if baby { 7.0 } else { 9.0 };
                    self.particles.push(Particle {
                        kind: PKind::Drop,
                        x: x + a.cos() * 5.0,
                        y: y - hgt,
                        vx: a.cos() * 22.0,
                        vy: -18.0 + a.sin() * 6.0,
                        life: 0.4,
                        max: 0.4,
                    });
                }
            }
        }

        let water_mul = if on_land { 1.0 } else { 0.8 };
        let st = self.ducks[i].st;
        match st {
            St::Held => {}
            St::Idle => {
                if self.ducks[i].next_think <= 0.0 {
                    let showing = self.ducks.iter().filter(|d| d.think > 0.0).count();
                    let d = &mut self.ducks[i];
                    d.next_think = self.rng.range(45.0, 140.0);
                    if showing < 3 {
                        d.think = self.rng.range(2.8, 4.2);
                        let r = self.rng.f();
                        // mostly bread, obviously
                        d.idea = if r < 0.38 {
                            0
                        } else if r < 0.55 {
                            1
                        } else if r < 0.67 {
                            2
                        } else if r < 0.78 {
                            3
                        } else if r < 0.88 {
                            4
                        } else {
                            5
                        };
                    }
                }
                self.ducks[i].stay = (self.ducks[i].stay - dt).max(0.0);
                if self.ducks[i].stay <= 0.0 {
                    if let Some((tx, ty, parent_moving)) = self.follow_spot(i) {
                        let far = (tx - x).hypot(ty - y) > 14.0;
                        if parent_moving || far {
                            self.ducks[i].st = St::Follow;
                            return;
                        }
                    }
                }
                let d = &mut self.ducks[i];
                d.timer -= dt;
                if d.timer <= 0.0 {
                    let r = self.rng.f();
                    let wander_p = 0.55 - self.night * 0.3;
                    let sleep_p = wander_p + 0.08 + self.night * 0.35;
                    if r < wander_p {
                        let a = self.rng.range(0.0, 6.283);
                        let rad = self.rng.range(4.0, ROAM);
                        // ducklings with family potter around their parent instead of home
                        let anchored = self.ducks[i].stay <= 0.0 && self.ducks[i].parent.is_some();
                        let (hx, hy, rad) = match (
                            anchored,
                            self.ducks[i].parent.and_then(|p| self.ducks.get(p)),
                        ) {
                            (true, Some(pd)) => (pd.x, pd.y, rad * 0.4),
                            _ => (self.ducks[i].home.0, self.ducks[i].home.1, rad),
                        };
                        let (tx, ty) = self.clamp(hx + a.cos() * rad, hy + a.sin() * rad * 0.6);
                        self.ducks[i].st = St::Wander(tx, ty);
                    } else if r < sleep_p {
                        let d = &mut self.ducks[i];
                        d.st = St::Sleep;
                        d.timer = self.rng.range(8.0, 25.0);
                    } else {
                        let d = &mut self.ducks[i];
                        d.timer = self.rng.range(3.0, 10.0);
                        if r > 0.97 {
                            d.right = !d.right;
                        }
                    }
                }
            }
            St::Sleep => {
                let d = &mut self.ducks[i];
                d.timer -= dt;
                if d.timer <= 0.0 {
                    d.st = St::Idle;
                    d.timer = 2.0;
                }
            }
            St::Wander(tx, ty) => {
                let v = SLOW * self.ducks[i].pace * water_mul;
                if self.step(i, tx, ty, v, dt) {
                    let d = &mut self.ducks[i];
                    d.st = St::Idle;
                    d.timer = self.rng.range(3.0, 11.0);
                }
            }
            St::Home => {
                let (hx, hy) = self.ducks[i].home;
                let v = SCURRY * self.ducks[i].pace;
                if self.step(i, hx, hy, v, dt) {
                    let top = if self.ducks[i].baby { 12.0 } else { 15.0 };
                    let sx = self.rng.range(-6.0, 6.0);
                    self.particles.push(Particle {
                        kind: PKind::Sparkle,
                        x: hx + sx,
                        y: hy - top,
                        vx: 0.0,
                        vy: -3.0,
                        life: 0.7,
                        max: 0.7,
                    });
                    let d = &mut self.ducks[i];
                    d.st = St::Idle;
                    d.timer = self.rng.range(5.0, 14.0);
                    if d.baby {
                        // stay put a while so the tidy layout lasts
                        d.stay = self.rng.range(60.0, 120.0);
                    }
                }
            }
            St::Follow => match self.follow_spot(i) {
                Some((tx, ty, parent_moving)) => {
                    let p = self.ducks[i].parent.unwrap_or(i);
                    let psp = self.ducks[p].vx.hypot(self.ducks[p].vy);
                    let v = (SLOW * 2.0 * self.ducks[i].pace).max(psp * 1.25) * water_mul;
                    let arrived = self.step(i, tx, ty, v, dt);
                    if arrived && !parent_moving {
                        let pr = self.ducks[p].right;
                        let t = self.rng.range(2.0, 6.0);
                        let d = &mut self.ducks[i];
                        d.st = St::Idle;
                        d.timer = t;
                        d.right = pr;
                    }
                }
                None => self.ducks[i].st = St::Idle,
            },
            St::Seek(id) => match self.crumb(id).map(|c| (c.x, c.y)) {
                Some((cx, cy)) => {
                    let v = EAGER * self.ducks[i].pace * water_mul;
                    let (tx, ty) = self.spot(i, cx, cy);
                    if self.step(i, tx, ty, v, dt) {
                        let d = &mut self.ducks[i];
                        d.right = cx > tx;
                        if d.slot == 0 {
                            d.st = St::Eat(id);
                            d.timer = 0.8;
                        } else {
                            d.st = St::Wait(id);
                            d.timer = self.rng.range(1.5, 4.0);
                        }
                    }
                }
                None => self.next_crumb_or_idle(i),
            },
            St::Wait(id) => {
                let Some((cx, cy)) = self.crumb(id).map(|c| (c.x, c.y)) else {
                    self.next_crumb_or_idle(i);
                    return;
                };
                // move up if the line has a gap (e.g. the eater wandered off)
                let mine = self.ducks[i].slot;
                let ahead = (0..self.ducks.len())
                    .filter(|&j| j != i && matches!(self.ducks[j].st, St::Seek(c) | St::Eat(c) | St::Wait(c) if c == id))
                    .filter(|&j| self.ducks[j].slot < mine)
                    .count() as u8;
                if ahead < mine {
                    self.ducks[i].slot = ahead;
                    self.ducks[i].st = St::Seek(id);
                    return;
                }
                let d = &mut self.ducks[i];
                d.right = cx > d.x;
                d.vx = 0.0;
                d.vy = 0.0;
                // the odd impatient quack
                d.timer -= dt;
                if d.timer <= 0.0 {
                    d.timer = self.rng.range(2.5, 5.0);
                    if self.rng.f() < 0.35 {
                        self.ducks[i].quack = 0.3;
                    }
                }
                let _ = cy;
            }
            St::Eat(id) => {
                let Some((cx, _)) = self.crumb(id).map(|c| (c.x, c.y)) else {
                    self.next_crumb_or_idle(i);
                    return;
                };
                // shuffle a little if someone is crowding, but keep facing the crumb
                let (px, py) = self.sep.get(i).copied().unwrap_or((0.0, 0.0));
                let d = &mut self.ducks[i];
                d.x += (px * 0.25 * dt).clamp(-0.6, 0.6);
                d.y += (py * 0.25 * dt).clamp(-0.4, 0.4);
                d.vx = 0.0;
                d.vy = 0.0;
                d.right = cx > d.x;
                d.dir = if d.right { 0 } else { 4 };
                self.ducks[i].timer -= dt;
                if self.ducks[i].timer <= 0.0 {
                    if let Some(c) = self.crumbs.iter_mut().find(|c| c.id == id) {
                        c.bites = c.bites.saturating_sub(1);
                    }
                    // a happy quack after every bite, crumbs flying
                    self.ducks[i].quack = 0.35;
                    self.voice(i);
                    let (dx, dy, r) = (self.ducks[i].x, self.ducks[i].y, self.ducks[i].right);
                    let bx = dx + if r { 7.0 } else { -7.0 };
                    for _ in 0..2 {
                        let vx = self.rng.range(-14.0, 14.0);
                        self.particles.push(Particle {
                            kind: PKind::Crumb,
                            x: bx,
                            y: dy - 2.0,
                            vx,
                            vy: -22.0,
                            life: 0.35,
                            max: 0.35,
                        });
                    }
                    if self.crumb(id).is_none() {
                        let hy = dy - if self.ducks[i].baby { 13.0 } else { 16.0 };
                        self.particles.push(Particle {
                            kind: PKind::Heart,
                            x: dx,
                            y: hy,
                            vx: 0.0,
                            vy: -9.0,
                            life: 1.3,
                            max: 1.3,
                        });
                    }
                    if self.crumb(id).is_some() {
                        // polite: if someone is waiting, step to the back of the line
                        let waiting: Vec<usize> = (0..self.ducks.len())
                            .filter(|&j| j != i && matches!(self.ducks[j].st, St::Wait(c) | St::Seek(c) if c == id))
                            .collect();
                        if waiting.is_empty() {
                            self.ducks[i].timer = 0.8;
                        } else {
                            for &j in &waiting {
                                let d = &mut self.ducks[j];
                                d.slot = d.slot.saturating_sub(1);
                                d.st = St::Seek(id);
                            }
                            let back = waiting.len() as u8;
                            let d = &mut self.ducks[i];
                            d.slot = back;
                            d.st = St::Seek(id);
                        }
                    } else {
                        self.next_crumb_or_idle(i);
                    }
                }
            }
        }
    }

    /// Pick which way the duck is drawn from its velocity. The pond is seen
    /// at an angle, so vertical motion counts extra. A new heading has to hold
    /// for a moment before the sprite turns, so ducks don't flicker.
    fn update_heading(&mut self, i: usize, dt: f32) {
        let d = &mut self.ducks[i];
        let moving = matches!(d.st, St::Wander(..) | St::Seek(_) | St::Home | St::Follow);
        if !moving {
            // once a duck stops it turns side-on so you see its face
            d.dir = if d.right { 0 } else { 4 };
            d.cand_t = 0.0;
            return;
        }
        if d.vx.hypot(d.vy) < 3.0 {
            return;
        }
        let a = (d.vy * 1.5).atan2(d.vx);
        let sector = ((a / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(8) as u8;
        if sector == d.dir {
            d.cand_t = 0.0;
            return;
        }
        if sector != d.cand {
            d.cand = sector;
            d.cand_t = 0.0;
        }
        d.cand_t += dt;
        let hold = if d.st == St::Home { 0.06 } else { 0.14 };
        if d.cand_t >= hold {
            d.dir = sector;
            match sector {
                0 | 1 | 7 => d.right = true,
                3 | 4 | 5 => d.right = false,
                _ => {}
            }
        }
    }

    /// Swimming ducks trail little rings; resting ones send out a slow one now
    /// and then.
    fn update_wake(&mut self, i: usize, dt: f32) {
        let d = &self.ducks[i];
        if d.hatch < 1.0 || d.st == St::Held || self.is_land(d.x, d.y) || self.ripples.len() > 400 {
            return;
        }
        let speed = d.vx.hypot(d.vy);
        let (x, y, vx, vy) = (d.x, d.y, d.vx, d.vy);
        let d = &mut self.ducks[i];
        d.wake -= dt;
        if d.wake > 0.0 {
            return;
        }
        if speed > 4.0 {
            // drop the ring a little behind the tail
            let k = 5.0 / speed;
            self.ripples.push(Ripple {
                x: x - vx * k,
                y: y - vy * k * 0.5 - 0.5,
                r: 1.2,
                life: 0.75,
            });
            d.wake = if d.st == St::Home { 0.12 } else { 0.28 };
        } else {
            self.ripples.push(Ripple {
                x,
                y: y - 0.5,
                r: 3.0,
                life: 0.9,
            });
            d.wake = 3.0 + (x * 0.37 + y).sin().abs() * 3.0;
        }
    }

    fn next_crumb_or_idle(&mut self, i: usize) {
        self.ducks[i].st = St::Idle;
        if !self.claim_crumb(i, 50.0) {
            self.ducks[i].timer = 1.5;
        }
    }

    /// Steer duck i toward (tx, ty) with momentum: it turns smoothly, eases
    /// off as it arrives, and edges around neighbours. True on arrival.
    fn step(&mut self, i: usize, tx: f32, ty: f32, max_speed: f32, dt: f32) -> bool {
        let homing = self.ducks[i].st == St::Home;
        let (px, py) = if homing {
            (0.0, 0.0)
        } else {
            self.sep.get(i).copied().unwrap_or((0.0, 0.0))
        };
        let d = &mut self.ducks[i];
        let (dx, dy) = (tx - d.x, ty - d.y);
        let dist = dx.hypot(dy);
        let speed = d.vx.hypot(d.vy);
        if dist < 0.8 && speed < max_speed * 0.35 + 2.0 {
            d.x = tx;
            d.y = ty;
            d.vx = 0.0;
            d.vy = 0.0;
            return true;
        }
        // ease in over the last few pixels
        let want = max_speed * (dist / 10.0).clamp(0.2, 1.0);
        let (mut wx, mut wy) = if dist > 0.0 {
            (dx / dist * want, dy / dist * want)
        } else {
            (0.0, 0.0)
        };
        wx += px;
        wy += py;
        let turn = (if homing { 9.0 } else { 4.5 } * dt).min(1.0);
        d.vx += (wx - d.vx) * turn;
        d.vy += (wy - d.vy) * turn;
        // don't overshoot the target in one tick
        let step = d.vx.hypot(d.vy) * dt;
        if step > dist && px == 0.0 && py == 0.0 {
            d.x = tx;
            d.y = ty;
        } else {
            d.x += d.vx * dt;
            d.y += d.vy * dt;
        }
        if d.vx.abs() > 2.0 {
            d.right = d.vx > 0.0;
        }
        false
    }

    /// Personal space: a push away from close neighbours for every duck that
    /// is on the move or eating (resting ducks keep their exact homes).
    fn compute_separation(&mut self) {
        const R: f32 = 15.0;
        self.sep.clear();
        self.sep.resize(self.ducks.len(), (0.0, 0.0));
        for i in 0..self.ducks.len() {
            let a = &self.ducks[i];
            if !matches!(
                a.st,
                St::Seek(_) | St::Eat(_) | St::Wait(_) | St::Wander(..) | St::Follow
            ) || a.hatch < 1.0
            {
                continue;
            }
            let (mut fx, mut fy) = (0.0, 0.0);
            for (j, b) in self.ducks.iter().enumerate() {
                if i == j || b.st == St::Held || b.hatch < 1.0 {
                    continue;
                }
                // depth matters less than width: ducks side by side overlap most
                let (dx, dy) = (a.x - b.x, (a.y - b.y) * 1.8);
                let d = dx.hypot(dy);
                if d < R && d > 0.01 {
                    let k = (R - d) / R * 26.0;
                    fx += dx / d * k;
                    fy += dy / d * k * 0.55;
                } else if d <= 0.01 {
                    fx += if i < j { 8.0 } else { -8.0 };
                }
            }
            self.sep[i] = (fx, fy);
        }
    }

    // ------------------------------------------------------------------ draw

    /// (sheet column, mirrored?) for a duck right now.
    fn frame_for(&self, d: &Duck) -> (usize, bool) {
        use Frame as F;
        let water = !self.is_land(d.x, d.y);
        let moving = matches!(d.st, St::Wander(..) | St::Seek(_) | St::Home | St::Follow);
        let fast = d.st == St::Home;
        let side_flip = !d.right;
        if d.st == St::Held {
            return (F::Quack as usize, side_flip);
        }
        // side-only poses
        let side_pose = matches!(d.st, St::Eat(_) | St::Sleep) || (d.quack > 0.0 && !water);
        // view: None = side; Some(v) indexes VIEW_BASE blocks (0 S, 1 SE, 2 NE, 3 N)
        let (view, flip) = if side_pose {
            (None, side_flip)
        } else {
            match d.dir {
                0 => (None, false),
                4 => (None, true),
                1 => (Some(1), false),
                3 => (Some(1), true),
                2 => (Some(0), side_flip),
                6 => (Some(3), side_flip),
                7 => (Some(2), false),
                _ => (Some(2), true), // 5: NW
            }
        };
        if let Some(v) = view {
            let base = VIEW_BASE + v * VIEW_FRAMES;
            let k = if water {
                let p = if moving {
                    if fast {
                        0.12
                    } else {
                        0.3
                    }
                } else {
                    0.75
                };
                6 + ((d.anim / p) as usize) % 2
            } else if moving {
                let p = if fast { 0.05 } else { 0.12 };
                2 + ((d.anim / p) as usize) % 4
            } else if d.blink > 0.0 {
                1
            } else {
                0
            };
            return (base + k, flip);
        }
        let fr = if water {
            if let St::Eat(_) = d.st {
                if d.quack > 0.0 {
                    F::Swim1 as usize
                } else if (d.anim * 2.5) as i32 % 2 == 0 {
                    F::Dabble as usize
                } else {
                    F::Swim1 as usize
                }
            } else if d.st == St::Sleep {
                if ((d.anim / 1.5) as i32) % 2 == 0 {
                    F::Sleepw as usize
                } else {
                    F::Sleepw2 as usize
                }
            } else {
                let p = if moving {
                    if fast {
                        0.12
                    } else {
                        0.3
                    }
                } else {
                    0.75
                };
                if (d.anim / p) as i32 % 2 == 0 {
                    F::Swim1 as usize
                } else {
                    F::Swim2 as usize
                }
            }
        } else if d.quack > 0.0 {
            F::Quack as usize
        } else {
            match d.st {
                St::Eat(_) => {
                    if (d.anim * 5.0) as i32 % 2 == 0 {
                        F::Peck1 as usize
                    } else {
                        F::Peck2 as usize
                    }
                }
                St::Sleep => {
                    if ((d.anim / 1.5) as i32) % 2 == 0 {
                        F::Sleep as usize
                    } else {
                        F::Sleep2 as usize
                    }
                }
                _ if moving => {
                    let p = if fast { 0.05 } else { 0.12 };
                    F::Walk1 as usize + ((d.anim / p) as usize) % 4
                }
                _ => {
                    if d.blink > 0.0 {
                        F::Blink as usize
                    } else {
                        F::Idle as usize
                    }
                }
            }
        };
        (fr, flip)
    }

    pub fn render(&mut self, frame: &mut [u8]) {
        frame.copy_from_slice(if self.bg_tint.len() == self.bg.len() {
            &self.bg_tint
        } else {
            &self.bg
        });
        let w = self.w as i32;
        let h = self.h as i32;

        // water glints
        for &(idx, ph) in &self.glints {
            let v = (self.t * 1.3 + ph).sin();
            if v > 0.82 {
                let k = ((v - 0.82) * 260.0 * (1.0 - self.night * 0.7)) as u16;
                let (gx, gy) = (idx % self.w, idx / self.w);
                for by in gy..(gy + self.bg_k).min(self.h) {
                    for bx in gx..(gx + self.bg_k).min(self.w) {
                        let i = ((by * self.w + bx) * 4) as usize;
                        for c in 0..3 {
                            frame[i + c] = (frame[i + c] as u16 + k).min(255) as u8;
                        }
                    }
                }
            }
        }

        // ripples: broken two-tone rings, brighter on the near side
        for r in &self.ripples {
            let life = r.life.clamp(0.0, 1.0);
            let a = (life * life * 200.0) as u8;
            if a < 8 {
                continue;
            }
            let steps = ((r.r * 6.5) as i32).max(10);
            let mut last = (i32::MIN, i32::MIN);
            for s in 0..steps {
                // gaps that drift as the ring grows
                if (s + (r.r * 2.0) as i32) % 5 == 0 {
                    continue;
                }
                let th = s as f32 / steps as f32 * std::f32::consts::TAU;
                let (sn, cs) = th.sin_cos();
                let x = (r.x + cs * r.r).round() as i32;
                let y = (r.y + sn * r.r * 0.42).round() as i32;
                if (x, y) == last {
                    continue;
                }
                last = (x, y);
                if sn > 0.0 {
                    blend(frame, w, h, x, y, [236, 246, 255, a]);
                    blend(frame, w, h, x, y + 1, [20, 40, 80, a / 4]);
                } else {
                    blend(frame, w, h, x, y, [220, 236, 255, a / 2]);
                }
            }
        }

        // petals: a tiny pink two-tone petal, gently turning
        for p in &self.petals {
            let (x, y) = (p.x.round() as i32, p.y.round() as i32);
            let turn = ((self.t * 0.8 + p.spin).sin() > 0.0) as i32;
            blend(frame, w, h, x, y + 1, [40, 50, 110, 50]);
            blend(frame, w, h, x, y, [255, 196, 214, 255]);
            blend(frame, w, h, x + 1, y, [255, 226, 236, 255]);
            blend(frame, w, h, x + turn, y - 1, [240, 160, 190, 255]);
        }

        // bread: a little crust-edged crumb with a highlight
        for c in &self.crumbs {
            let (x, y) = (c.x as i32, c.y as i32);
            let water = c.landed && !self.is_land(c.x, c.y);
            let bob = if water && ((self.t * 1.6 + c.id as f32 * 0.7) as i32 % 2 == 0) {
                1
            } else {
                0
            };
            // shadow grows as it falls
            let sh = if c.landed { 0.0 } else { (16.0 - c.z) / 16.0 };
            if !c.landed {
                blend(
                    frame,
                    w,
                    h,
                    x,
                    y + 1,
                    [20, 20, 50, (40.0 + 60.0 * sh) as u8],
                );
                blend(
                    frame,
                    w,
                    h,
                    x + 1,
                    y + 1,
                    [20, 20, 50, (40.0 + 60.0 * sh) as u8],
                );
            } else if water {
                blend(frame, w, h, x - 1, y + 1 + bob, [20, 30, 70, 50]);
                blend(frame, w, h, x + 2, y + 1 + bob, [20, 30, 70, 50]);
            }
            let cy = y - c.z as i32 - 1 + bob;
            let crust = [196, 132, 66, 255];
            let crumb = [246, 214, 150, 255];
            let light = [255, 240, 200, 255];
            if c.bites > 1 {
                // 3x2 crumb
                blend(frame, w, h, x - 1, cy, crust);
                blend(frame, w, h, x, cy, light);
                blend(frame, w, h, x + 1, cy, crumb);
                blend(frame, w, h, x - 1, cy + 1, crust);
                blend(frame, w, h, x, cy + 1, crumb);
                blend(frame, w, h, x + 1, cy + 1, crust);
            } else {
                // nibbled: 2x2
                blend(frame, w, h, x, cy, light);
                blend(frame, w, h, x + 1, cy, crust);
                blend(frame, w, h, x, cy + 1, crust);
            }
        }

        // ducks back-to-front
        self.order.clear();
        self.order.extend(0..self.ducks.len());
        let ducks = &self.ducks;
        self.order
            .sort_by(|&a, &b| ducks[a].y.total_cmp(&ducks[b].y));

        for oi in 0..self.order.len() {
            let i = self.order[oi];
            let d = &self.ducks[i];
            if d.hatch < 0.0 {
                continue;
            }
            let drop = if d.hatch < 1.0 {
                ((1.0 - d.hatch).powi(2) * 40.0) as i32
            } else {
                0
            };
            let held_lift = if d.st == St::Held { 3 } else { 0 };
            let shake_dx = if d.shake > 0.0 {
                if ((d.shake * 30.0) as i32) % 2 == 0 {
                    1
                } else {
                    -1
                }
            } else {
                0
            };
            let fx = d.x.round() as i32 + shake_dx;
            let fy = d.y.round() as i32;
            let hop = if d.hop > 0.0 {
                ((d.hop / 0.38) * std::f32::consts::PI).sin() * 4.0
            } else {
                0.0
            };
            let hop = hop.round() as i32;
            let on_land = self.is_land(d.x, d.y);

            // soft shadow on land / when lifted
            if on_land || held_lift > 0 || drop > 0 || hop > 0 {
                let rx = if d.baby { 4 } else { 6 };
                for sx in -rx..=rx {
                    blend(frame, w, h, fx + sx, fy + 1, [30, 24, 60, 60]);
                }
                for sx in -rx + 2..=rx - 2 {
                    blend(frame, w, h, fx + sx, fy + 2, [30, 24, 60, 35]);
                }
            }
            if Some(i) == self.selected {
                for s in 0..28 {
                    if s % 2 == 0 {
                        let th = s as f32 / 28.0 * std::f32::consts::TAU;
                        let x = fx + (th.cos() * 9.0) as i32;
                        let y = fy + 1 + (th.sin() * 3.0) as i32;
                        blend(frame, w, h, x, y, [255, 236, 140, 230]);
                    }
                }
            }

            let (fr, flip) = self.frame_for(d);
            let row = d.breed * 2 + d.baby as usize;
            let left = fx - CELL_W as i32 / 2;
            let top = fy - CELL_H as i32 + 1 - drop - held_lift - hop;
            let swimming = !on_land && drop == 0 && held_lift == 0 && hop == 0;
            let ext = self.extents.get(row).and_then(|r| r.get(fr)).copied();
            let water_row = ext.filter(|e| e.0 >= 0).map(|(b, x0, x1)| {
                let (x0, x1) = if flip {
                    (CELL_W as i32 - 1 - x1, CELL_W as i32 - 1 - x0)
                } else {
                    (x0, x1)
                };
                (top + b + 1, left + x0, left + x1)
            });
            if swimming {
                if let Some((wy, x0, x1)) = water_row {
                    // the body under the surface: water darkens, tapering down
                    for (k, a) in [(0, 70u8), (1, 45), (2, 22)] {
                        for x in x0 + k..=x1 - k {
                            blend(frame, w, h, x, wy + k, [18, 30, 70, a]);
                        }
                    }
                }
            }
            self.blit_duck(frame, row, fr, left, top, flip);
            if d.st == St::Sleep && bubbler(&d.name) && d.hatch >= 1.0 {
                // slowly inflating sleep bubble that pops, then starts again
                let cyc = ((self.t + (hash(&d.name) % 97) as f32 * 0.13) % 4.0) / 4.0;
                if cyc < 0.92 {
                    let a = ANCHORS[row][fr];
                    let bx = if flip {
                        CELL_W as i32 - 1 - a.beak.0
                    } else {
                        a.beak.0
                    };
                    let s = if flip { -1 } else { 1 };
                    let r = 0.6 + cyc * 2.6;
                    let (cx, cy) = (
                        left + bx + s * (1 + r.round() as i32),
                        top + a.beak.1 - r.round() as i32 + 1,
                    );
                    // pale inside so it reads as a bubble, not a ring
                    let ri = r.round() as i32;
                    for yy in -ri..=ri {
                        for xx in -ri..=ri {
                            if ((xx * xx + yy * yy) as f32) < (r - 0.6).max(0.3).powi(2) {
                                blend(frame, w, h, cx + xx, cy + yy, [225, 240, 255, 110]);
                            }
                        }
                    }
                    let steps = (r * 8.0) as i32 + 6;
                    for k in 0..steps {
                        let th = k as f32 / steps as f32 * std::f32::consts::TAU;
                        let x = cx + (th.cos() * r).round() as i32;
                        let y = cy + (th.sin() * r).round() as i32;
                        blend(frame, w, h, x, y, [190, 225, 255, 220]);
                    }
                    if r > 1.4 {
                        blend(frame, w, h, cx - s, cy - 1, [255, 255, 255, 255]);
                    }
                }
            }
            if swimming {
                if let Some((wy, x0, x1)) = water_row {
                    // shimmering waterline hugging the body, with little curls at the ends
                    let ph = (self.t * 3.0 + d.x * 0.3) as i32;
                    for x in x0 - 1..=x1 + 1 {
                        if (x + ph).rem_euclid(4) != 0 {
                            blend(frame, w, h, x, wy, [235, 246, 255, 170]);
                        }
                    }
                    blend(frame, w, h, x0 - 2, wy + 1, [235, 246, 255, 110]);
                    blend(frame, w, h, x1 + 2, wy + 1, [235, 246, 255, 110]);
                }
            }

            if let Some(hi) = self.hat_index(d).filter(|_| self.show_hats) {
                let a = ANCHORS[row][fr];
                let info = HATS[hi];
                let mx = |x: i32| if flip { CELL_W as i32 - 1 - x } else { x };
                let (hx, hy) = match info.anchor {
                    HatAnchor::Top => (
                        mx(a.head_top.0) - info.w as i32 / 2,
                        a.head_top.1 + info.overlap - info.h as i32,
                    ),
                    HatAnchor::Eye => (
                        mx(a.eye.0) - info.w as i32 / 2,
                        a.eye.1 - info.h as i32 / 2 + 1,
                    ),
                    HatAnchor::Head => (
                        mx(a.head.0) - info.w as i32 / 2,
                        a.head.1 - info.h as i32 / 2,
                    ),
                };
                // in water, don't draw a hat below the waterline cut
                self.blit_hat(frame, hi, left + hx, top + hy, flip);
            }

            if d.quack > 0.0 {
                // three little sound lines fanning out from the beak
                let a = ANCHORS[row][fr];
                let s = if d.right { 1 } else { -1 };
                let hx = if d.right {
                    left + a.head.0
                } else {
                    left + CELL_W as i32 - 1 - a.head.0
                };
                let (bx, by) = (hx + s * 6, top + a.head.1 - 2);
                let al = (d.quack.min(0.3) / 0.3 * 230.0) as u8;
                let ink = [255, 255, 255, al];
                blend(frame, w, h, bx, by - 2, ink);
                blend(frame, w, h, bx + s, by - 3, ink);
                blend(frame, w, h, bx + s, by, ink);
                blend(frame, w, h, bx + s * 2, by, ink);
                blend(frame, w, h, bx, by + 2, ink);
                blend(frame, w, h, bx + s, by + 3, ink);
            }
        }

        // particles: hearts, z's, flying crumbs, droplets
        for p in &self.particles {
            let (x, y) = (p.x.round() as i32, p.y.round() as i32);
            let a = ((p.life / p.max).clamp(0.0, 1.0) * 255.0) as u8;
            match p.kind {
                PKind::Heart => {
                    let pink = [255, 120, 160, a];
                    let hi = [255, 210, 225, a];
                    for (dx, dy, c) in [
                        (-2, 0, pink),
                        (-1, 0, hi),
                        (1, 0, pink),
                        (2, 0, pink),
                        (-2, 1, pink),
                        (-1, 1, pink),
                        (0, 1, pink),
                        (1, 1, pink),
                        (2, 1, pink),
                        (-1, 2, pink),
                        (0, 2, pink),
                        (1, 2, pink),
                        (0, 3, pink),
                    ] {
                        blend(frame, w, h, x + dx, y + dy, c);
                    }
                }
                PKind::Zed => {
                    let c = [235, 240, 255, (a as f32 * 0.85) as u8];
                    for (dx, dy) in [
                        (0, 0),
                        (1, 0),
                        (2, 0),
                        (3, 0),
                        (2, 1),
                        (1, 2),
                        (0, 3),
                        (1, 3),
                        (2, 3),
                        (3, 3),
                    ] {
                        blend(frame, w, h, x + dx, y + dy, c);
                    }
                }
                PKind::Crumb => blend(frame, w, h, x, y, [240, 200, 130, a]),
                PKind::Drop => blend(frame, w, h, x, y, [225, 240, 255, a]),
                PKind::Sparkle => {
                    let big = ((p.life * 12.0) as i32) % 2 == 0;
                    let c = [255, 246, 190, a];
                    blend(frame, w, h, x, y, [255, 255, 255, a]);
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        blend(frame, w, h, x + dx, y + dy, c);
                    }
                    if big {
                        for (dx, dy) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
                            blend(frame, w, h, x + dx, y + dy, [255, 246, 190, a / 2]);
                        }
                    }
                }
            }
        }

        // dragonfly: teal body, flickering wings, faint shadow on the water
        if let Some(f) = &self.dragonfly {
            let (x, y) = (f.x.round() as i32, f.y.round() as i32);
            let facing = if f.tx >= f.x { 1 } else { -1 };
            blend(frame, w, h, x, y + 7, [20, 30, 70, 45]);
            blend(frame, w, h, x - facing, y + 7, [20, 30, 70, 45]);
            for k in 0..4 {
                blend(
                    frame,
                    w,
                    h,
                    x - facing * k,
                    y,
                    if k == 0 {
                        [40, 70, 110, 255]
                    } else {
                        [50, 170, 170, 255]
                    },
                );
            }
            let up = ((f.t * 28.0) as i32) % 2 == 0;
            let wing = [225, 240, 255, 200];
            if up {
                blend(frame, w, h, x - facing, y - 1, wing);
                blend(frame, w, h, x - facing * 2, y - 1, wing);
                blend(frame, w, h, x - facing, y - 2, [225, 240, 255, 120]);
            } else {
                blend(frame, w, h, x - facing, y + 1, wing);
                blend(frame, w, h, x - facing * 2, y + 1, wing);
            }
        }

        // fireflies: soft blinking glow
        for &(fx, fy, ph) in &self.fireflies {
            let cyc = ((self.t + ph) % 3.2) / 3.2;
            let on = if cyc < 0.35 {
                (cyc / 0.35 * std::f32::consts::PI).sin()
            } else {
                0.0
            };
            if on <= 0.02 {
                continue;
            }
            let (x, y) = (fx.round() as i32, fy.round() as i32);
            let a = (on * 255.0) as u8;
            blend(frame, w, h, x, y, [255, 255, 210, a]);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                blend(
                    frame,
                    w,
                    h,
                    x + dx,
                    y + dy,
                    [220, 255, 140, (on * 170.0) as u8],
                );
            }
            for (dx, dy) in [(1, 1), (-1, 1), (1, -1), (-1, -1), (2, 0), (-2, 0)] {
                blend(
                    frame,
                    w,
                    h,
                    x + dx,
                    y + dy,
                    [200, 255, 120, (on * 40.0) as u8],
                );
            }
            // reflection on the water
            if !self.is_land(fx, fy + 6.0) {
                blend(frame, w, h, x, y + 6, [230, 255, 160, (on * 60.0) as u8]);
            }
        }

        // thought bubbles: a little cloud with a dream inside
        for &i in &self.order {
            let d = &self.ducks[i];
            if d.think <= 0.0 || d.hatch < 1.0 {
                continue;
            }
            let pop = ((4.0 - d.think).min(0.15) / 0.15).min(1.0);
            let fade = (d.think / 0.35).min(1.0);
            let a = (255.0 * fade) as u8;
            let s = if d.right { 1 } else { -1 };
            let head_top = d.y.round() as i32 - if d.baby { 13 } else { 16 };
            let hx = d.x.round() as i32 + s * 3;
            // trailing dots
            blend(frame, w, h, hx + s * 2, head_top - 1, [255, 255, 255, a]);
            blend(frame, w, h, hx + s * 3, head_top - 3, [255, 255, 255, a]);
            blend(frame, w, h, hx + s * 4, head_top - 3, [255, 255, 255, a]);
            // bubble body (11x9 rounded), springs up as it appears
            let bw = 11;
            let bh = 9;
            let bx = if s > 0 { hx + 3 } else { hx - 3 - bw };
            let by = head_top - 4 - bh + ((1.0 - pop) * 3.0) as i32;
            let ink = [120, 104, 160, a];
            for yy in 0..bh {
                for xx in 0..bw {
                    let corner = (yy == 0 || yy == bh - 1) && (xx < 2 || xx > bw - 3)
                        || (yy == 1 || yy == bh - 2) && (xx == 0 || xx == bw - 1);
                    if corner {
                        continue;
                    }
                    let edge = yy == 0
                        || yy == bh - 1
                        || xx == 0
                        || xx == bw - 1
                        || ((yy == 1 || yy == bh - 2) && (xx == 1 || xx == bw - 2));
                    blend(
                        frame,
                        w,
                        h,
                        bx + xx,
                        by + yy,
                        if edge { ink } else { [255, 255, 255, a] },
                    );
                }
            }
            draw_idea(frame, w, h, bx + 3, by + 2, d.idea, a);
        }

        // name tags on top of everything
        let mut tags: Vec<usize> = vec![];
        if self.show_labels {
            tags.extend(self.order.iter().copied());
        }
        for o in [self.selected, self.hover].into_iter().flatten() {
            tags.retain(|&t| t != o);
            if !self.hires_tags {
                tags.push(o);
            }
        }
        for i in tags {
            if let Some(d) = self.ducks.get(i) {
                let txt = font::label_for(&d.name, 22);
                let tw = font::text_width(&txt);
                let x = (d.x.round() as i32 - tw / 2).clamp(2, w - tw - 2);
                let ytop = d.y.round() as i32 - CELL_H as i32 - 6;
                let y = if ytop < 2 {
                    d.y.round() as i32 + 4
                } else {
                    ytop
                };
                let strong = Some(i) == self.selected || Some(i) == self.hover;
                let bg = if strong {
                    [40, 30, 70, 215]
                } else {
                    [40, 30, 70, 150]
                };
                for yy in y - 1..y + 6 {
                    for xx in x - 2..x + tw + 2 {
                        let corner =
                            (yy == y - 1 || yy == y + 5) && (xx == x - 2 || xx == x + tw + 1);
                        if !corner {
                            blend(frame, w, h, xx, yy, bg);
                        }
                    }
                }
                font::draw(&txt, x, y, |px, py| {
                    blend(frame, w, h, px, py, [255, 250, 240, 255])
                });
            }
        }
    }

    /// Hover + selected tags for the hi-res overlay: (name, x, top of duck)
    /// in art pixels, selected first.
    pub fn hires_tag_targets(&self) -> Vec<(String, f32, f32)> {
        let mut out = vec![];
        for i in [self.selected, self.hover].into_iter().flatten() {
            if out.iter().any(|(_, _, _, j)| *j == i) {
                continue;
            }
            if let Some(d) = self.ducks.get(i) {
                let top = d.y - if d.baby { 14.0 } else { CELL_H as f32 } + 1.0;
                out.push((d.name.clone(), d.x, top, i));
            }
        }
        out.into_iter().map(|(n, x, t, _)| (n, x, t)).collect()
    }

    fn hat_index(&self, d: &Duck) -> Option<usize> {
        // babies only wear the small, sweet ones
        let hi = d.hat?;
        if d.baby && matches!(HATS[hi].name, "mortar" | "propeller" | "cap" | "straw") {
            return HATS.iter().position(|h| h.name == "bow");
        }
        Some(hi)
    }

    fn blit_duck(&self, frame: &mut [u8], row: usize, fr: usize, left: i32, top: i32, flip: bool) {
        let sw = self.sheet.w as usize;
        let (w, h) = (self.w as i32, self.h as i32);
        for sy in 0..CELL_H as usize {
            for sx in 0..CELL_W as usize {
                let si = ((row * CELL_H as usize + sy) * sw + fr * CELL_W as usize + sx) * 4;
                let p = &self.sheet.px[si..si + 4];
                if p[3] == 0 {
                    continue;
                }
                let dx = if flip { CELL_W as usize - 1 - sx } else { sx };
                let c = self.duck_tint([p[0], p[1], p[2], p[3]]);
                blend(frame, w, h, left + dx as i32, top + sy as i32, c);
            }
        }
    }

    fn blit_hat(&self, frame: &mut [u8], hi: usize, left: i32, top: i32, flip: bool) {
        let info = HATS[hi];
        let sw = self.hats.w as usize;
        let (w, h) = (self.w as i32, self.h as i32);
        for sy in 0..info.h as usize {
            for sx in 0..info.w as usize {
                let si = (sy * sw + hi * HAT_CELL_W as usize + sx) * 4;
                let p = &self.hats.px[si..si + 4];
                if p[3] == 0 {
                    continue;
                }
                let dx = if flip { info.w as usize - 1 - sx } else { sx };
                blend(
                    frame,
                    w,
                    h,
                    left + dx as i32,
                    top + sy as i32,
                    [p[0], p[1], p[2], p[3]],
                );
            }
        }
    }

    /// Rescale homes/positions when the art resolution changes.
    pub fn resize(&mut self, w: u32, h: u32, pond: &Img, landmask: &Img) {
        let (sx, sy) = (w as f32 / self.w as f32, h as f32 / self.h as f32);
        self.w = w;
        self.h = h;
        self.build_background(pond, landmask);
        self.apply_tint();
        for d in &mut self.ducks {
            d.x *= sx;
            d.y *= sy;
            d.home = (d.home.0 * sx, d.home.1 * sy);
        }
        self.crumbs.clear();
        self.ripples.clear();
    }
}

fn sprite_extents(sheet: &Img) -> Vec<Vec<(i32, i32, i32)>> {
    let rows = sheet.h / CELL_H;
    let cols = sheet.w / CELL_W;
    (0..rows)
        .map(|r| {
            (0..cols)
                .map(|c| {
                    let mut bottom = -1;
                    for y in 0..CELL_H {
                        for x in 0..CELL_W {
                            let i =
                                (((r * CELL_H + y) * sheet.w + c * CELL_W + x) * 4 + 3) as usize;
                            if sheet.px[i] > 0 {
                                bottom = y as i32;
                            }
                        }
                    }
                    let (mut x0, mut x1) = (CELL_W as i32, -1);
                    if bottom >= 0 {
                        for x in 0..CELL_W {
                            let i = (((r * CELL_H + bottom as u32) * sheet.w + c * CELL_W + x) * 4
                                + 3) as usize;
                            if sheet.px[i] > 0 {
                                x0 = x0.min(x as i32);
                                x1 = x1.max(x as i32);
                            }
                        }
                    }
                    (bottom, x0, x1)
                })
                .collect()
        })
        .collect()
}

/// Half the flock sleeps with a little bubble at the beak, half with z's.
fn bubbler(name: &str) -> bool {
    (hash(name) >> 17) & 1 == 0
}

/// 5x5 dream icons: 0 bread, 1 heart, 2 fish, 3 flower, 4 note, 5 duckling.
fn draw_idea(frame: &mut [u8], w: i32, h: i32, x: i32, y: i32, idea: u8, a: u8) {
    let (rows, pal): ([&str; 5], &[(char, [u8; 3])]) = match idea {
        0 => (
            [".ccc.", "cllbc", "clbbc", "cbbbc", "ccccc"],
            &[
                ('c', [196, 132, 66]),
                ('b', [246, 214, 150]),
                ('l', [255, 240, 200]),
            ],
        ),
        1 => (
            ["pp.pp", "plppp", "ppppp", ".ppp.", "..p.."],
            &[('p', [255, 120, 160]), ('l', [255, 210, 225])],
        ),
        2 => (
            ["..bb.", "bbbeb", ".bbbb", "bbbb.", "....."],
            &[('b', [90, 150, 220]), ('e', [30, 30, 50])],
        ),
        3 => (
            [".p.p.", "ppypp", ".ppp.", "..g..", ".gg.."],
            &[
                ('p', [255, 160, 200]),
                ('y', [255, 220, 90]),
                ('g', [90, 170, 90]),
            ],
        ),
        4 => (
            ["..kk.", "..k.k", "..k..", "kkk..", "kkk.."],
            &[('k', [70, 60, 110])],
        ),
        _ => (
            [".yy..", "yyeo.", "yyyy.", ".yyy.", "....."],
            &[
                ('y', [255, 220, 80]),
                ('e', [30, 30, 50]),
                ('o', [255, 150, 60]),
            ],
        ),
    };
    for (yy, row) in rows.iter().enumerate() {
        for (xx, ch) in row.chars().enumerate() {
            if let Some((_, c)) = pal.iter().find(|(k, _)| *k == ch) {
                blend(
                    frame,
                    w,
                    h,
                    x + xx as i32,
                    y + yy as i32,
                    [c[0], c[1], c[2], a],
                );
            }
        }
    }
}

#[inline]
fn blend(frame: &mut [u8], w: i32, h: i32, x: i32, y: i32, c: [u8; 4]) {
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let i = ((y * w + x) * 4) as usize;
    let a = c[3] as u32;
    if a == 255 {
        frame[i..i + 3].copy_from_slice(&c[..3]);
        return;
    }
    let ia = 255 - a;
    for k in 0..3 {
        frame[i + k] = ((c[k] as u32 * a + frame[i + k] as u32 * ia) / 255) as u8;
    }
}

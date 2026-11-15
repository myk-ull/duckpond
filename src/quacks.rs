// Sound library: real, softened quacks (see assets/quacks/CREDITS.md) and synthesised peeps.
pub static QUACKS: [&[u8]; 12] = [
    include_bytes!("../assets/quacks/quack01.wav"),
    include_bytes!("../assets/quacks/quack02.wav"),
    include_bytes!("../assets/quacks/quack03.wav"),
    include_bytes!("../assets/quacks/quack04.wav"),
    include_bytes!("../assets/quacks/quack05.wav"),
    include_bytes!("../assets/quacks/quack06.wav"),
    include_bytes!("../assets/quacks/quack07.wav"),
    include_bytes!("../assets/quacks/quack08.wav"),
    include_bytes!("../assets/quacks/quack09.wav"),
    include_bytes!("../assets/quacks/quack10.wav"),
    include_bytes!("../assets/quacks/quack11.wav"),
    include_bytes!("../assets/quacks/quack12.wav"),
];
pub static PEEPS: [&[u8]; 4] = [
    include_bytes!("../assets/quacks/peep1.wav"),
    include_bytes!("../assets/quacks/peep2.wav"),
    include_bytes!("../assets/quacks/peep3.wav"),
    include_bytes!("../assets/quacks/peep4.wav"),
];

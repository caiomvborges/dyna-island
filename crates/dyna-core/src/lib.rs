//! Platform-independent models and logic for Dyna Island.
//!
//! Everything here is pure Rust with no OS calls so it can be unit-tested on
//! any machine. The Windows crate (`dyna-island`) owns all I/O and rendering and
//! feeds clocks/dates into these types. `store` (the JSON files) uses `std`
//! I/O, which still runs anywhere.

pub mod addons;
pub mod agenda;
pub mod cliphist;
pub mod datetime;
pub mod dock;
pub mod frames;
pub mod frametime;
pub mod geom;
pub mod glass;
pub mod hdr;
pub mod ics;
pub mod inuse;
pub mod look;
pub mod notes;
pub mod nowplaying;
pub mod pomodoro;
pub mod power;
pub mod script;
pub mod settings;
pub mod spring;
pub mod store;
pub mod svgpath;
pub mod sysmon;
pub mod timeline;
pub mod todo;

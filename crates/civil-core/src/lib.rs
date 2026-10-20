#![recursion_limit = "512"]
//! civil-core: survey points, TIN surfaces, alignments, profiles, design-standard
//! checks, earthwork, drawings, and an append-only audit log.
pub mod hydrology;
pub mod culvert;
pub mod slope;
pub mod subsurface;
pub mod mech;
pub mod reinforce;
pub mod traffic;
pub mod landuse;
pub mod ext;
pub mod align;
pub mod landxml;
pub mod superelev;
pub mod audit;
pub mod drawing;
pub mod font;
pub mod corridor;
pub mod grading;
pub mod intersect;
pub mod pipes;
pub mod quantities;
pub mod points;
pub mod pavement;
pub mod profile;
pub mod report;
pub mod route;
pub mod deliver;
pub mod protection;

pub mod sheet;
pub mod sheetset;
pub mod standards;
pub mod tin;
pub mod tools;
pub mod walls;
pub mod widening;
pub mod workflow;

pub type Result<T> = std::result::Result<T, String>;

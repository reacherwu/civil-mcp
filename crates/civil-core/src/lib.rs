#![recursion_limit = "512"]
#![allow(
    clippy::too_many_arguments,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::field_reassign_with_default,
    clippy::doc_lazy_continuation,
    clippy::type_complexity
)]
//! civil-core: survey points, TIN surfaces, alignments, profiles, design-standard
//! checks, earthwork, drawings, and an append-only audit log.
pub mod align;
pub mod audit;
pub mod corridor;
pub mod culvert;
pub mod deliver;
pub mod drawing;
pub mod ext;
pub mod ext6;
pub mod font;
pub mod grading;
pub mod hydrology;
pub mod intersect;
pub mod landuse;
pub mod landxml;
pub mod lea;
pub mod mech;
pub mod pavement;
pub mod pipes;
pub mod points;
pub mod profile;
pub mod protection;
pub mod quantities;
pub mod reinforce;
pub mod report;
pub mod route;
pub mod signals;
pub mod slope;
pub mod subsurface;
pub mod superelev;
pub mod traffic;

pub mod sheet;
pub mod sheetset;
pub mod standards;
pub mod tin;
pub mod tools;
pub mod walls;
pub mod wallsel;
pub mod widening;
pub mod workflow;

pub type Result<T> = std::result::Result<T, String>;

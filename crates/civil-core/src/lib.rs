//! civil-core: survey points, TIN surfaces, alignments, profiles, design-standard
//! checks, earthwork, drawings, and an append-only audit log.
pub mod align;
pub mod audit;
pub mod drawing;
pub mod earthwork;
pub mod points;
pub mod profile;
pub mod report;
pub mod sheet;
pub mod standards;
pub mod tin;
pub mod tools;

pub type Result<T> = std::result::Result<T, String>;

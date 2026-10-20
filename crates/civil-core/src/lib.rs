//! civil-core: survey points, TIN surfaces, alignments, profiles, design-standard
//! checks, earthwork, drawings, and an append-only audit log.
pub mod align;
pub mod landxml;
pub mod superelev;
pub mod audit;
pub mod drawing;
pub mod font;
pub mod corridor;
pub mod grading;
pub mod pipes;
pub mod quantities;
pub mod points;
pub mod profile;
pub mod report;
pub mod sheet;
pub mod standards;
pub mod tin;
pub mod tools;

pub type Result<T> = std::result::Result<T, String>;

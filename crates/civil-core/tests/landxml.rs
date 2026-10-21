use civil_core::align::{Alignment, AlignmentInput};
use civil_core::landxml::{read, to_alignment, write, Export};
use civil_core::points::SurveyPoint;
use civil_core::profile::{Profile, ProfileInput};
use civil_core::tin::Tin;

fn sample_alignment() -> Alignment {
    Alignment::build(AlignmentInput {
        name: "CL".into(),
        pis: vec![[1000.0, 2000.0], [1300.0, 2200.0], [1500.0, 2150.0], [1800.0, 2500.0]],
        radii: vec![150.0, 120.0],
        spirals: vec![50.0, 0.0],
        start_station: 0.0,
    })
    .unwrap()
}

#[test]
fn roundtrip_alignment_profile_surface_points() {
    let al = sample_alignment();
    let prof =
        Profile::build(ProfileInput { name: "FG".into(), vpis: vec![[0.0, 100.0], [300.0, 106.0], [al.length, 98.0]], curve_lengths: vec![120.0] }).unwrap();
    let pts: Vec<[f64; 3]> = (0..60)
        .map(|i| {
            let (x, y) = ((i * 37 % 100) as f64, (i * 61 % 90) as f64);
            [x, y, 50.0 + 0.1 * x - 0.05 * y]
        })
        .collect();
    let (tin, st) = Tin::build(&pts, None).unwrap();
    let sp = vec![SurveyPoint { id: "1".into(), e: 1.5, n: 2.5, z: 3.25, desc: "road & <edge> é".into() }];
    let x = write(&Export {
        points: &sp,
        surfaces: vec![("EG", &tin)],
        alignment: Some(&al),
        profiles: vec![&prof],
        networks: vec![],
        corridor: None,
        epsg: None,
        parcels: None,
    });
    let d = read(&x).unwrap();
    assert_eq!(d.points.len(), 1);
    assert_eq!(d.points[0].desc, "road & <edge> é");
    assert!((d.points[0].e - 1.5).abs() < 1e-9 && (d.points[0].n - 2.5).abs() < 1e-9);
    let s = &d.surfaces[0];
    assert_eq!(s.faces.len(), st.triangles);
    let (t2, st2) = Tin::from_faces(&s.points, &s.faces).unwrap();
    assert!((st2.area_m2 - st.area_m2).abs() < 1e-3, "{} {}", st2.area_m2, st.area_m2);
    for (x, y) in [(10.0, 10.0), (50.0, 40.0), (77.0, 33.0)] {
        if let (Some(a), Some(b)) = (tin.elevation_at(x, y), t2.elevation_at(x, y)) {
            assert!((a - b).abs() < 1e-3);
        }
    }
    let a = &d.alignments[0];
    assert!(a.max_end_deviation < 1e-5, "end deviation {}", a.max_end_deviation);
    let al2 = to_alignment(a).unwrap();
    assert_eq!(al2.elements.len(), al.elements.len());
    assert!((al2.length - al.length).abs() < 1e-5);
    let mut s = 0.0;
    while s <= al.length {
        let (p, q) = (al.point_at(s).unwrap(), al2.point_at(s).unwrap());
        assert!(((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt() < 1e-5, "station {s}");
        s += 7.3;
    }
    let p2 = Profile::build(a.profiles[0].clone()).unwrap();
    let mut s = 0.0;
    while s <= al.length {
        assert!((p2.elevation_at(s).unwrap() - prof.elevation_at(s).unwrap()).abs() < 1e-5);
        s += 5.0;
    }
}

/// Civil 3D 2023 export (RustedGeom/rusted-geom, MIT): alignment + profile.
#[test]
fn civil3d_alignment_fixture() {
    let d = read(include_str!("fixtures/c3dprofile2.xml")).unwrap();
    let a = &d.alignments[0];
    let al = to_alignment(a).unwrap();
    let reported = a.reported_length.unwrap();
    // measured on 2026-10-20: |diff| < 1e-6 m, max end deviation < 1e-6 m (see docs/VALIDATION.md)
    assert!((al.length - reported).abs() < 1e-4, "{} vs {}", al.length, reported);
    assert!(a.max_end_deviation < 1e-3, "{}", a.max_end_deviation);
    assert!(!a.profiles.is_empty());
    Profile::build(a.profiles[0].clone()).unwrap();
}

# Demo data

- `dem.npy`: 200×200 float32 elevations (m), USGS 3D Elevation Program (3DEP), fetched 2026-10-20 from
  `https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer/exportImage`
  bbox lon −122.170..−122.160, lat 37.400..37.408 (foothills near Palo Alto, CA), resampled bilinear
  to WGS84 / UTM zone 10N at 4.4769 m pixels. Upper-left corner E 573453.119, N 4140465.753. USGS data is public domain.
- `usgs3dep_palo_alto_points.csv`: 5,607 pixel centres drawn at random (seed 42) from that raster plus the four
  corners, used as stand-in "survey points". They are not field survey shots.

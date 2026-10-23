-- PostGIS's types, as Supabase and Neon ship them: geometry with and without a type and SRID,
-- geography, box2d and geometry[] columns. Inside a JSON document PostGIS writes a geometry as
-- GeoJSON, through its cast to JSON, and the other types as their text. An array of geometries
-- separates its elements with a colon, not a comma.
create extension postgis;
create table places (
  id int primary key,
  location geometry(Point, 4326) not null,
  shape geometry,
  area geography,
  bounds box2d,
  trail geometry[]
);

-- PostGIS compares geometries by their bytes, so POINT(-0 -0), POINT(0 0) and a point under another SRID stay distinct to DISTINCT though GeoJSON writes them alike: an array of them says nothing.
create extension postgis;
create table shapes (id int primary key, g geometry);
insert into shapes values (1, 'POINT(-0 -0)'), (2, 'POINT(0 0)'), (3, 'SRID=4326;POINT(1 2)'), (4, 'POINT(1 2)');

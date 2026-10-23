select id, st_asgeojson(location) as geojson, st_astext(shape) as wkt, st_srid(shape) as srid, st_x(location) as x from places

select id, st_makepoint(:x, :y) as made, st_setsrid(st_makepoint(:lng, :lat), 4326)::geography as point from places where location && st_makeenvelope(0, 0, 10, 10, 4326)

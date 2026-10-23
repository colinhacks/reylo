select id, st_distance(area, :p) as meters from places where st_dwithin(area, :p2, :d)

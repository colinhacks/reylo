select id, earth_distance(spot, ll_to_earth(:lat, :lng)) as meters, latitude(spot) as lat, longitude(spot) as lng, ll_to_earth(1, 2) as made from stock

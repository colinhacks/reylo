select p.id, d from places p, st_dump(p.shape) d

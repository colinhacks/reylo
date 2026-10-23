select id, cube_dim(dims) as n, cube_ll_coord(dims, 1) as low, dims <-> :c as distance, dims @> :c2 as holds from stock

select array_agg(location) as locations, array_agg(shape) filter (where shape is not null) as shapes, st_collect(location) as collected, st_extent(shape) as extent from places

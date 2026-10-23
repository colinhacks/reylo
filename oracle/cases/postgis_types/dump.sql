select id, (st_dump(shape)).path, (st_dump(shape)).geom from places

select json_build_object('r', (select x from doc x order by x.id limit 1)) as o

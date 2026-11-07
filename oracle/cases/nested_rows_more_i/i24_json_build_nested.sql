select x.j from cc, lateral (select json_build_object('r', (select c from replies c limit 1)) as j) x

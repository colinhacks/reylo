select x.j from cc, lateral (select json_build_object('r', c) as j from replies c) x

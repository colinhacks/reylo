select jsonb_build_object('a', jsonb_build_object('b', name)) as o from team group by rollup ((jsonb_build_object('b', name)))

select json_build_object('js', (select array_agg(j) from doc), 'one', array[d.j]) as o from doc d

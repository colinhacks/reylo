select jsonb_build_object('j', j, 'n', n) as o, json_build_array(j) as a from doc where id = :id

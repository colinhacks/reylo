select json_agg(c.*) filter (where c.body <> '') as doc from bodies_for(1) c

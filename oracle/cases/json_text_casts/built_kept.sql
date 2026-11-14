select jsonb_build_object('inner', json_build_object('id', id, 'label', label)::jsonb, 'list', (select json_agg(d.id) from doc d)::jsonb) as o from doc where id = :id

select m.id, x.items from member m left join lateral (select jsonb_agg(jsonb_build_object('name', t.name)) as items from team t where t.id = m.id) x on true

select x.snapshot from member m join lateral (select jsonb_build_object('name', t.name) as snapshot from team t where t.id = m.id) x on true

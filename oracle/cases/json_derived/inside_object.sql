select jsonb_build_object('teams', x.teams) as j from member m left join lateral (select json_agg(t.name) as teams from team t where t.id = m.id) x on true

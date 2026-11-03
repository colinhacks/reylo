select jsonb_build_object('team', t, 'size', t.n) as doc from (select tm.id, tm.name, count(m.id) as n from teams tm left join members m on m.team_id = tm.id group by tm.id) t

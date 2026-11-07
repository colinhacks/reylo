select to_jsonb(t) as doc from members m left join teams t (a) on t.a = m.team_id

select jsonb_agg(s) as rows from (select id, name from teams) s

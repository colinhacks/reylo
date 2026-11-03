select array_agg(s order by s.id) as rows, (select s2 from (select id, score from bar) s2 order by s2.id limit 1) as first from (select id, name from bar) s

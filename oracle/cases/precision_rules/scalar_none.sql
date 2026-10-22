select (select generate_series(1, 0)) as g, (select unnest('{}'::int[])) as u, (select count(*) from member limit 0) as l, (select abs(generate_series(1, 0))) as a

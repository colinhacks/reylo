select json_build_object('total', (select coalesce(sum(q), 0) from (select n as sum from s where false) q)) as j

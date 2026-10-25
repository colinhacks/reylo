select (select coalesce(sum(x.n), 0) from s x) as total, (select coalesce(sum(n), 0) from s) as bare, json_build_object('t', (select coalesce(sum(x.n), 0) from s x)) as j

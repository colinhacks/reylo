select x.j from (select a as c from aa) o cross join lateral (select row_to_json(c) as j from (select 1 as id) c) x

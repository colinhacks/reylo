select x.j from cc, lateral (select (select row_to_json(c) from (select 1 as id) c) as j) x

select x.j from cc, lateral (select row_to_json(c) as j from (select 1 as id) c) x

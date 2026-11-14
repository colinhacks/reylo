select d.j, x.j as k from cc, (select to_jsonb(c) as j from (select 1 as id) c) d, lateral (select to_jsonb(c) as j from (select 1 as id) c) x

select d.j from cc, (select to_jsonb(c) as j from (select 1 as id) c) d

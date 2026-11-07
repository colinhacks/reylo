select d.j from cc join (select to_jsonb(c) as j from (select 1 as id) c) d on true

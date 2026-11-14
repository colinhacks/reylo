select d.j from cc, (select to_jsonb(c) as j from (select 1 as id) c union all select to_jsonb(c) from (select 2 as id) c) d

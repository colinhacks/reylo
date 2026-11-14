with w as (select to_jsonb(c) as j from (select 1 as id) c) select w.j from cc, w

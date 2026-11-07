select d.j, d.arr from cc, (select to_jsonb(c) as j, array(select c from (select 'z' as street) c) as arr from (select 1 as id) c) d

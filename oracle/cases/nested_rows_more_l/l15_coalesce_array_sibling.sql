select d.arr from cc, (select array(select coalesce(c, null) from (select 'z' as street) c) as arr) d

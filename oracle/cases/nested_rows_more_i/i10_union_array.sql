select x.arr from cc, lateral (select array(select c from (select 'z' as street) c) as arr union all select array(select c from (select 'z' as street) c)) x

select d.arr from cc, (select array(select c from (select 'z' as street) c) as arr) d

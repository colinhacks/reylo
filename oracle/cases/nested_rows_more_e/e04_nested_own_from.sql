select array(select d from (select x.id, array(select c from (select 'z' as street) c) as arr from cc x) d) as arr

select array(select d from (select l.arr from cc x, lateral (select array(select c from (select 'z' as street) c) as arr) l) d) as arr

select l2.arr from cc, lateral (select 1 as q) l1, lateral (select array(select c from (select 'z' as street) c) as arr) l2

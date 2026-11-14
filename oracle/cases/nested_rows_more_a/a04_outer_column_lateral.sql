select l.arr from cc, lateral (select array(select c from (select 'z' as street) c) as arr) l

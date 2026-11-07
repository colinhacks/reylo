select l.arr from cc, lateral (select m.arr from lateral (select array(select c from (select 'z' as street) c) as arr) m) l

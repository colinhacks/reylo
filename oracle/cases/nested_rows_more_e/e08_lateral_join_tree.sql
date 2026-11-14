select l.arr from users u join cc on true cross join lateral (select array(select c from (select 'z' as street) c) as arr) l

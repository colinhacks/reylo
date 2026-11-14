select l.arr from (select 1) o(c), lateral (select array(select c from (select 'z' as street) c) as arr) l

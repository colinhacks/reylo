select array(select d from (select x.id, l.arr from cc x cross join lateral (select array(select c from replies c where c.id > x.id - 100) as arr) l) d) as arr

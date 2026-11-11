select array(select d from (select l.arr from cc x cross join lateral (select array(select c from replies c) as arr) l) d) as arr

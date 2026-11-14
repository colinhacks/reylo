select array(select d from (select l.arr from lateral (select array(select c from replies c) as arr) l) d) as arr from cc

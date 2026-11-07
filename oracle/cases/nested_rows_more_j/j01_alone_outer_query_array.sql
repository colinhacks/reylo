select (select l.arr from lateral (select array(select c from replies c) as arr) l) as arr from cc

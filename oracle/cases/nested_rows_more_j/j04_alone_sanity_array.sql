select l.arr from lateral (select array(select c from replies c) as arr) l

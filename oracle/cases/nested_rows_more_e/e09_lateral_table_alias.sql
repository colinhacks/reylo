select l.arr from cc, lateral (select array(select c from replies c) as arr) l

select x.arr from cc, lateral (select d.arr from (select array(select c from replies c) as arr) d) x

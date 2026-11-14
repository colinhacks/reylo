select l.arr from holder2, lateral (select array(select c from replies c) as arr) l

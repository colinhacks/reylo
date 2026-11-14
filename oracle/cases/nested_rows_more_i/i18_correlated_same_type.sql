select l.arr from holder2, lateral (select array(select c from replies c where c.id > holder2.id - 100) as arr) l

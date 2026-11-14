select l.arr from cc, lateral (select array(select c from replies c where c.id > cc.id - 100) as arr) l

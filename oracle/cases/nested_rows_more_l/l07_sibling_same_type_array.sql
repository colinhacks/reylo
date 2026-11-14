select d.arr from holder2, (select array(select c from replies c) as arr) d

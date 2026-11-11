select (select array(select c from replies c where c.id > 0)) as arr from cc

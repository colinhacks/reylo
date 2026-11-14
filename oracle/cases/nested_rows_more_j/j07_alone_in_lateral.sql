select l.arr from cc cross join lateral (select (select m.arr from lateral (select array(select c from replies c) as arr) m) as arr) l

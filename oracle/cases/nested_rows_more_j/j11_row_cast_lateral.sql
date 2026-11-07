select l.arr from cc, lateral (select array(select row(r.body, 1)::addr from replies r) as arr) l

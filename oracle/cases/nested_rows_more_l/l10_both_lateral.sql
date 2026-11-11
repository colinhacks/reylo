select x.j, x.arr from holder2, lateral (select to_jsonb(c) as j, array(select c from replies c) as arr from (select 1 as id) c) x

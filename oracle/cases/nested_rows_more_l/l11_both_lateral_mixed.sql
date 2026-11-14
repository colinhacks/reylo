select x.j, x.arr from cc, lateral (select to_jsonb(r) as j, array(select c from (select 'z' as street) c) as arr from replies r) x

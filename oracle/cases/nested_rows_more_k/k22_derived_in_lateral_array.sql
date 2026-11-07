select x.arr from aa o(id, c) cross join lateral (select d.arr from (select array(select c from (select 'z' as street) c) as arr) d) x

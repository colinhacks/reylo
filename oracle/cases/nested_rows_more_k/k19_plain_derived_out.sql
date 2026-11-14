select (select d.arr from (select array(select c from (select 'z' as street) c) as arr) d) as arr from aa o(id, c)

select (select l.arr from lateral (select array(select c from (select 'z' as street) c) as arr) l) as arr from aa o(id, c)

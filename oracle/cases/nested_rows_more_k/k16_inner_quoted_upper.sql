select (select l.arr from lateral (select array(select "C" from (select 'z' as street) "C") as arr) l) as arr from aa o(id, c)

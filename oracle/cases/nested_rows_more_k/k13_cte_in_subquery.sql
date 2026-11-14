select (with w as (select array(select c from (select 'z' as street) c) as arr) select arr from w) as arr from aa o(id, c)

with w as (select c from cc) select l.arr from w, lateral (select array(select c from (select 'z' as street) c) as arr) l

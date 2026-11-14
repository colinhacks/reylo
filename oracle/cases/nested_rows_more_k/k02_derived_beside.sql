select l.arr from (select row('s', 1)::addr as c union all select null) o, lateral (select array(select c from (select 'z' as street) c) as arr) l

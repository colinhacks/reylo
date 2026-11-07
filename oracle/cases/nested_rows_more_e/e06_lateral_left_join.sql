select l.arr from cc left join lateral (select array(select c from (select 'z' as street) c) as arr) l on true

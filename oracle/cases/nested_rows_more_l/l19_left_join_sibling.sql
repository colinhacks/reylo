select d.arr from cc left join (select array(select c from (select 'z' as street) c) as arr) d on true

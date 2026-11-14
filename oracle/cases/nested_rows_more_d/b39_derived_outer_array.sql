select d.arr from users u left join (select array(select c from (select 1 as n) c) as arr) d on false

select array_append(array(select c from (select 1 as n) c), null) as arr

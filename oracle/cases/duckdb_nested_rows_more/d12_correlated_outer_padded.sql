select array(select c from (select o.id + 1 as m) c) as arr from (select 1 as z) z left join posts o on false

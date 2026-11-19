select d from docs d where d.id in (select 1 union select 2)

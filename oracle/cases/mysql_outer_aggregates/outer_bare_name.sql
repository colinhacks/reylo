select b.id, (select count(name) from posts limit 1) as n from nobody b

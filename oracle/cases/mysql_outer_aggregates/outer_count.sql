select b.id, b.name, (select count(b.name) from posts p limit 1) as n from nobody b

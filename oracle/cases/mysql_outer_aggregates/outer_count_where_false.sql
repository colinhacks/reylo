select b.id, (select count(b.name) from posts p where false) as n from nobody b

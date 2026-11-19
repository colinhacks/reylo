select b.id, (select count(*) from posts p where p.user_id = b.id) as n from nobody b

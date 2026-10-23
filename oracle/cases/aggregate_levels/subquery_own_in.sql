select (select count(p.id in (select post_id from comments)) from posts p where false) as n from users

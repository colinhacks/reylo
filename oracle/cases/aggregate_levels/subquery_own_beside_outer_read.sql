select (select count(p.id + (select count(name) from comments)) from posts p where false) as n from users

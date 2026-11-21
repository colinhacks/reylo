select x.r from (select role as r from users) x union select w.k from (select kind as k from posts) w

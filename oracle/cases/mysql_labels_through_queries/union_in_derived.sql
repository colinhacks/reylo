select x.r from (select role as r from users union select kind from posts) x

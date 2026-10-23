select (select count(name || (select body from comments limit 1)) from posts where false) as n from users

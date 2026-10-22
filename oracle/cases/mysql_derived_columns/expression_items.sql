select d.next, d.shout, d.mail from (select id + 1 as next, upper(name) as shout, lower(email) as mail from users) d

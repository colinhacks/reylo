-- A row assigned fewer values than it has columns is the database's own error.
update posts set (user_id, slug, lang) = (select id, email from users where users.id = posts.user_id)

select to_jsonb(u) as author, to_jsonb(p) as post from users u full join posts p on p.user_id = u.id

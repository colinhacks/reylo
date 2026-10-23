select p.id, to_jsonb(u) as author from users u right join posts p on p.user_id = u.id

select u.id, to_jsonb(v) as post from users u left join post_titles v on v.user_id = u.id

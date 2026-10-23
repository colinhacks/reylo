select u.id, to_jsonb(p) as post, to_json(p) as plain, row_to_json(p) as row, to_jsonb(p.*) as star from users u left join posts p on p.user_id = u.id

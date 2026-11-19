select u.name, p.title, p.published_at from users u left join posts p on p.user_id = u.id where date_format(p.published_at, '%Y') >= '2000'

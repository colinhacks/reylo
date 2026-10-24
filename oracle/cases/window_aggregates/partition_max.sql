select max(p.title) over (partition by p.user_id) as last_title from posts p

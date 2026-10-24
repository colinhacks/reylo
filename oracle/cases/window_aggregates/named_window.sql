select sum(p.id) over w as total, max(p.title) over w as last_title from posts p window w as (partition by p.user_id order by p.id)

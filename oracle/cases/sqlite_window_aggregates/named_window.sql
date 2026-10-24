select max(p.id) over w as top, group_concat(p.title) over w as titles from posts p window w as (partition by p.user_id order by p.id)

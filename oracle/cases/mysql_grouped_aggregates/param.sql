select p.user_id, max(p.id) as top from posts p where p.user_id = ? group by p.user_id

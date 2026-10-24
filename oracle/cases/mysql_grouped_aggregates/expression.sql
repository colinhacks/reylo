select p.user_id, sum(p.id) + 1 as next, coalesce(max(p.note), '') as note from posts p group by p.user_id

select p.id, array(select v from comment_view v where v.post_id = p.id) as comments from posts p

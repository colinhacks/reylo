delete from comments c using users u where c.author_id = u.id returning c.author_id, c.parent_id

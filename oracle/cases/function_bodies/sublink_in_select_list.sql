select c.id, (select pick(c.post_id)) as v from comments c

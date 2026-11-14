select c.id from comments c where c.id = (select pick(c.post_id))::bigint

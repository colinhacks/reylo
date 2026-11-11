select distinct c.post_id from comments c where c.post_id = (select pick(c.id))::bigint

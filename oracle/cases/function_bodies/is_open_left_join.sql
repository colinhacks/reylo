select is_open(t) as open from comments c left join posts t on t.id = c.post_id and c.note is null

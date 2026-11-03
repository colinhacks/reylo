select p.tags, s.tags as u from posts p cross join (select tags from posts union all select '{NULL}'::text[]) s

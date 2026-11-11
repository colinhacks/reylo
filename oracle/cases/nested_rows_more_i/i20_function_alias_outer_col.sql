select x.j from cc, lateral (select to_jsonb(c) as j from comments_for(1) c) x

select x.j from cc, lateral (select to_jsonb(c) as j from (values (1)) c(id)) x

select x.j from replies, lateral (select to_jsonb(replies) as j from (select 1 as id) replies) x

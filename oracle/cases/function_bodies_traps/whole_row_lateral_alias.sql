select only_archived(p) as v from (select id, title, nullif(done, false) as done, archived, subtitle from posts) p, lateral (select 1 from posts p limit 1) x

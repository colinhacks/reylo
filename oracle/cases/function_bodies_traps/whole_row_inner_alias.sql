select only_archived(p) as v from (select id, title, nullif(done, false) as done, archived, subtitle from posts p) p

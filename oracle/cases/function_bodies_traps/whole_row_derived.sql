select only_archived(s) as v from (select id, title, nullif(done, false) as done, archived, subtitle from posts) s

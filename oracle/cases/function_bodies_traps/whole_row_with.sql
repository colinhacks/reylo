with s as (select id, title, nullif(done, false) as done, archived, subtitle from posts) select only_archived(s) as v from s

with posts as (select id, title, nullif(done, false) as done, archived, subtitle from public.posts) select only_archived(p) as v from posts p

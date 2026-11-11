with recent as (select id, title from posts where id > :after) select r.id, r.title from recent r

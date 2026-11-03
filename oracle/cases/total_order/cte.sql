with recent as (select id, title from posts where score > :min) select title from recent order by id

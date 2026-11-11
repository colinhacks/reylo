select distinct on (title) author_id as title from posts where title = :t order by title

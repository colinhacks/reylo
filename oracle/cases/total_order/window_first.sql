select id, title from posts order by row_number() over (), id

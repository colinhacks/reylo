select p.id, p.title, author_of(p) as author, tags_of(p) as tags, latest_comments(p) as comments, comment_count(p) as comment_count from post p where visible_to(p, :viewer::int8) order by p.id

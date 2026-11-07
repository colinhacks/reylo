select p.id from posts p where exists (select 1 from comments_for(p.id))

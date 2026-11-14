drop macro table posts_of;
create macro posts_of (uid) as table (select * from posts where ((user_id = uid) and (title is not null)));

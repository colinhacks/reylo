create table comment (id integer primary key, parent_id integer references comment (id), comment varchar, author varchar); create index comment_author on comment (author);

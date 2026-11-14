drop index comment_author;
alter table comment rename column comment to body;
create index comment_author on "comment"(author);
-- comment.comment is renamed body, keeping its rows. If they are two columns, drop the one and add the other instead.

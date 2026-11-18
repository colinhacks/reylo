alter table posts
  add fulltext key body_ft (body),
  add key note_idx (note);
alter table posts add fulltext key title_ft (title);

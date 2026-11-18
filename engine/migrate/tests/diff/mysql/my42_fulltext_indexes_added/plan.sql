alter table posts
  add fulltext key a_ft (a),
  add key c_prefix (c(10)),
  add key note_idx (note);
alter table posts add fulltext key b_ft (b);
alter table posts add fulltext key body_ft (body);
alter table posts add fulltext key title_ft (title);

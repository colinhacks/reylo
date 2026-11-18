create table posts (id int primary key, title text not null, body text not null, note varchar(10), fulltext key title_ft (title), fulltext key body_ft (body), key note_idx (note));

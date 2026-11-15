create sequence s increment by 2; create table pages (id integer primary key, parent_id integer references pages (id), kind varchar default 'a' check (kind <> 's'));

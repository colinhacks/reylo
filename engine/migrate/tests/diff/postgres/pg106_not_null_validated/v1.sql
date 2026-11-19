create table users (id integer primary key, phone text, email text);
alter table users add constraint users_phone_not_null not null phone not valid;

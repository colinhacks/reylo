create extension if not exists btree_gist;
create table bookings (id integer primary key, room integer not null, during tstzrange not null);

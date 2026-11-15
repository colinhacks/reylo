create extension if not exists btree_gist;
create table bookings (id integer primary key, room integer not null, during tstzrange not null, exclude using gist (room with =, during with &&) where (room > 0));

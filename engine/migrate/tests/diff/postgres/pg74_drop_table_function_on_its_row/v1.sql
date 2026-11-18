create table kept (id integer primary key);
create type booking_status as enum ('pending', 'confirmed');
create table bookings (id integer primary key, status booking_status not null default 'pending');
create function booking_priority(b bookings) returns integer language sql immutable as $$ select case b.status when 'pending' then 0 else 1 end $$;
alter table bookings add constraint bookings_priority_check check (booking_priority(bookings) >= 0);
create index bookings_priority on bookings (booking_priority(bookings));

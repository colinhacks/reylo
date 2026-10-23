-- What `.try()`'s error type is narrowed to: each constraint a statement writes through, and
-- nothing where the statement may run a user's code (wiki/run-safe.md).
create domain positive as int constraint positive_check check (value > 0);
create table users (id int primary key, email text unique not null, age positive, constraint adult check (age >= 18));
create unique index users_lower_email on users (lower(email));
create table posts (id int primary key, user_id int not null references users (id) on delete cascade, title text not null);
create table comments (id int primary key, post_id int references posts (id) on delete restrict);
create table audited (id int primary key);
create function audit() returns trigger language plpgsql as $$ begin return new; end $$;
create trigger audited_audit before insert on audited for each row execute function audit();
create function next_code() returns text language sql as $$ select 'x' $$;
create table coded (id int primary key, code text default next_code());
create function boom() returns int language plpgsql as $$ begin raise exception 'boom' using errcode = 'U0001'; end $$;
create table rooms (id int primary key, during tstzrange, constraint rooms_no_overlap exclude using gist (during with &&));
create table events (id int, at date not null, primary key (id, at)) partition by range (at);
create table events_2026 partition of events for values from ('2026-01-01') to ('2027-01-01');
create view user_emails as select id, email from users;
insert into users (id, email, age) values (1, 'ada@example.com', 30);
insert into posts (id, user_id, title) values (1, 1, 'hello');
insert into comments (id, post_id) values (1, 1);
create type mood as enum ('ok', 'sad');
create table moods (id int primary key, m mood);
create function judge(mood) returns int language plpgsql as $$ begin if $1 = 'sad' then raise exception 'sad' using errcode = 'U0002'; end if; return 1; end $$;
create function mood_over(mood, int) returns bool language plpgsql as $$ begin if $1 = 'sad' then raise exception 'sad' using errcode = 'U0003'; end if; return true; end $$;
create operator ~~~ (leftarg = mood, rightarg = int, function = mood_over);
insert into moods (id, m) values (1, 'ok'), (2, 'sad');

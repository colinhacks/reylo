create type mood as enum ('sad', 'ok', 'happy');
create domain good_mood as mood check (value <> 'sad');
create type entry as (m mood, note text);
create table t (id integer primary key, g good_mood, e entry);

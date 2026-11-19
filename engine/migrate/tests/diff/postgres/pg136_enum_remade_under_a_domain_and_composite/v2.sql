create type mood as enum ('ok', 'happy');
create domain good_mood as mood check (value <> 'ok');
create type entry as (m mood, note text);
create table t (id integer primary key, g good_mood, e entry);

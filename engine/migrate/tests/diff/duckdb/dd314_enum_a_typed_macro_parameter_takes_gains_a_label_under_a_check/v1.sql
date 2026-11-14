create type mood as enum ('a', 'b'); create macro is_a(m mood) as m = 'a'; create table t (id integer primary key, m mood check (is_a(m) or m = 'b'));

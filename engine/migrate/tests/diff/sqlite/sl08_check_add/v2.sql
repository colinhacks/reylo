create table users (id integer primary key, full_name text not null, n integer not null, code text, constraint n_pos check (n > 0));

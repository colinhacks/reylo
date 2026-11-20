create table t (a int not null, b int primary key, c int);
create statistics s on a, c from t;
create domain d1 as int constraint d1_check check (value > 0);
create domain d2 as int constraint d2_check check (value > 0);
comment on constraint d2_check on domain d2 is 'positive';
create table x (c int, constraint x_c_key unique (c));
comment on index x_c_key is 'the index of a key';

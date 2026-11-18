create table test (id integer, constraint "my.constraint" check (id > 0));
comment on constraint "my.constraint" on test is 'a constraint with a dot in its name';

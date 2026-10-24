-- A call of one argument that no function takes is the argument's column of that name where the
-- argument is a row, a table's whole row, a subquery's or a composite column (ParseFuncOrColumn's
-- projection), and that column may be NULL however the call's own rule reads it. A call over a
-- value that is no row is the function still.
create table t (id int primary key, length int, md5 text, abs int, cardinality int, now timestamptz, concat_ws text, uuidv7 uuid);
create type pair as (length int, md5 text);
create table c (id int primary key, p pair not null, name text not null);
create table s (id int primary key, sum int, every bool, n int not null);
insert into t values (1, null, null, null, null, null, null, null);
insert into c values (1, row(null, null), 'x');
insert into s values (1, null, null, 1), (2, null, null, 2);

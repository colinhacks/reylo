-- Nullability is the server's NOT NULL flag, which the engine takes as it is: sound where it is
-- set, and nullable for string and date functions and most casts of NOT NULL inputs. Only what the
-- statement proves narrows it: a strict test in the WHERE (`engine/mysql/src/mysql_nulls.rs`)
-- proves the column it tests and the row of an outer join, an aggregate under a plain GROUP BY and
-- one over a window frame that holds the current row are as NULL as their argument, and their
-- JSON_ARRAYAGG and JSON_OBJECTAGG are never NULL (`grouped_flags` and `window_flags` in
-- `engine/mysql/src/mysql.rs`). Every
-- column here is NOT NULL but those named `maybe_`, and the edge state holds zero dates, the
-- bounds of every integer and an empty string.
create table t (
  id int primary key, n int not null, big bigint not null, u bigint unsigned not null, small tinyint not null,
  price decimal(10,2) not null, ratio double not null, name varchar(20) not null, body text not null,
  raw varbinary(8) not null, day date not null, at datetime not null, ts timestamp not null, tm time not null, y year not null,
  kind enum('a','b') not null, tags set('x','y') not null, doc json not null, bits bit(8) not null, flag boolean not null,
  maybe_n int, maybe_name varchar(20), maybe_day date, maybe_doc json
);
create table other (id int primary key, t_id int not null, label varchar(20) not null, maybe_label varchar(20));
create table nothing (id int primary key, n int not null, check (false));
create view shown as select id, n, n > 1 as more, coalesce(n, 0) as m, small, quarter(day) as q, big from t;
create view far_days as select id, from_days(n) as d, from_days(3652499) as far from t;
